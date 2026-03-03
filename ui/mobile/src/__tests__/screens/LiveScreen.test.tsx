import React from 'react';
import { render } from '@testing-library/react-native';
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

describe('LiveScreen', () => {
  it('module exports LiveScreen component', () => {
    const mod = require('@/screens/LiveScreen');
    expect(mod.LiveScreen).toBeDefined();
    expect(typeof mod.LiveScreen).toBe('function');
  });

  it('default export is also available', () => {
    const mod = require('@/screens/LiveScreen');
    expect(mod.default).toBeDefined();
  });

  it('renders without crashing', () => {
    const { LiveScreen } = require('@/screens/LiveScreen');
    const { toJSON } = render(
      <ThemeProvider>
        <LiveScreen />
      </ThemeProvider>,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('renders loading state when not ready', () => {
    const { LiveScreen } = require('@/screens/LiveScreen');
    const { getByText } = render(
      <ThemeProvider>
        <LiveScreen />
      </ThemeProvider>,
    );
    // The screen shows "Loading live renderer" when not ready
    expect(getByText('Loading live renderer')).toBeTruthy();
  });
});
