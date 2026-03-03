import { usePoseStore } from '@/stores/poseStore';
import type { SensingFrame } from '@/types/sensing';

const makeFrame = (overrides: Partial<SensingFrame> = {}): SensingFrame => ({
  type: 'sensing_update',
  timestamp: Date.now(),
  source: 'simulated',
  nodes: [{ node_id: 1, rssi_dbm: -45, position: [0, 0, 0] }],
  features: {
    mean_rssi: -45,
    variance: 1.5,
    motion_band_power: 0.1,
    breathing_band_power: 0.05,
    spectral_entropy: 0.8,
  },
  classification: {
    motion_level: 'present_still',
    presence: true,
    confidence: 0.85,
  },
  signal_field: {
    grid_size: [20, 1, 20],
    values: new Array(400).fill(0.5),
  },
  ...overrides,
});

describe('usePoseStore', () => {
  beforeEach(() => {
    usePoseStore.getState().reset();
  });

  describe('initial state', () => {
    it('has disconnected connectionStatus', () => {
      expect(usePoseStore.getState().connectionStatus).toBe('disconnected');
    });

    it('has isSimulated false', () => {
      expect(usePoseStore.getState().isSimulated).toBe(false);
    });

    it('has null lastFrame', () => {
      expect(usePoseStore.getState().lastFrame).toBeNull();
    });

    it('has empty rssiHistory', () => {
      expect(usePoseStore.getState().rssiHistory).toEqual([]);
    });

    it('has null features', () => {
      expect(usePoseStore.getState().features).toBeNull();
    });

    it('has null classification', () => {
      expect(usePoseStore.getState().classification).toBeNull();
    });

    it('has null signalField', () => {
      expect(usePoseStore.getState().signalField).toBeNull();
    });

    it('has zero messageCount', () => {
      expect(usePoseStore.getState().messageCount).toBe(0);
    });

    it('has null uptimeStart', () => {
      expect(usePoseStore.getState().uptimeStart).toBeNull();
    });
  });

  describe('handleFrame', () => {
    it('updates features from frame', () => {
      const frame = makeFrame();
      usePoseStore.getState().handleFrame(frame);
      expect(usePoseStore.getState().features).toEqual(frame.features);
    });

    it('updates classification from frame', () => {
      const frame = makeFrame();
      usePoseStore.getState().handleFrame(frame);
      expect(usePoseStore.getState().classification).toEqual(frame.classification);
    });

    it('updates signalField from frame', () => {
      const frame = makeFrame();
      usePoseStore.getState().handleFrame(frame);
      expect(usePoseStore.getState().signalField).toEqual(frame.signal_field);
    });

    it('increments messageCount', () => {
      usePoseStore.getState().handleFrame(makeFrame());
      usePoseStore.getState().handleFrame(makeFrame());
      usePoseStore.getState().handleFrame(makeFrame());
      expect(usePoseStore.getState().messageCount).toBe(3);
    });

    it('tracks RSSI history from mean_rssi', () => {
      usePoseStore.getState().handleFrame(
        makeFrame({ features: { mean_rssi: -40, variance: 1, motion_band_power: 0.1, breathing_band_power: 0.05, spectral_entropy: 0.8 } }),
      );
      usePoseStore.getState().handleFrame(
        makeFrame({ features: { mean_rssi: -50, variance: 1, motion_band_power: 0.1, breathing_band_power: 0.05, spectral_entropy: 0.8 } }),
      );
      const history = usePoseStore.getState().rssiHistory;
      expect(history).toEqual([-40, -50]);
    });

    it('sets uptimeStart on first frame only', () => {
      usePoseStore.getState().handleFrame(makeFrame());
      const firstUptime = usePoseStore.getState().uptimeStart;
      expect(firstUptime).not.toBeNull();

      usePoseStore.getState().handleFrame(makeFrame());
      expect(usePoseStore.getState().uptimeStart).toBe(firstUptime);
    });

    it('stores lastFrame', () => {
      const frame = makeFrame();
      usePoseStore.getState().handleFrame(frame);
      expect(usePoseStore.getState().lastFrame).toBe(frame);
    });
  });

  describe('setConnectionStatus', () => {
    it('updates connectionStatus', () => {
      usePoseStore.getState().setConnectionStatus('connected');
      expect(usePoseStore.getState().connectionStatus).toBe('connected');
    });

    it('sets isSimulated true for simulated status', () => {
      usePoseStore.getState().setConnectionStatus('simulated');
      expect(usePoseStore.getState().isSimulated).toBe(true);
    });

    it('sets isSimulated false for connected status', () => {
      usePoseStore.getState().setConnectionStatus('simulated');
      usePoseStore.getState().setConnectionStatus('connected');
      expect(usePoseStore.getState().isSimulated).toBe(false);
    });

    it('sets isSimulated false for disconnected status', () => {
      usePoseStore.getState().setConnectionStatus('simulated');
      usePoseStore.getState().setConnectionStatus('disconnected');
      expect(usePoseStore.getState().isSimulated).toBe(false);
    });
  });

  describe('reset', () => {
    it('clears everything back to initial state', () => {
      usePoseStore.getState().setConnectionStatus('connected');
      usePoseStore.getState().handleFrame(makeFrame());
      usePoseStore.getState().handleFrame(makeFrame());

      usePoseStore.getState().reset();

      const state = usePoseStore.getState();
      expect(state.connectionStatus).toBe('disconnected');
      expect(state.isSimulated).toBe(false);
      expect(state.lastFrame).toBeNull();
      expect(state.rssiHistory).toEqual([]);
      expect(state.features).toBeNull();
      expect(state.classification).toBeNull();
      expect(state.signalField).toBeNull();
      expect(state.messageCount).toBe(0);
      expect(state.uptimeStart).toBeNull();
    });
  });
});
