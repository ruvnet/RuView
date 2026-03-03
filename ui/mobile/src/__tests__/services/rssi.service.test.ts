// In the Jest environment (jsdom/node), Platform.OS defaults to a value that
// causes rssi.service.ts to load the web implementation. We test the web
// version which provides synthetic data.

jest.mock('react-native', () => {
  const RN = jest.requireActual('react-native');
  return {
    ...RN,
    Platform: { ...RN.Platform, OS: 'web' },
  };
});

describe('RssiService (web)', () => {
  let rssiService: any;

  beforeEach(() => {
    jest.useFakeTimers();
    jest.isolateModules(() => {
      rssiService = require('@/services/rssi.service').rssiService;
    });
  });

  afterEach(() => {
    rssiService?.stopScanning();
    jest.useRealTimers();
  });

  describe('subscribe / unsubscribe', () => {
    it('subscribe returns an unsubscribe function', () => {
      const listener = jest.fn();
      const unsub = rssiService.subscribe(listener);
      expect(typeof unsub).toBe('function');
      unsub();
    });

    it('listener is not called without scanning', () => {
      const listener = jest.fn();
      rssiService.subscribe(listener);
      jest.advanceTimersByTime(5000);
      // Without startScanning, the listener should not be called
      // (unless the service sends an initial broadcast, which web does on start)
      expect(listener).not.toHaveBeenCalled();
    });
  });

  describe('startScanning / stopScanning', () => {
    it('startScanning delivers network data to subscribers', () => {
      const listener = jest.fn();
      rssiService.subscribe(listener);
      rssiService.startScanning(1000);

      // The web service immediately broadcasts once and sets up interval
      expect(listener).toHaveBeenCalled();
      const networks = listener.mock.calls[0][0];
      expect(Array.isArray(networks)).toBe(true);
      expect(networks.length).toBeGreaterThan(0);
      expect(networks[0]).toHaveProperty('ssid');
      expect(networks[0]).toHaveProperty('level');
    });

    it('stopScanning stops delivering data', () => {
      const listener = jest.fn();
      rssiService.subscribe(listener);
      rssiService.startScanning(1000);
      const callCount = listener.mock.calls.length;

      rssiService.stopScanning();
      jest.advanceTimersByTime(5000);

      // No new calls after stopping
      expect(listener.mock.calls.length).toBe(callCount);
    });

    it('unsubscribed listener does not receive scan results', () => {
      const listener = jest.fn();
      const unsub = rssiService.subscribe(listener);
      unsub();

      rssiService.startScanning(1000);
      jest.advanceTimersByTime(3000);

      expect(listener).not.toHaveBeenCalled();
    });
  });

  describe('getLatestScan equivalent behavior', () => {
    it('returns empty networks initially when no scan has run', () => {
      // The web rssi service does not have a getLatestScan method,
      // but we verify that without scanning no data is emitted.
      const listener = jest.fn();
      rssiService.subscribe(listener);
      // No startScanning called
      expect(listener).not.toHaveBeenCalled();
    });
  });
});
