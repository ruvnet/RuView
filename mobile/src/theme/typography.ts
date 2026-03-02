import { Platform } from 'react-native';

export const typography = {
  displayXl: { fontSize: 48, fontWeight: '700', letterSpacing: -1 },
  displayLg: { fontSize: 32, fontWeight: '700', letterSpacing: -0.5 },
  displayMd: { fontSize: 24, fontWeight: '600' },
  labelLg: {
    fontSize: 16,
    fontWeight: '600',
    letterSpacing: 0.5,
    textTransform: 'uppercase',
  },
  labelMd: {
    fontSize: 12,
    fontWeight: '600',
    letterSpacing: 1,
    textTransform: 'uppercase',
  },
  bodyLg: { fontSize: 16, fontWeight: '400' },
  bodyMd: { fontSize: 14, fontWeight: '400' },
  bodySm: { fontSize: 12, fontWeight: '400' },
  mono: {
    fontFamily: Platform.OS === 'ios' ? 'Courier New' : 'monospace',
    fontSize: 13,
  },
};
