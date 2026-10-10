// Opt-in cross-repository prototype. Imports no hardware, network or model client.
import { types } from 'node:util';

const snapshotError = () => new TypeError(
  'calibrated metric scene snapshot required; raw RF/LiDAR packets and non-JSON data are not supported',
);

// Structural preflight only, matching the reviewed companion's JSON budgets.
// Never invoke input methods, getters or proxy traps while inspecting data.
function validateSnapshotData(value, depth = 0, budget = { nodes: 0 }) {
  if (depth > 12 || ++budget.nodes > 20000) throw snapshotError();
  if (value === null || typeof value === 'boolean') return;
  if (typeof value === 'string' && value.length <= 4096) return;
  if (typeof value === 'number' && Number.isFinite(value)) return;
  if (typeof value !== 'object' || types.isProxy(value)) throw snapshotError();

  const array = Array.isArray(value);
  if (Object.getPrototypeOf(value) !== (array ? Array.prototype : Object.prototype)) {
    throw snapshotError();
  }
  if (array) {
    const length = Object.getOwnPropertyDescriptor(value, 'length').value;
    if (length > 1024) throw snapshotError();
    if (Reflect.ownKeys(value).length !== length + 1) throw snapshotError();
    for (let index = 0; index < length; index++) {
      const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
      if (!descriptor?.enumerable || !Object.hasOwn(descriptor, 'value')) throw snapshotError();
      validateSnapshotData(descriptor.value, depth + 1, budget);
    }
    return;
  }
  const keys = Reflect.ownKeys(value);
  if (keys.length > 64) throw snapshotError();
  for (const key of keys) {
    if (typeof key !== 'string' || key.length > 128) throw snapshotError();
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor.enumerable || !Object.hasOwn(descriptor, 'value')) throw snapshotError();
    validateSnapshotData(descriptor.value, depth + 1, budget);
  }
}

export function createVirtualViewAdapter({ broker, privateKey, enabled = false }) {
  if (typeof broker !== 'function' || typeof enabled !== 'boolean') throw new TypeError('adapter configuration');
  return Object.freeze({
    observe(snapshot) {
      if (!enabled) return { decision: 'BLOCK', reason: 'disabled', authority: 'none', svg: null };
      validateSnapshotData(snapshot);
      // Explicit metric scene contract only. RF Gaussians are not manipulation geometry.
      const schema = snapshot && Object.getOwnPropertyDescriptor(snapshot, 'schema');
      const frame = snapshot && Object.getOwnPropertyDescriptor(snapshot, 'frame');
      if (schema?.value !== 'virtual-view.v1' || frame?.value !== 'room_enu') {
        throw snapshotError();
      }
      return broker(snapshot, { privateKey });
    },
    hardwareActuation: false,
  });
}
