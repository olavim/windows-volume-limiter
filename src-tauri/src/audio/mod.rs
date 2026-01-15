use std::collections::HashMap;

mod windows;

type AudioDeviceEnumeratorImpl = crate::audio::windows::WinAudioDeviceEnumerator;
type AudioDeviceVolumeImpl = crate::audio::windows::WinAudioDeviceVolume;

pub struct AudioDevice {
  id: String,
  name: String,
  volume_interface: AudioDeviceVolumeImpl
}

impl AudioDevice {
  pub fn new(id: String, name: String, volume_interface: AudioDeviceVolumeImpl) -> Self {
    AudioDevice {
      id,
      name,
      volume_interface
    }
  }

  pub fn get_volume(&self) -> Result<f32, String> {
    self.volume_interface.get_volume()
  }

  pub fn set_volume(&mut self, volume: f32) -> Result<(), String> {
    self.volume_interface.set_volume(volume)
  }
}

pub trait AudioDeviceVolume {
  fn get_volume(&self) -> Result<f32, String>;
  fn set_volume(&mut self, volume: f32) -> Result<(), String>;
}

pub trait AudioDeviceEnumerator {
  fn init() -> Result<Self, String> where Self: Sized;
  fn get_devices(&self) -> Result<Vec<AudioDevice>, String>;
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
  device_enumerator: AudioDeviceEnumeratorImpl,
  device_cache: Vec<AudioDevice>,
  global_max_volume: f32,
  device_max_volumes: HashMap<String, f32>
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
  pub fn init(config: AudioDeviceConfig) -> Result<Self, String> {
    Ok(AudioController {
      device_enumerator: AudioDeviceEnumeratorImpl::init()?,
      device_cache: Vec::new(),
      global_max_volume: config.global_max_volume,
      device_max_volumes: config.device_max_volumes
    })
  }

  pub fn update_devices(&mut self) -> Result<bool, String> {
    let new_devices = self.device_enumerator.get_devices()?;
    let changed = new_devices.len() != self.device_cache.len()
      || new_devices.iter().zip(self.device_cache.iter()).any(|(new, old)| new.id != old.id);
    self.device_cache = new_devices;
    Ok(changed)
  }

  pub fn get_device_info(&self) -> Vec<AudioDeviceInfo> {
    self.device_cache.iter().map(|device| {
      AudioDeviceInfo {
        id: device.id.clone(),
        name: device.name.clone(),
        max_volume: self.device_max_volumes.get(&device.id).cloned().unwrap_or(1.0)
      }
    }).collect()
  }

  pub fn get_global_max_volume(&self) -> f32 {
    self.global_max_volume
  }

  pub fn set_device_max_volume(&mut self, device_id: &str, max_volume: f32) -> Result<(), String> {
    if max_volume < 0.0 || max_volume > 1.0 {
      return Err("Max volume must be between 0.0 and 1.0".to_string());
    }

    self.device_max_volumes.insert(device_id.to_string(), max_volume);
    self.apply_volume_limits()
  }

  pub fn set_global_max_volume(&mut self, max_volume: f32) -> Result<(), String> {
    if max_volume < 0.0 || max_volume > 1.0 {
      return Err("Max volume must be between 0.0 and 1.0".to_string());
    }
    
    self.global_max_volume = max_volume;
    self.apply_volume_limits()
  }

  pub fn apply_volume_limits(&mut self) -> Result<(), String> {
    for device in &mut self.device_cache {
      let max_volume = match self.device_max_volumes.get(&device.id) {
        Some(volume) => f32::min(*volume, self.global_max_volume),
        None => self.global_max_volume,
      };
      if device.get_volume()? > max_volume {
        device.set_volume(max_volume)?;
      }
    }

    Ok(())
  }
}
