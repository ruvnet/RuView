const expoPreset = require('jest-expo/jest-preset');

module.exports = {
  preset: 'jest-expo',
  setupFiles: [
    '<rootDir>/jest.setup.pre.js',
    ...(expoPreset.setupFiles || []),
  ],
  setupFilesAfterEnv: ['<rootDir>/jest.setup.ts'],
  testPathIgnorePatterns: ['/node_modules/', '/__mocks__/'],
  transformIgnorePatterns: [
    'node_modules/(?!(expo|expo-.+|react-native|@react-native|react-native-webview|react-native-reanimated|react-native-svg|react-native-safe-area-context|react-native-screens|@react-navigation|@expo|@unimodules|expo-modules-core|react-native-worklets)/)',
  ],
};
