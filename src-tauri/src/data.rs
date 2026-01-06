use tauri::{Manager, AppHandle, path::BaseDirectory};

use crate::audio::AudioDeviceConfig;

const DEVICE_DATA_FILE: &str = "devices.json";

pub fn init_device_data(app_handle: &AppHandle) -> tauri::Result<()> {
  let devices_path = app_handle.path().resolve(DEVICE_DATA_FILE, BaseDirectory::AppData)?;
  if !devices_path.exists() {
    std::fs::create_dir_all(devices_path.parent().unwrap())?;
    std::fs::write(&devices_path, serde_json::to_string_pretty(&AudioDeviceConfig::default())?)?;
  }

  let json_str = std::fs::read_to_string(&devices_path)?;
  if serde_json::from_str::<AudioDeviceConfig>(&json_str).is_err() {
    std::fs::write(&devices_path, serde_json::to_string_pretty(&AudioDeviceConfig::default())?)?;
  }
  Ok(())
}

pub fn write_device_data(app_handle: &AppHandle, data: AudioDeviceConfig) -> Result<(), String> {
  let devices_path = app_handle
    .path()
    .resolve(DEVICE_DATA_FILE, BaseDirectory::AppData)
    .map_err(|err| format!("{}", err))?;

  let json_str = serde_json::to_string_pretty(&data)
    .map_err(|err| format!("{}", err))?;

  std::fs::write(&devices_path, json_str)
    .map_err(|err| format!("{}", err))?;

  Ok(())
}

pub fn read_device_data(app_handle: &AppHandle) -> Result<AudioDeviceConfig, String> {
  let devices_path = app_handle.path().resolve(DEVICE_DATA_FILE, BaseDirectory::AppData)
    .map_err(|err| format!("{}", err))?;

  let json_str = std::fs::read_to_string(&devices_path)
    .map_err(|err| format!("{}", err))?;
  
  serde_json::from_str(&json_str)
    .map_err(|err| format!("{}", err))
}
