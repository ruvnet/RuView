// Opt-in cross-repository prototype. Imports no hardware, network or model client.
export function createVirtualViewAdapter({ broker, privateKey, enabled = false }) {
  if (typeof broker !== 'function' || typeof enabled !== 'boolean') throw new TypeError('adapter configuration');
  return Object.freeze({
    observe(snapshot) {
      if (!enabled) return { decision: 'BLOCK', reason: 'disabled', authority: 'none', svg: null };
      // Explicit metric scene contract only. RF Gaussians are not manipulation geometry.
      if (snapshot?.schema !== 'virtual-view.v1' || snapshot?.frame !== 'room_enu') {
        throw new TypeError('calibrated metric scene snapshot required; raw RF/LiDAR packets are not supported');
      }
      return broker(snapshot, { privateKey });
    },
    hardwareActuation: false,
  });
}
