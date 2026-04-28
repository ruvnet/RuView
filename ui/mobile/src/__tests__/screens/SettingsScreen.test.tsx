import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react-native';
import { ThemeProvider } from '@/theme/ThemeContext';
import { apiService } from '@/services/api.service';
import { useSettingsStore } from '@/stores/settingsStore';

jest.mock('@/services/ws.service', () => ({
  wsService: {
    connect: jest.fn(),
    disconnect: jest.fn(),
    subscribe: jest.fn(() => jest.fn()),
    getStatus: jest.fn(() => 'disconnected'),
  },
}));

jest.mock('@/services/api.service', () => ({
  apiService: {
    setBaseUrl: jest.fn(),
    get: jest.fn(),
    post: jest.fn(),
    getStatus: jest.fn(),
  },
}));

describe('SettingsScreen', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    useSettingsStore.setState({
      serverUrl: 'http://localhost:3000',
      rssiScanEnabled: false,
      theme: 'system',
      alertSoundEnabled: true,
    });
  });

  it('module exports SettingsScreen component', () => {
    const mod = require('@/screens/SettingsScreen');
    expect(mod.SettingsScreen).toBeDefined();
    expect(typeof mod.SettingsScreen).toBe('function');
  });

  it('default export is also available', () => {
    const mod = require('@/screens/SettingsScreen');
    expect(mod.default).toBeDefined();
  });

  it('renders without crashing', () => {
    const { SettingsScreen } = require('@/screens/SettingsScreen');
    const { toJSON } = render(
      <ThemeProvider>
        <SettingsScreen />
      </ThemeProvider>,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('renders the SERVER section', () => {
    const { SettingsScreen } = require('@/screens/SettingsScreen');
    render(
      <ThemeProvider>
        <SettingsScreen />
      </ThemeProvider>,
    );
    expect(screen.getByText('SERVER')).toBeTruthy();
  });

  it('renders the SENSING section', () => {
    const { SettingsScreen } = require('@/screens/SettingsScreen');
    render(
      <ThemeProvider>
        <SettingsScreen />
      </ThemeProvider>,
    );
    expect(screen.getByText('SENSING')).toBeTruthy();
  });

  it('renders the ABOUT section with version', () => {
    const { SettingsScreen } = require('@/screens/SettingsScreen');
    render(
      <ThemeProvider>
        <SettingsScreen />
      </ThemeProvider>,
    );
    expect(screen.getByText('ABOUT')).toBeTruthy();
    expect(screen.getByText('WiFi-DensePose Mobile v1.0.0')).toBeTruthy();
  });

  it('tests the current draft URL before it is saved', async () => {
    const { SettingsScreen } = require('@/screens/SettingsScreen');
    (apiService.getStatus as jest.Mock).mockResolvedValue({ ok: true });

    render(
      <ThemeProvider>
        <SettingsScreen />
      </ThemeProvider>,
    );

    fireEvent.changeText(
      screen.getByPlaceholderText('http://192.168.1.100:8080'),
      'http://10.0.0.42:9090',
    );
    fireEvent.press(screen.getByText('Test Connection'));

    await waitFor(() => {
      expect(apiService.getStatus).toHaveBeenCalledWith('http://10.0.0.42:9090');
    });
  });
});
