import { createRuView } from '../../src/sdk.js';
const ruview = createRuView();
ruview.devices.mmwave({ source: 'esphome', host: '127.0.0.1', api_port: 6053, seconds: 1 });
ruview.devices.mmwave({ port: 'COM3', model: 'mr60bha2' });
ruview.devices.mmwave({ source: 'serial', port: 'COM3', model: 'ld2410' });
ruview.devices.esp32({ spectrum: true, spectrum_bins: 8, spectrum_frames: 8, udp_port: 5005 });
ruview.devices.lidar({ source: 'iphone', url: 'ws://127.0.0.1:8787/ws/lidar', max_frames: 1 });
ruview.devices.lidar({ source: 'rplidar', port: 'COM3' });
// @ts-expect-error an ESPHome device still needs its host
ruview.devices.mmwave({ source: 'esphome' });
// @ts-expect-error default serial access still needs its port
ruview.devices.mmwave({ model: 'mr60bha2' });
// @ts-expect-error numeric capture option remains numeric
ruview.devices.esp32({ spectrum_frames: 'eight' });
// @ts-expect-error iPhone packet cap remains numeric
ruview.devices.lidar({ source: 'iphone', url: 'ws://127.0.0.1:8787/ws/lidar', max_frames: 'one' });
// @ts-expect-error known backends remain constrained
ruview.devices.esp32({ backend: 'gpu' });
