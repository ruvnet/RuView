import { LayoutChangeEvent, StyleSheet } from 'react-native';
import type { RefObject } from 'react';
import { WebView, type WebViewMessageEvent } from 'react-native-webview';
import GAUSSIAN_SPLATS_HTML from '@/assets/webview/gaussian-splats.html';

type GaussianSplatWebViewProps = {
  onMessage: (event: WebViewMessageEvent) => void;
  onError: () => void;
  webViewRef: RefObject<WebView | null>;
  onLayout?: (event: LayoutChangeEvent) => void;
};

export const GaussianSplatWebView = ({
  onMessage,
  onError,
  webViewRef,
  onLayout,
}: GaussianSplatWebViewProps) => {
  const html = typeof GAUSSIAN_SPLATS_HTML === 'string' ? GAUSSIAN_SPLATS_HTML : '';

  return (
    <WebView
      ref={webViewRef}
      source={{ html }}
      originWhitelist={['*']}
      allowFileAccess={false}
      javaScriptEnabled
      onMessage={onMessage}
      onError={onError}
      onLayout={onLayout}
      style={styles.webView}
    />
  );
};

const styles = StyleSheet.create({
  webView: {
    flex: 1,
    backgroundColor: '#0A0E1A',
  },
});
