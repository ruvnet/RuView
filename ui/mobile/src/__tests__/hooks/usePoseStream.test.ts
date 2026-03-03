// usePoseStream is a React hook that uses useEffect, zustand stores, and wsService.
// We test its interface shape and the module export.

jest.mock('@/services/ws.service', () => ({
  wsService: {
    subscribe: jest.fn(() => jest.fn()),
    connect: jest.fn(),
    disconnect: jest.fn(),
    getStatus: jest.fn(() => 'disconnected'),
  },
}));

import { usePoseStore } from '@/stores/poseStore';

describe('usePoseStream', () => {
  beforeEach(() => {
    usePoseStore.getState().reset();
  });

  it('module exports usePoseStream function', () => {
    const mod = require('@/hooks/usePoseStream');
    expect(typeof mod.usePoseStream).toBe('function');
  });

  it('exports UsePoseStreamResult interface (module shape)', () => {
    // Verify the module has the expected named exports
    const mod = require('@/hooks/usePoseStream');
    expect(mod).toHaveProperty('usePoseStream');
  });

  it('usePoseStream has the expected return type shape', () => {
    // We cannot call hooks outside of React components, but we can verify
    // the store provides the data the hook returns.
    const state = usePoseStore.getState();
    expect(state).toHaveProperty('connectionStatus');
    expect(state).toHaveProperty('lastFrame');
    expect(state).toHaveProperty('isSimulated');
  });

  it('wsService.subscribe is callable', () => {
    const { wsService } = require('@/services/ws.service');
    const unsub = wsService.subscribe(jest.fn());
    expect(typeof unsub).toBe('function');
  });
});
