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

// Mock the MatWebView which uses react-native-webview
jest.mock('@/screens/MATScreen/MatWebView', () => {
  const { View } = require('react-native');
  return {
    MatWebView: (props: any) => require('react').createElement(View, { testID: 'mat-webview', ...props }),
  };
});

// Mock the useMatBridge hook
jest.mock('@/screens/MATScreen/useMatBridge', () => ({
  useMatBridge: () => ({
    webViewRef: { current: null },
    ready: false,
    onMessage: jest.fn(),
    sendFrameUpdate: jest.fn(),
    postEvent: jest.fn(() => jest.fn()),
  }),
}));

describe('MATScreen', () => {
  it('module exports MATScreen component', () => {
    const mod = require('@/screens/MATScreen');
    expect(mod.MATScreen).toBeDefined();
    expect(typeof mod.MATScreen).toBe('function');
  });

  it('default export is also available', () => {
    const mod = require('@/screens/MATScreen');
    expect(mod.default).toBeDefined();
  });

  it('renders without crashing', () => {
    const { MATScreen } = require('@/screens/MATScreen');
    const { toJSON } = render(
      <ThemeProvider>
        <MATScreen />
      </ThemeProvider>,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('renders the connection banner', () => {
    const { MATScreen } = require('@/screens/MATScreen');
    const { getByText } = render(
      <ThemeProvider>
        <MATScreen />
      </ThemeProvider>,
    );
    // Simulated status maps to 'simulated' banner -> "SIMULATED DATA"
    expect(getByText('SIMULATED DATA')).toBeTruthy();
  });

  it('shows simulation warning overlay when simulated and not acknowledged', () => {
    // Reset store to ensure overlay is shown
    const { useMatStore } = require('@/stores/matStore');
    useMatStore.setState({ dataSource: 'simulated', simulationAcknowledged: false });

    const { MATScreen } = require('@/screens/MATScreen');
    const { getByText } = render(
      <ThemeProvider>
        <MATScreen />
      </ThemeProvider>,
    );
    expect(getByText('I UNDERSTAND')).toBeTruthy();
  });

  it('hides overlay after acknowledgment', () => {
    const { useMatStore } = require('@/stores/matStore');
    useMatStore.setState({ dataSource: 'simulated', simulationAcknowledged: true });

    const { MATScreen } = require('@/screens/MATScreen');
    const { queryByText } = render(
      <ThemeProvider>
        <MATScreen />
      </ThemeProvider>,
    );
    expect(queryByText('I UNDERSTAND')).toBeNull();
  });
});
