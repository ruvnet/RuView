import React from 'react';
import { render, screen } from '@testing-library/react-native';
import { ThemeProvider } from '@/theme/ThemeContext';

jest.mock('@/hooks/usePoseStream', () => ({
  usePoseStream: () => ({
    connectionStatus: 'simulated' as const,
    lastFrame: null,
    isSimulated: true,
  }),
}));

jest.mock('react-native-svg', () => {
  const { View } = require('react-native');
  return {
    __esModule: true,
    default: View,
    Svg: View,
    Circle: View,
    G: View,
    Text: View,
    Rect: View,
    Line: View,
    Path: View,
  };
});

describe('VitalsScreen', () => {
  it('module exports VitalsScreen as default', () => {
    const mod = require('@/screens/VitalsScreen');
    expect(mod.default).toBeDefined();
    expect(typeof mod.default).toBe('function');
  });

  it('renders without crashing', () => {
    const VitalsScreen = require('@/screens/VitalsScreen').default;
    const { toJSON } = render(
      <ThemeProvider>
        <VitalsScreen />
      </ThemeProvider>,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('renders the RSSI HISTORY section', () => {
    const VitalsScreen = require('@/screens/VitalsScreen').default;
    render(
      <ThemeProvider>
        <VitalsScreen />
      </ThemeProvider>,
    );
    expect(screen.getByText('RSSI HISTORY')).toBeTruthy();
  });

  it('renders the classification label', () => {
    const VitalsScreen = require('@/screens/VitalsScreen').default;
    render(
      <ThemeProvider>
        <VitalsScreen />
      </ThemeProvider>,
    );
    // With no data, classification defaults to 'ABSENT'
    expect(screen.getByText('Classification: ABSENT')).toBeTruthy();
  });

  it('renders the connection banner', () => {
    const VitalsScreen = require('@/screens/VitalsScreen').default;
    render(
      <ThemeProvider>
        <VitalsScreen />
      </ThemeProvider>,
    );
    expect(screen.getByText('SIMULATED DATA')).toBeTruthy();
  });
});
