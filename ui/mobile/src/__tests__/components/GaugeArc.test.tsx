import React from 'react';
import { render } from '@testing-library/react-native';
import { ThemeProvider } from '@/theme/ThemeContext';

jest.mock('react-native-svg', () => {
  const { View } = require('react-native');
  return {
    __esModule: true,
    default: View, // Svg
    Svg: View,
    Circle: View,
    G: View,
    Text: View,
    Rect: View,
    Line: View,
    Path: View,
  };
});

// GaugeArc uses Animated.createAnimatedComponent(Circle), so we need
// the reanimated mock (already in jest.setup.ts) and SVG mock above.
import { GaugeArc } from '@/components/GaugeArc';

const renderWithTheme = (ui: React.ReactElement) =>
  render(<ThemeProvider>{ui}</ThemeProvider>);

describe('GaugeArc', () => {
  it('renders without crashing', () => {
    const { toJSON } = renderWithTheme(
      <GaugeArc value={50} max={100} label="BPM" unit="bpm" color="#00FF00" />,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('renders with min and max values', () => {
    const { toJSON } = renderWithTheme(
      <GaugeArc value={0} min={0} max={200} label="Test" unit="x" color="#FF0000" />,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('renders with colorTo gradient', () => {
    const { toJSON } = renderWithTheme(
      <GaugeArc
        value={75}
        max={100}
        label="HR"
        unit="bpm"
        color="#00FF00"
        colorTo="#FF0000"
        size={200}
      />,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('renders with custom size', () => {
    const { toJSON } = renderWithTheme(
      <GaugeArc value={30} max={60} label="BR" unit="brpm" color="#0088FF" size={80} />,
    );
    expect(toJSON()).not.toBeNull();
  });
});
