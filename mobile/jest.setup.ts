jest.mock('@react-native-async-storage/async-storage', () =>
  require('@react-native-async-storage/async-storage/jest/async-storage-mock')
);

jest.mock('react-native-wifi-reborn', () => ({
  loadWifiList: jest.fn(async () => []),
}));

jest.mock('react-native-reanimated', () =>
  require('react-native-reanimated/mock')
);
