import { create } from 'zustand';
import { RingBuffer } from '@/utils/ringBuffer';
import type { Classification, ConnectionStatus, FeatureSet, SensingFrame, SignalField } from '@/types/sensing';

export interface PoseState {
  connectionStatus: ConnectionStatus;
  isSimulated: boolean;
  lastFrame: SensingFrame | null;
  rssiHistory: number[];
  features: FeatureSet | null;
  classification: Classification | null;
  signalField: SignalField | null;
  messageCount: number;
  uptimeStart: number | null;
  handleFrame: (frame: SensingFrame) => void;
  setConnectionStatus: (status: ConnectionStatus) => void;
  reset: () => void;
}

const MAX_RSSI_HISTORY = 60;
const rssiHistory = new RingBuffer<number>(MAX_RSSI_HISTORY, (a, b) => a - b);

export const usePoseStore = create<PoseState>((set) => ({
  connectionStatus: 'disconnected',
  isSimulated: false,
  lastFrame: null,
  rssiHistory: [],
  features: null,
  classification: null,
  signalField: null,
  messageCount: 0,
  uptimeStart: null,

  handleFrame: (frame: SensingFrame) => {
    if (typeof frame.features?.mean_rssi === 'number') {
      rssiHistory.push(frame.features.mean_rssi);
    }

    set((state) => ({
      lastFrame: frame,
      features: frame.features,
      classification: frame.classification,
      signalField: frame.signal_field,
      messageCount: state.messageCount + 1,
      uptimeStart: state.uptimeStart ?? Date.now(),
      rssiHistory: rssiHistory.toArray(),
    }));
  },

  setConnectionStatus: (status: ConnectionStatus) => {
    set({
      connectionStatus: status,
      isSimulated: status === 'simulated',
    });
  },

  reset: () => {
    rssiHistory.clear();
    set({
      connectionStatus: 'disconnected',
      isSimulated: false,
      lastFrame: null,
      rssiHistory: [],
      features: null,
      classification: null,
      signalField: null,
      messageCount: 0,
      uptimeStart: null,
    });
  },
}));
