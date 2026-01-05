use std::collections::HashMap;

use windows::{Win32::{
  Devices::FunctionDiscovery::PKEY_DeviceInterface_FriendlyName, Media::Audio::{
    DEVICE_STATE, DEVICE_STATE_ACTIVE, DEVICE_STATE_DISABLED, Endpoints::IAudioEndpointVolume, IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator, eRender
  }, System::Com::{CLSCTX_ALL, CLSCTX_INPROC_SERVER, CoCreateInstance, STGM_READ}
}, core::PWSTR};

unsafe fn get_device_name(device: &IMMDevice) -> String {
  let props = device.OpenPropertyStore(STGM_READ).unwrap_or_else(|err| {
    panic!("ERROR: Couldn't open device property store {err}");
  });
  let name = props.GetValue(&PKEY_DeviceInterface_FriendlyName).unwrap_or_else(|err| {
    panic!("ERROR: Couldn't get device friendly name {err}");
  });
  name.to_string()
}

unsafe fn get_audio_endpoints(imm_device_enumerator: &IMMDeviceEnumerator) -> IMMDeviceCollection {
  imm_device_enumerator
    .EnumAudioEndpoints(eRender, DEVICE_STATE(DEVICE_STATE_ACTIVE.0 | DEVICE_STATE_DISABLED.0))
    .unwrap_or_else(|err| {
      panic!("ERROR: Couldn't enumerate audio devices {err}");
    })
}

unsafe fn get_current_devices(imm_device_enumerator: &IMMDeviceEnumerator) -> Vec<AudioDevice> {
  let endpoints = get_audio_endpoints(imm_device_enumerator);
  let device_count = endpoints.GetCount().unwrap_or_else(|err| {
    panic!("ERROR: Couldn't get device count {err}");
  });
  
  let mut devices: Vec<AudioDevice> = Vec::new();
  for i in 0..device_count {
    let device = endpoints.Item(i).unwrap_or_else(|err| {
      panic!("ERROR: Couldn't get audio device at index {i}: {err}");
    });
    let device_id = device.GetId()
      .unwrap_or_else(|err| {
        panic!("ERROR: Couldn't get device ID for device at index {i}: {err}");
      });
    let device_name = get_device_name(&device);
    devices.push(AudioDevice {
      id: device_id,
      name: device_name,
      ptr: device
    });
  }

  devices
}

pub struct AudioDevice {
  id: PWSTR,
  name: String,
  ptr: IMMDevice
}

impl AudioDevice {
  pub unsafe fn get_volume(&self) -> f32 {
    self.ptr
      .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
      .unwrap_or_else(|err| {
        eprintln!("ERROR: Couldn't activate IAudioEndpointVolume for device \"{}\": {err}", self.name);
        std::process::exit(1);
      })
      .GetMasterVolumeLevelScalar()
      .unwrap_or_else(|err| {
        eprintln!("ERROR: Couldn't get volume level for device \"{}\": {err}", self.name);
        std::process::exit(1);
      })
  }

  pub unsafe fn set_volume(&mut self, volume: f32) -> () {
    self.ptr
      .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
      .unwrap_or_else(|err| {
        eprintln!("ERROR: Couldn't activate IAudioEndpointVolume for device \"{}\": {err}", self.name);
        std::process::exit(1);
      })
      .SetMasterVolumeLevelScalar(volume, std::ptr::null())
      .unwrap_or_else(|err| {
        eprintln!("ERROR: Couldn't set volume level for device \"{}\": {err}", self.name);
        std::process::exit(1);
      })
  }
}

#[derive(serde::Serialize)]
pub struct AudioDeviceInfo {
  id: String,
  name: String,
  max_volume: f32
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AudioDeviceConfig {
  pub global_max_volume: f32,
  pub device_max_volumes: HashMap<String, f32>
}

impl Default for AudioDeviceConfig {
  fn default() -> Self {
    AudioDeviceConfig {
      global_max_volume: 1.0,
      device_max_volumes: HashMap::new()
    }
  }
}

pub struct AudioController {
  imm_device_enumerator: IMMDeviceEnumerator,
  global_max_volume: f32,
  device_max_volumes: HashMap<String, f32>,
  devices: Vec<AudioDevice>
}

impl Into<AudioDeviceConfig> for &mut AudioController {
  fn into(self) -> AudioDeviceConfig {
    AudioDeviceConfig {
      global_max_volume: self.global_max_volume,
      device_max_volumes: self.device_max_volumes.clone()
    }
  }
}

impl AudioController {
  pub fn new(config: AudioDeviceConfig) -> Self {
    unsafe {
      let imm_device_enumerator: IMMDeviceEnumerator = CoCreateInstance(
        &windows::Win32::Media::Audio::MMDeviceEnumerator,
        None,
        CLSCTX_INPROC_SERVER,
      )
      .expect("Failed to create IMMDeviceEnumerator");

      let devices = get_current_devices(&imm_device_enumerator);

      AudioController {
        imm_device_enumerator: imm_device_enumerator,
        global_max_volume: config.global_max_volume,
        device_max_volumes: config.device_max_volumes,
        devices: devices
      }
    }
  }

  pub fn update_devices(&mut self) -> bool {
    unsafe {
      let new_devices = get_current_devices(&self.imm_device_enumerator);
      let changed = new_devices.len() != self.devices.len()
        || new_devices.iter().zip(self.devices.iter()).any(|(new, old)| new.id != old.id);
      self.devices = new_devices;
      changed
    }
  }

  pub fn get_devices(&self) -> Vec<AudioDeviceInfo> {
    unsafe {
      self.devices.iter().map(|device| {
        AudioDeviceInfo {
          id: device.id.to_string().unwrap(),
          name: device.name.clone(),
          max_volume: self.device_max_volumes.get(&device.id.to_string().unwrap()).cloned().unwrap_or(1.0)
        }
      }).collect()
    }
  }

  pub fn get_global_max_volume(&self) -> f32 {
    self.global_max_volume
  }

  pub fn set_device_max_volume(&mut self, device_id: &str, max_volume: f32) -> () {
    self.device_max_volumes.insert(device_id.to_string(), max_volume);
    self.apply_volume_limits();
  }

  pub fn set_global_max_volume(&mut self, max_volume: f32) -> () {
    self.global_max_volume = max_volume;
    self.apply_volume_limits();
  }

  pub fn apply_volume_limits(&mut self) -> () {
    unsafe {
      for device in &mut self.devices {
        let device_id = device.id.to_string().unwrap();
        let max_volume = match self.device_max_volumes.get(&device_id) {
          Some(volume) => f32::min(*volume, self.global_max_volume),
          None => self.global_max_volume,
        };
        if device.get_volume() > max_volume {
          device.set_volume(max_volume);
        }
      }
    }
  }
}
