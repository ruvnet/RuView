import React from 'react';
import { render, screen } from '@testing-library/react-native';
import { SignalBar } from '@/components/SignalBar';
import { ThemeProvider } from '@/theme/ThemeContext';

const renderWithTheme = (ui: React.ReactElement) =>
  render(<ThemeProvider>{ui}</ThemeProvider>);

describe('SignalBar', () => {
  it('renders the label text', () => {
    renderWithTheme(<SignalBar value={0.5} label="Signal Strength" />);
    expect(screen.getByText('Signal Strength')).toBeTruthy();
  });

  it('renders the percentage text', () => {
    renderWithTheme(<SignalBar value={0.75} label="Test" />);
    expect(screen.getByText('75%')).toBeTruthy();
  });

  it('clamps value at 0 for negative input', () => {
    renderWithTheme(<SignalBar value={-0.5} label="Low" />);
    expect(screen.getByText('0%')).toBeTruthy();
  });

  it('clamps value at 100 for input above 1', () => {
    renderWithTheme(<SignalBar value={1.5} label="High" />);
    expect(screen.getByText('100%')).toBeTruthy();
  });

  it('renders without crashing with custom color', () => {
    const { toJSON } = renderWithTheme(
      <SignalBar value={0.5} label="Custom" color="#FF0000" />,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('renders 0% for zero value', () => {
    renderWithTheme(<SignalBar value={0} label="Zero" />);
    expect(screen.getByText('0%')).toBeTruthy();
  });

  it('renders 100% for value of 1', () => {
    renderWithTheme(<SignalBar value={1} label="Full" />);
    expect(screen.getByText('100%')).toBeTruthy();
  });
});
