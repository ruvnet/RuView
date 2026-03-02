import React, { PropsWithChildren } from 'react';
import { render, type RenderOptions } from '@testing-library/react-native';
import { NavigationContainer } from '@react-navigation/native';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { ThemeProvider } from '@/theme/ThemeContext';

type TestProvidersProps = PropsWithChildren<object>;

const TestProviders = ({ children }: TestProvidersProps) => (
  <GestureHandlerRootView style={{ flex: 1 }}>
    <SafeAreaProvider>
      <ThemeProvider>{children}</ThemeProvider>
    </SafeAreaProvider>
  </GestureHandlerRootView>
);

const TestProvidersWithNavigation = ({ children }: TestProvidersProps) => (
  <TestProviders>
    <NavigationContainer>{children}</NavigationContainer>
  </TestProviders>
);

interface RenderWithProvidersOptions extends Omit<RenderOptions, 'wrapper'> {
  withNavigation?: boolean;
}

export const renderWithProviders = (
  ui: React.ReactElement,
  { withNavigation, ...options }: RenderWithProvidersOptions = {},
) => {
  return render(ui, {
    ...options,
    wrapper: withNavigation ? TestProvidersWithNavigation : TestProviders,
  });
};
