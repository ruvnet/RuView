import { generateSimulatedData } from '@/services/simulation.service';

describe('generateSimulatedData', () => {
  it('returns a valid SensingFrame shape', () => {
    const frame = generateSimulatedData();
    expect(frame).toHaveProperty('type', 'sensing_update');
    expect(frame).toHaveProperty('timestamp');
    expect(frame).toHaveProperty('source', 'simulated');
    expect(typeof frame.tick).toBe('number');
  });

  it('has a nodes array with at least one node', () => {
    const frame = generateSimulatedData();
    expect(Array.isArray(frame.nodes)).toBe(true);
    expect(frame.nodes.length).toBeGreaterThanOrEqual(1);

    const node = frame.nodes[0];
    expect(typeof node.node_id).toBe('number');
    expect(typeof node.rssi_dbm).toBe('number');
    expect(Array.isArray(node.position)).toBe(true);
    expect(node.position).toHaveLength(3);
  });

  it('has features object with expected numeric fields', () => {
    const frame = generateSimulatedData();
    const { features } = frame;
    expect(typeof features.mean_rssi).toBe('number');
    expect(typeof features.variance).toBe('number');
    expect(typeof features.motion_band_power).toBe('number');
    expect(typeof features.breathing_band_power).toBe('number');
    expect(typeof features.spectral_entropy).toBe('number');
    expect(typeof features.std).toBe('number');
    expect(typeof features.dominant_freq_hz).toBe('number');
  });

  it('has classification with valid motion_level', () => {
    const frame = generateSimulatedData();
    const { classification } = frame;
    expect(['absent', 'present_still', 'active']).toContain(classification.motion_level);
    expect(typeof classification.presence).toBe('boolean');
    expect(typeof classification.confidence).toBe('number');
    expect(classification.confidence).toBeGreaterThanOrEqual(0);
    expect(classification.confidence).toBeLessThanOrEqual(1);
  });

  it('has signal_field with correct grid_size', () => {
    const frame = generateSimulatedData();
    const { signal_field } = frame;
    expect(signal_field.grid_size).toEqual([20, 1, 20]);
    expect(Array.isArray(signal_field.values)).toBe(true);
    expect(signal_field.values.length).toBe(20 * 20);
  });

  it('has signal_field values clamped between 0 and 1', () => {
    const frame = generateSimulatedData();
    for (const v of frame.signal_field.values) {
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThanOrEqual(1);
    }
  });

  it('has vital_signs present', () => {
    const frame = generateSimulatedData();
    expect(frame.vital_signs).toBeDefined();
    expect(typeof frame.vital_signs!.breathing_bpm).toBe('number');
    expect(typeof frame.vital_signs!.hr_proxy_bpm).toBe('number');
    expect(typeof frame.vital_signs!.confidence).toBe('number');
  });

  it('has estimated_persons field', () => {
    const frame = generateSimulatedData();
    expect(typeof frame.estimated_persons).toBe('number');
    expect(frame.estimated_persons).toBeGreaterThanOrEqual(0);
  });

  it('produces different data for different timestamps', () => {
    const frame1 = generateSimulatedData(1000);
    const frame2 = generateSimulatedData(5000);
    // The RSSI values should differ since the simulation is time-based
    expect(frame1.features.mean_rssi).not.toBe(frame2.features.mean_rssi);
  });

  it('accepts a custom timeMs parameter', () => {
    const t = 1700000000000;
    const frame = generateSimulatedData(t);
    expect(frame.timestamp).toBe(t);
  });
});
