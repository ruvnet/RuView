import React from 'react';
import { render } from '@testing-library/react-native';
import { SparklineChart } from '@/components/SparklineChart';
import { ThemeProvider } from '@/theme/ThemeContext';

const renderWithTheme = (ui: React.ReactElement) =>
  render(<ThemeProvider>{ui}</ThemeProvider>);

describe('SparklineChart', () => {
  it('renders without crashing with data points', () => {
    const { toJSON } = renderWithTheme(
      <SparklineChart data={[-50, -45, -48, -42, -47]} />,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('renders with empty data array', () => {
    const { toJSON } = renderWithTheme(<SparklineChart data={[]} />);
    expect(toJSON()).not.toBeNull();
  });

  it('renders with single data point', () => {
    const { toJSON } = renderWithTheme(<SparklineChart data={[42]} />);
    expect(toJSON()).not.toBeNull();
  });

  it('renders with custom color', () => {
    const { toJSON } = renderWithTheme(
      <SparklineChart data={[1, 2, 3]} color="#FF0000" />,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('renders with custom height', () => {
    const { toJSON } = renderWithTheme(
      <SparklineChart data={[1, 2, 3]} height={100} />,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('has an image accessibility role', () => {
    const { getByRole } = renderWithTheme(
      <SparklineChart data={[1, 2, 3]} />,
    );
    expect(getByRole('image')).toBeTruthy();
  });

  it('renders with all identical values', () => {
    const { toJSON } = renderWithTheme(
      <SparklineChart data={[5, 5, 5, 5, 5]} />,
    );
    expect(toJSON()).not.toBeNull();
  });
});
