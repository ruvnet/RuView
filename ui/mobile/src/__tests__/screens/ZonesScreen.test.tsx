import React from 'react';
import { render, screen } from '@testing-library/react-native';
import { ThemeProvider } from '@/theme/ThemeContext';
import { usePoseStore } from '@/stores/poseStore';

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

// Mock the subcomponents that may have heavy dependencies
jest.mock('@/screens/ZonesScreen/FloorPlanSvg', () => {
  const { View } = require('react-native');
  return {
    FloorPlanSvg: (props: any) => require('react').createElement(View, { testID: 'floor-plan', ...props }),
  };
});

jest.mock('@/screens/ZonesScreen/ZoneLegend', () => {
  const { View } = require('react-native');
  return {
    ZoneLegend: () => require('react').createElement(View, { testID: 'zone-legend' }),
  };
});

jest.mock('@/screens/ZonesScreen/useOccupancyGrid', () => ({
  useOccupancyGrid: () => ({
    gridValues: new Array(400).fill(0),
    personPositions: [],
  }),
}));

describe('ZonesScreen', () => {
  beforeEach(() => {
    usePoseStore.getState().reset();
  });

  it('module exports ZonesScreen component', () => {
    const mod = require('@/screens/ZonesScreen');
    expect(mod.ZonesScreen).toBeDefined();
    expect(typeof mod.ZonesScreen).toBe('function');
  });

  it('default export is also available', () => {
    const mod = require('@/screens/ZonesScreen');
    expect(mod.default).toBeDefined();
  });

  it('renders without crashing', () => {
    const { ZonesScreen } = require('@/screens/ZonesScreen');
    const { toJSON } = render(
      <ThemeProvider>
        <ZonesScreen />
      </ThemeProvider>,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('renders the floor plan heading', () => {
    const { ZonesScreen } = require('@/screens/ZonesScreen');
    render(
      <ThemeProvider>
        <ZonesScreen />
      </ThemeProvider>,
    );
    expect(screen.getByText(/Floor Plan/)).toBeTruthy();
  });

  it('renders occupancy count', () => {
    const { ZonesScreen } = require('@/screens/ZonesScreen');
    render(
      <ThemeProvider>
        <ZonesScreen />
      </ThemeProvider>,
    );
    expect(screen.getByText(/0 persons detected/)).toBeTruthy();
  });

  it('renders last update text', () => {
    const { ZonesScreen } = require('@/screens/ZonesScreen');
    render(
      <ThemeProvider>
        <ZonesScreen />
      </ThemeProvider>,
    );
    expect(screen.getByText(/Last update: N\/A/)).toBeTruthy();
  });
});
