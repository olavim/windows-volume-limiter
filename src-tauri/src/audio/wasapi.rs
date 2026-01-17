use windows::Win32::Devices::FunctionDiscovery::{PKEY_DeviceInterface_FriendlyName};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{DEVICE_STATE_ACTIVE, IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator, MMDeviceEnumerator, eRender};
use windows::Win32::System::Com::{CLSCTX_ALL, CLSCTX_INPROC_SERVER, CoCreateInstance, STGM_READ};

use crate::audio::{AudioDevice, AudioDeviceEnumerator};

pub struct WasapiAudioDevice {
  mm_device: IMMDevice,
  volume_interface: IAudioEndpointVolume
}

impl WasapiAudioDevice {
  pub fn from_mm_device(mm_device: IMMDevice) -> Result<Self, String> {
    let volume_interface = unsafe { 
      mm_device
        .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
        .map_err(|err| format!("Couldn't activate IAudioEndpointVolume: {err}"))?
    };
    Ok(WasapiAudioDevice {
      mm_device,
      volume_interface
    })
  }

  unsafe fn get_property(&self, pkey: &PROPERTYKEY) -> Result<String, String> {
    let value = self.mm_device
      .OpenPropertyStore(STGM_READ)
      .map_err(|err| format!("Couldn't open device property store: {err}"))?
      .GetValue(pkey)
      .map_err(|err| format!("Couldn't get property value: {err}"))?;

    Ok(value.to_string())
  }
}

impl AudioDevice for WasapiAudioDevice {
  fn get_id(&self) -> Result<String, String> {
    let id = unsafe {
      self.mm_device
        .GetId()
        .map_err(|err| format!("Couldn't get device ID: {err}"))?
        .to_string()
        .map_err(|err| format!("Couldn't get device ID: {err}"))?
    };
    Ok(id)
  }

  fn get_name(&self) -> Result<String, String> {
    unsafe { self.get_property(&PKEY_DeviceInterface_FriendlyName) }
  }

  fn get_volume(&self) -> Result<f32, String> {
    unsafe {
      self.volume_interface
        .GetMasterVolumeLevelScalar()
        .map_err(|err| format!("Couldn't get device volume: {err}"))
    }
  }

  fn set_volume(&mut self, volume: f32) -> Result<(), String> {
    unsafe {
      self.volume_interface
        .SetMasterVolumeLevelScalar(volume, std::ptr::null())
        .map_err(|err| format!("Couldn't set device volume: {err}"))
    }
  }
}

struct WasapiAudioDeviceCollection {
  mm_device_collection: IMMDeviceCollection
}

impl WasapiAudioDeviceCollection {
  pub fn from_enumerator(enumerator: &IMMDeviceEnumerator) -> Result<Self, String> {
    let mm_device_collection = unsafe { 
      enumerator
        .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
        .map_err(|err| format!("Couldn't get active device collection: {err}"))?
    };
    Ok(WasapiAudioDeviceCollection { mm_device_collection })
  }

  pub fn get_count(&self) -> Result<u32, String> {
    unsafe {
      self.mm_device_collection
        .GetCount()
        .map_err(|err| format!("Couldn't get device collection count: {err}"))
    }
  }

  pub fn get_device(&self, index: u32) -> Result<WasapiAudioDevice, String> {
    let device = unsafe {
      self.mm_device_collection
        .Item(index)
        .map_err(|err| format!("Couldn't get device at index {index}: {err}"))?
    };
    WasapiAudioDevice::from_mm_device(device)
  }
}

impl IntoIterator for WasapiAudioDeviceCollection {
  type Item = WasapiAudioDevice;
  type IntoIter = WasapiAudioDeviceCollectionIntoIter;

  fn into_iter(self) -> Self::IntoIter {
    WasapiAudioDeviceCollectionIntoIter {
      collection: self,
      index: 0
    }
  }
}

pub struct WasapiAudioDeviceCollectionIntoIter {
  collection: WasapiAudioDeviceCollection,
  index: u32
}

impl Iterator for WasapiAudioDeviceCollectionIntoIter {
  type Item = WasapiAudioDevice;

  fn next(&mut self) -> Option<Self::Item> {
    if self.index >= self.collection.get_count().ok()? {
      return None;
    }

    let device = self.collection.get_device(self.index).ok()?;
    self.index += 1;
    Some(device)
  }
}

pub struct WasapiAudioDeviceEnumerator {
  mm_device_enumerator: IMMDeviceEnumerator
}

impl AudioDeviceEnumerator<WasapiAudioDevice> for WasapiAudioDeviceEnumerator {
  fn init() -> Result<Self, String> {
    let mm_device_enumerator = unsafe {
      CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
        .map_err(|err| format!("Couldn't create device enumerator instance: {err}"))?
    };

    Ok(WasapiAudioDeviceEnumerator { mm_device_enumerator })
  }

  fn into_iter(&self) -> impl Iterator<Item = WasapiAudioDevice> {
    WasapiAudioDeviceCollection::from_enumerator(&self.mm_device_enumerator)
      .unwrap()
      .into_iter()
  }
}
