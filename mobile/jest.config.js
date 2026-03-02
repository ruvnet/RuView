module.exports = {
  preset: 'jest-expo',
  setupFilesAfterEnv: ['<rootDir>/jest.setup.ts'],
  testPathIgnorePatterns: ['/src/__tests__/'],
  transformIgnorePatterns: [
    'node_modules/(?!(expo|expo-.+|react-native|@react-native|react-native-webview|react-native-reanimated|react-native-svg|react-native-safe-area-context|react-native-screens|@react-navigation|@expo|@unimodules|expo-modules-core)/)',
  ],
};
