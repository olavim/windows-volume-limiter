use windows::Win32::Devices::FunctionDiscovery::{PKEY_Device_InstanceId, PKEY_DeviceInterface_FriendlyName};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{DEVICE_STATE_ACTIVE, IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator, MMDeviceEnumerator, eRender};
use windows::Win32::System::Com::{CLSCTX_ALL, CLSCTX_INPROC_SERVER, CoCreateInstance, STGM_READ};

use crate::audio::{AudioDevice, AudioDeviceEnumerator, AudioDeviceVolume};

unsafe fn get_device_property(device: &IMMDevice, pkey: &PROPERTYKEY) -> Result<String, String> {
  let value = device
    .OpenPropertyStore(STGM_READ)
    .map_err(|err| format!("Couldn't open device property store: {err}"))?
    .GetValue(pkey)
    .map_err(|err| format!("Couldn't get property value: {err}"))?;

  Ok(value.to_string())
}

unsafe fn get_device_volume_interface(device: &IMMDevice) -> Result<IAudioEndpointVolume, String> {
  device
    .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
    .map_err(|err| format!("Couldn't activate IAudioEndpointVolume: {err}"))
}

unsafe fn get_active_device_collection(enumerator: &IMMDeviceEnumerator) -> Result<IMMDeviceCollection, String> {
  enumerator
    .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
    .map_err(|err| format!("Couldn't get active device collection: {err}"))
}

unsafe fn get_device_collection_count(collection: &IMMDeviceCollection) -> Result<u32, String> {
  collection
    .GetCount()
    .map_err(|err| format!("Couldn't get device collection count: {err}"))
}

unsafe fn get_device_collection_device(collection: &IMMDeviceCollection, index: u32) -> Result<IMMDevice, String> {
  collection
    .Item(index)
    .map_err(|err| format!("Couldn't get device at index {index}: {err}"))
}

pub struct WinAudioDeviceVolume {
  volume_interface: IAudioEndpointVolume,
  device_name: String
}

impl WinAudioDeviceVolume {
  pub fn new(volume_interface: IAudioEndpointVolume, device_name: String) -> Result<Self, String> {
    Ok(WinAudioDeviceVolume {
      volume_interface: volume_interface,
      device_name: device_name
    })
  }
}

impl AudioDeviceVolume for WinAudioDeviceVolume {
  fn get_volume(&self) -> Result<f32, String> {
    unsafe {
      self.volume_interface
        .GetMasterVolumeLevelScalar()
        .map_err(|err| format!("Couldn't get volume for device \"{}\": {err}", self.device_name))
    }
  }

  fn set_volume(&mut self, volume: f32) -> Result<(), String> {
    unsafe {
      self.volume_interface
        .SetMasterVolumeLevelScalar(volume, std::ptr::null())
        .map_err(|err| format!("Couldn't set volume for device \"{}\": {err}", self.device_name))
    }
  }
}

pub struct WinAudioDeviceEnumerator {
  imm_device_enumerator: IMMDeviceEnumerator
}

impl AudioDeviceEnumerator for WinAudioDeviceEnumerator {
  fn init() -> Result<Self, String> {
    unsafe {
      let imm_device_enumerator = 
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
          .map_err(|err| format!("Couldn't create MMDeviceEnumerator: {err}"))?;

      Ok(WinAudioDeviceEnumerator {
        imm_device_enumerator: imm_device_enumerator
      })
    }
  }

  fn get_devices(&self) -> Result<Vec<AudioDevice>, String> {
    unsafe {
      let device_collection = get_active_device_collection(&self.imm_device_enumerator)?;
      let device_count = get_device_collection_count(&device_collection)?;
      let mut devices = Vec::with_capacity(device_count as usize);

      for i in 0..device_count {
        let device = get_device_collection_device(&device_collection, i)?;
        let device_id = get_device_property(&device, &PKEY_Device_InstanceId)?;
        let device_name = get_device_property(&device, &PKEY_DeviceInterface_FriendlyName)?;
        let device_volume_interface = get_device_volume_interface(&device)?;
        let device_volume = WinAudioDeviceVolume::new(device_volume_interface, device_name.clone())?;
        let audio_device = AudioDevice::new(device_id, device_name, device_volume);
        devices.push(audio_device);
      }

      Ok(devices)
    }
  }
}
