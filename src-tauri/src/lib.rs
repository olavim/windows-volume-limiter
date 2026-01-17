use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{Builder, Emitter, Manager, State, WindowEvent};

use crate::audio::AudioDeviceInfo;
use crate::data::{init_device_data, read_device_data, write_device_data};

mod audio;
mod data;

#[tauri::command]
fn set_device_max_volume(app_handle: tauri::AppHandle, device_id: &str, volume: f32) -> Result<(), String> {
  let state = app_handle.state::<Mutex<AppState>>();
  let controller = &mut state.lock().unwrap().audio_controller;

  controller.set_device_max_volume(device_id, volume)?;
  write_device_data(&app_handle, controller.into())?;
  Ok(())
}

#[tauri::command]
fn set_global_max_volume(app_handle: tauri::AppHandle, volume: f32) -> Result<(), String> {
  let state = app_handle.state::<Mutex<AppState>>();
  let controller = &mut state.lock().unwrap().audio_controller;
  controller.set_global_max_volume(volume)?;
  write_device_data(&app_handle, controller.into())?;
  Ok(())
}

#[tauri::command]
fn get_devices(state: State<'_, Mutex<AppState>>) -> Vec<AudioDeviceInfo> {
  (&state).lock().unwrap().audio_controller.get_devices()
}

#[tauri::command]
fn get_global_max_volume(state: State<'_, Mutex<AppState>>) -> f32 {
  (&state).lock().unwrap().audio_controller.get_global_max_volume()
}

struct AppState {
  audio_controller: audio::AudioController
}
unsafe impl Send for AppState {}

async fn run_periodic(interval_ms: u64, cb: impl Fn() + Send + 'static) {
  loop {
    cb();
    tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;
  }
}

async fn periodic_update_devices(interval_ms: u64, app_handle: tauri::AppHandle) {
  run_periodic(interval_ms, move || {
    let state = app_handle.state::<Mutex<AppState>>();
    let controller = &mut state.lock().unwrap().audio_controller;

    match controller.update_devices() {
      Err(err) => app_handle.emit("error", format!("Couldn't update audio devices: {err}")).unwrap(),
      Ok(true) => app_handle.emit("devices-updated", &controller.get_devices()).unwrap(),
      Ok(false) => {}
    }
  }).await;
}

async fn periodic_apply_volume_limits(interval_ms: u64, app_handle: tauri::AppHandle) {
  run_periodic(interval_ms, move || {
    let state = app_handle.state::<Mutex<AppState>>();
    let controller = &mut state.lock().unwrap().audio_controller;
    for device in controller.get_devices() {
      match controller.apply_max_volume(&device.id) {
        Err(err) => app_handle.emit("error", format!("Couldn't apply volume limit to device '{}': {err}", device.name)).unwrap(),
        Ok(()) => {}
      }
    }
  }).await;
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  Builder::default()
    .setup(|app| {
      let show_item = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
      let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
      let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

      TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Volume Limiter")
        .on_tray_icon_event(|tray, event| match event {
          TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => {
            let window = tray.app_handle().get_webview_window("main").unwrap();
            window.show().unwrap();
            window.set_focus().unwrap();
          },
          _ => {}
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
          "show" => {
            let window = app.get_webview_window("main").unwrap();
            window.show().unwrap();
            window.set_focus().unwrap();
          },
          "quit" => app.exit(0),
          _ => {}
        })
        .build(app)?;

      init_device_data(app.handle())?;
      let device_data = read_device_data(app.handle())?;

      app.manage(Mutex::new(AppState {
        audio_controller: audio::AudioController::init(device_data)?
      }));

      tauri::async_runtime::spawn(periodic_update_devices(500, app.handle().clone()));
      tauri::async_runtime::spawn(periodic_apply_volume_limits(50, app.handle().clone()));

      Ok(())
    })
    .on_window_event(|window, event| match event {
      WindowEvent::CloseRequested { api, .. } => {
        window.hide().unwrap();
        api.prevent_close();
      },
      _ => {}
    })
    .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
      let _ = app.get_webview_window("main")
        .expect("no main window")
        .set_focus();
    }))
    .plugin(tauri_plugin_opener::init())
    .invoke_handler(tauri::generate_handler![set_device_max_volume, set_global_max_volume, get_global_max_volume, get_devices])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
