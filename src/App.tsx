import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useState } from 'react';
import Slider from 'rc-slider';

interface DeviceInfo {
  id: string;
  name: string;
  max_volume: number;
}

async function fetchDevices(): Promise<DeviceInfo[]> {
  return invoke('get_devices');
}

async function fetchGlobalMaxVolume(): Promise<number> {
  return invoke('get_global_max_volume');
}

function DeviceInfo(props: { device: DeviceInfo, onChangeMaxVolume: (id: string, volumePercentage: number) => void }) {
  const { device, onChangeMaxVolume: onChangeVolume } = props;
  const volumePercentage = Math.floor(device.max_volume * 100);

  const handleMaxVolumeChange = (value: number | number[]) => {
    onChangeVolume(device.id, value as number);
  };

  return (
    <div className="device-info">
      <h2 className="device-name">{device.name}</h2>
      <div className="device-volume">
        <Slider
          min={1}
          max={100}
          step={1}
          className="device-volume-slider"
          value={volumePercentage}
          onChange={handleMaxVolumeChange}
        />
        <label className="device-volume-label">{volumePercentage}</label>
      </div>
      <input type="hidden" value={device.id} />
    </div>
  );
}

export default function App() {
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [globalMaxVolume, setGlobalMaxVolume] = useState<number>(1);

  const onChangeDeviceMaxVolume = useCallback(async (deviceId: string, volumePercentage: number) => {
    const volume = volumePercentage / 100;
    await invoke('set_device_max_volume', { deviceId, volume });
    setDevices(await fetchDevices());
  }, []);

  const onChangeGlobalMaxVolume = useCallback(async (_deviceId: string, volumePercentage: number) => {
    const volume = volumePercentage / 100;
    await invoke('set_global_max_volume', { volume });
    setGlobalMaxVolume(volume);
  }, []);

  useEffect(() => {
    fetchDevices().then(setDevices);
    fetchGlobalMaxVolume().then(setGlobalMaxVolume);
    listen<DeviceInfo[]>('devices-updated', event => setDevices(event.payload));
  }, []);

  return (
    <div className="content">
      <DeviceInfo device={{ id: "global", name: "Global Maximum Volume", max_volume: globalMaxVolume }} onChangeMaxVolume={onChangeGlobalMaxVolume} />
      <div className="divider" />
      {devices.map((device) => (
        <DeviceInfo key={device.id} device={device} onChangeMaxVolume={onChangeDeviceMaxVolume} />
      ))}
    </div>
  );
}
