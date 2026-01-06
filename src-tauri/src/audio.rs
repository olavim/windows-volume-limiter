use std::collections::HashMap;

use windows::{Win32::{
  Devices::FunctionDiscovery::PKEY_DeviceInterface_FriendlyName, Media::Audio::{
    DEVICE_STATE, DEVICE_STATE_ACTIVE, DEVICE_STATE_DISABLED, Endpoints::IAudioEndpointVolume, IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator, eRender
  }, System::Com::{CLSCTX_ALL, CLSCTX_INPROC_SERVER, CoCreateInstance, STGM_READ}
}, core::PWSTR};

struct IMMDeviceHandle {
  imm_device: IMMDevice
}

impl IMMDeviceHandle {
  pub fn new(imm_device: IMMDevice) -> Self {
    IMMDeviceHandle {
      imm_device
    }
  }

  pub fn get_id(&self) -> Result<PWSTR, String> {
    unsafe {
      self.imm_device
        .GetId()
        .map_err(|err| format!("Couldn't get device ID: {err}"))
    }
  }

  pub fn get_name(&self) -> Result<String, String> {
    unsafe {
      let name = self.imm_device
        .OpenPropertyStore(STGM_READ)
        .map_err(|err| format!("Couldn't open device property store: {err}"))?
        .GetValue(&PKEY_DeviceInterface_FriendlyName)
        .map_err(|err| format!("Couldn't get property value: {err}"))?
        .to_string();

      Ok(name)
    }
  }

  pub fn get_volume_interface(&self) -> Result<IAudioEndpointVolume, String> {
    unsafe {
      self.imm_device
        .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
        .map_err(|err| format!("Couldn't activate IAudioEndpointVolume: {err}"))
    }
  }
}

struct AudioDeviceVolume {
  volume_interface: IAudioEndpointVolume,
  device_name: String
}

impl AudioDeviceVolume {
  pub fn from(imm_device: IMMDeviceHandle) -> Result<Self, String> {
    let volume_interface = imm_device.get_volume_interface()?;
    let device_name = imm_device.get_name()?;
    Ok(AudioDeviceVolume {
      volume_interface,
      device_name
    })
  }

  pub unsafe fn get_volume(&self) -> Result<f32, String> {
    unsafe {
      self.volume_interface
        .GetMasterVolumeLevelScalar()
        .map_err(|err| format!("Couldn't get volume for device \"{}\": {err}", self.device_name))
    }
  }

  pub unsafe fn set_volume(&mut self, volume: f32) -> Result<(), String> {
    unsafe {
      self.volume_interface
        .SetMasterVolumeLevelScalar(volume, std::ptr::null())
        .map_err(|err| format!("Couldn't set volume for device \"{}\": {err}", self.device_name))
    }
  }
}

struct AudioDevice {
  id: PWSTR,
  name: String,
  device_volume: AudioDeviceVolume
}

impl AudioDevice {
  pub fn from(imm_device: IMMDeviceHandle) -> Result<Self, String> {
    Ok(AudioDevice {
      id: imm_device.get_id()?,
      name: imm_device.get_name()?,
      device_volume: AudioDeviceVolume::from(imm_device)?
    })
  }

  pub unsafe fn get_volume(&self) -> Result<f32, String> {
    self.device_volume.get_volume()
  }

  pub unsafe fn set_volume(&mut self, volume: f32) -> Result<(), String> {
    self.device_volume.set_volume(volume)
  }
}

struct AudioDeviceCollection {
  imm_device_collection: IMMDeviceCollection
}

impl AudioDeviceCollection {
  pub fn new(imm_device_collection: IMMDeviceCollection) -> Self {
    AudioDeviceCollection {
      imm_device_collection
    }
  }

  fn get_count(&self) -> Result<u32, String> {
    unsafe {
      self.imm_device_collection
        .GetCount()
        .map_err(|err| format!("ERROR: Couldn't get device count: {err}"))
    }
  }

  pub fn get(&self, index: u32) -> Result<AudioDevice, String> {
    unsafe {
      let imm_device = self.imm_device_collection.Item(index).map_err(|err| format!("ERROR: Couldn't get device at index {index}: {err}"))?;
      AudioDevice::from(IMMDeviceHandle::new(imm_device))
    }
  }

  pub fn iter(&self) -> AudioDeviceCollectionIterator {
    AudioDeviceCollectionIterator {
      collection: self,
      current_index: 0,
      total_count: self.get_count().unwrap_or(0)
    }
  }
}

struct AudioDeviceCollectionIterator<'a> {
  collection: &'a AudioDeviceCollection,
  current_index: u32,
  total_count: u32
}

impl Iterator for AudioDeviceCollectionIterator<'_> {
  type Item = AudioDevice;

  fn next(&mut self) -> Option<Self::Item> {
    if self.current_index >= self.total_count {
      None
    } else {
      let item = self.collection.get(self.current_index).unwrap();
      self.current_index += 1;
      Some(item)
    }
  }
}

struct AudioDeviceEnumerator {
  imm_device_enumerator: IMMDeviceEnumerator
}

impl AudioDeviceEnumerator {
  pub fn init() -> Result<Self, String> {
    unsafe {
      let imm_device_enumerator: IMMDeviceEnumerator = CoCreateInstance(
        &windows::Win32::Media::Audio::MMDeviceEnumerator,
        None,
        CLSCTX_INPROC_SERVER,
      )
      .map_err(|err| format!("Couldn't create IMMDeviceEnumerator: {err}"))?;

      Ok(AudioDeviceEnumerator {
        imm_device_enumerator: imm_device_enumerator
      })
    }
  }

  unsafe fn get_audio_endpoints(&self) -> Result<AudioDeviceCollection, String> {
    let collection = self.imm_device_enumerator
      .EnumAudioEndpoints(eRender, DEVICE_STATE(DEVICE_STATE_ACTIVE.0 | DEVICE_STATE_DISABLED.0))
      .map_err(|err| format!("Couldn't get audio endpoints {err}"))?;

    Ok(AudioDeviceCollection::new(collection))
  }

  pub fn get_devices(&self) -> Result<Vec<AudioDevice>, String> {
    unsafe {
      let endpoints = self.get_audio_endpoints()?;
      Ok(endpoints.iter().collect())
    }
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
  device_enumerator: AudioDeviceEnumerator,
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
      device_enumerator: AudioDeviceEnumerator::init()?,
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
    unsafe {
      self.device_cache.iter().map(|device| {
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
    unsafe {
      for device in &mut self.device_cache {
        let device_id = device.id.to_string().unwrap();
        let max_volume = match self.device_max_volumes.get(&device_id) {
          Some(volume) => f32::min(*volume, self.global_max_volume),
          None => self.global_max_volume,
        };
        if device.get_volume()? > max_volume {
          device.set_volume(max_volume)?;
        }
      }
    }

    Ok(())
  }
}
