use std::collections::HashMap;

mod wasapi;

type AudioDeviceEnumeratorImpl = crate::audio::wasapi::WasapiAudioDeviceEnumerator;

pub trait AudioDevice {
  fn get_id(&self) -> Result<String, String>;
  fn get_name(&self) -> Result<String, String>;
  fn get_volume(&self) -> Result<f32, String>;
  fn set_volume(&mut self, volume: f32) -> Result<(), String>;
}

pub trait AudioDeviceEnumerator<T: AudioDevice> {
  fn init() -> Result<Self, String> where Self: Sized;
  fn into_iter(&self) -> impl Iterator<Item = T>;
}

#[derive(serde::Serialize)]
pub struct AudioDeviceInfo {
  pub id: String,
  pub name: String,
  pub max_volume: f32
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
  device_cache: HashMap<String, Box<dyn AudioDevice>>,
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
      device_cache: HashMap::new(),
      global_max_volume: config.global_max_volume,
      device_max_volumes: config.device_max_volumes
    })
  }

  pub fn update_devices(&mut self) -> Result<bool, String> {
    let new_devices = self.device_enumerator.into_iter()
      .map(|device| {
        let id = device.get_id().unwrap_or_default();
        (id, Box::new(device) as Box<dyn AudioDevice>)
      })
      .collect::<HashMap<_, _>>();
    let changed = new_devices.len() != self.device_cache.len()
      || new_devices.keys().any(|id| !self.device_cache.contains_key(id));
    self.device_cache = new_devices;
    Ok(changed)
  }

  fn to_audio_device_info(&self, device: &Box<dyn AudioDevice>) -> Result<AudioDeviceInfo, String> {
    let id = device.get_id()?;
    Ok(AudioDeviceInfo {
      id: id.clone(),
      name: device.get_name()?,
      max_volume: self.device_max_volumes.get(&id).cloned().unwrap_or(1.0)
    })
  }

  pub fn get_devices(&self) -> Vec<AudioDeviceInfo> {
    let mut devices: Vec<_> = self.device_cache.iter()
      .filter_map(|(_, device)| {
        match self.to_audio_device_info(device) {
          Ok(info) => Some(info),
          Err(err) => {
            eprintln!("{err}");
            None
          }
        }
      })
      .collect::<Vec<_>>();

    devices.sort_by(|a, b| match a.name.cmp(&b.name) {
      std::cmp::Ordering::Equal => a.id.cmp(&b.id),
      other => other
    });
    devices
  }

  pub fn get_global_max_volume(&self) -> f32 {
    self.global_max_volume
  }

  pub fn set_device_max_volume(&mut self, device_id: &str, max_volume: f32) -> Result<(), String> {
    if max_volume < 0.0 || max_volume > 1.0 {
      return Err("Max volume must be between 0.0 and 1.0".to_string());
    }

    self.device_max_volumes.insert(device_id.to_string(), max_volume);
    self.apply_max_volume(device_id)
  }

  pub fn set_global_max_volume(&mut self, max_volume: f32) -> Result<(), String> {
    if max_volume < 0.0 || max_volume > 1.0 {
      return Err("Max volume must be between 0.0 and 1.0".to_string());
    }
    
    self.global_max_volume = max_volume;

    let device_ids: Vec<_> = self.device_cache.keys().cloned().collect();
    device_ids.iter().fold(Ok(()), |res, device_id| res.and(self.apply_max_volume(device_id)))
  }

  pub fn apply_max_volume(&mut self, device_id: &str) -> Result<(), String> {
    let device = self.device_cache.get_mut(device_id)
      .ok_or_else(|| format!("Device with ID '{}' not found", device_id))?;

    let device_volume = device.get_volume()?;
    let max_volume = match self.device_max_volumes.get(device_id) {
      Some(volume) => f32::min(*volume, self.global_max_volume),
      None => self.global_max_volume,
    };

    if device_volume > max_volume {
      device.set_volume(max_volume)?;
    }

    Ok(())
  }
}
