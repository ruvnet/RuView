import { useSettingsStore } from '@/stores/settingsStore';

describe('useSettingsStore', () => {
  beforeEach(() => {
    // Reset to defaults by manually setting all values
    useSettingsStore.setState({
      serverUrl: 'http://localhost:3000',
      rssiScanEnabled: false,
      theme: 'system',
      alertSoundEnabled: true,
    });
  });

  describe('default values', () => {
    it('has default serverUrl as http://localhost:3000', () => {
      expect(useSettingsStore.getState().serverUrl).toBe('http://localhost:3000');
    });

    it('has rssiScanEnabled false by default', () => {
      expect(useSettingsStore.getState().rssiScanEnabled).toBe(false);
    });

    it('has theme as system by default', () => {
      expect(useSettingsStore.getState().theme).toBe('system');
    });

    it('has alertSoundEnabled true by default', () => {
      expect(useSettingsStore.getState().alertSoundEnabled).toBe(true);
    });
  });

  describe('setServerUrl', () => {
    it('updates the server URL', () => {
      useSettingsStore.getState().setServerUrl('http://10.0.0.1:8080');
      expect(useSettingsStore.getState().serverUrl).toBe('http://10.0.0.1:8080');
    });

    it('handles empty string', () => {
      useSettingsStore.getState().setServerUrl('');
      expect(useSettingsStore.getState().serverUrl).toBe('');
    });
  });

  describe('setRssiScanEnabled', () => {
    it('toggles to true', () => {
      useSettingsStore.getState().setRssiScanEnabled(true);
      expect(useSettingsStore.getState().rssiScanEnabled).toBe(true);
    });

    it('toggles back to false', () => {
      useSettingsStore.getState().setRssiScanEnabled(true);
      useSettingsStore.getState().setRssiScanEnabled(false);
      expect(useSettingsStore.getState().rssiScanEnabled).toBe(false);
    });
  });

  describe('setTheme', () => {
    it('sets theme to dark', () => {
      useSettingsStore.getState().setTheme('dark');
      expect(useSettingsStore.getState().theme).toBe('dark');
    });

    it('sets theme to light', () => {
      useSettingsStore.getState().setTheme('light');
      expect(useSettingsStore.getState().theme).toBe('light');
    });

    it('sets theme back to system', () => {
      useSettingsStore.getState().setTheme('dark');
      useSettingsStore.getState().setTheme('system');
      expect(useSettingsStore.getState().theme).toBe('system');
    });
  });

  describe('setAlertSoundEnabled', () => {
    it('disables alert sound', () => {
      useSettingsStore.getState().setAlertSoundEnabled(false);
      expect(useSettingsStore.getState().alertSoundEnabled).toBe(false);
    });

    it('re-enables alert sound', () => {
      useSettingsStore.getState().setAlertSoundEnabled(false);
      useSettingsStore.getState().setAlertSoundEnabled(true);
      expect(useSettingsStore.getState().alertSoundEnabled).toBe(true);
    });
  });
});
