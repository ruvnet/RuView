jest.mock('@react-native-async-storage/async-storage', () =>
  require('@react-native-async-storage/async-storage/jest/async-storage-mock')
);

jest.mock('react-native-wifi-reborn', () => ({
  loadWifiList: jest.fn(async () => []),
}));

jest.mock('react-native-reanimated', () =>
  require('react-native-reanimated/mock')
);

jest.mock('react-native-webview', () => {
  const React = require('react');
  const { View } = require('react-native');

  const MockWebView = (props: unknown) => React.createElement(View, props);

  return {
    __esModule: true,
    default: MockWebView,
    WebView: MockWebView,
  };
});
