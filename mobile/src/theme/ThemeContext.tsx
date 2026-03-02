import React, { PropsWithChildren, useEffect, useState } from 'react';
import { useColorScheme } from 'react-native';
import { colors } from './colors';
import { spacing } from './spacing';
import { typography } from './typography';

export type ThemeMode = 'light' | 'dark' | 'system';

export type ThemeContextValue = {
  colors: typeof colors;
  typography: typeof typography;
  spacing: typeof spacing;
  isDark: boolean;
};

const fallbackThemeValue: ThemeContextValue = {
  colors,
  typography,
  spacing,
  isDark: true,
};

export const ThemeContext = React.createContext<ThemeContextValue>(fallbackThemeValue);

const isValidThemeMode = (value: unknown): value is ThemeMode => {
  return value === 'light' || value === 'dark' || value === 'system';
};

const readThemeFromSettings = async (): Promise<ThemeMode> => {
  try {
    const settingsStore = (await import('../stores/settingsStore')) as Record<string, unknown>;
    const stateAccessors = [settingsStore.useSettingsStore, (settingsStore as { useStore?: unknown }).useStore].filter(
      (candidate): candidate is { getState: () => { theme?: unknown } } =>
        typeof candidate === 'function' &&
        typeof (candidate as { getState?: unknown }).getState === 'function',
    );

    for (const accessor of stateAccessors) {
      const state = accessor.getState?.() as { theme?: unknown } | undefined;
      const candidateTheme = state?.theme;
      if (isValidThemeMode(candidateTheme)) {
        return candidateTheme;
      }
    }
  } catch {
    // No-op if store is unavailable during bootstrap.
  }

  return 'system';
};

export const ThemeProvider = ({ children }: PropsWithChildren<object>) => {
  const [themeMode, setThemeMode] = useState<ThemeMode>('system');
  const systemScheme = useColorScheme() ?? 'light';

  useEffect(() => {
    void readThemeFromSettings().then(setThemeMode);
  }, []);

  const isDark = themeMode === 'dark' || (themeMode === 'system' && systemScheme === 'dark');

  return (
    <ThemeContext.Provider
      value={{
        colors,
        typography,
        spacing,
        isDark,
      }}
    >
      {children}
    </ThemeContext.Provider>
  );
};
