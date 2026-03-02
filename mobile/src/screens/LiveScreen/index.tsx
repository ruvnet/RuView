import { useCallback, useEffect, useRef, useState } from 'react';
import { Button, LayoutChangeEvent, StyleSheet, View } from 'react-native';
import type { WebView } from 'react-native-webview';
import type { WebViewMessageEvent } from 'react-native-webview';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { LoadingSpinner } from '@/components/LoadingSpinner';
import { ThemedText } from '@/components/ThemedText';
import { ThemedView } from '@/components/ThemedView';
import { usePoseStream } from '@/hooks/usePoseStream';
import { colors, spacing } from '@/theme';
import type { ConnectionStatus, SensingFrame } from '@/types/sensing';
import { useGaussianBridge } from './useGaussianBridge';
import { GaussianSplatWebView } from './GaussianSplatWebView';
import { LiveHUD } from './LiveHUD';

type LiveMode = 'LIVE' | 'SIM' | 'RSSI';

const getMode = (
  status: ConnectionStatus,
  isSimulated: boolean,
  frame: SensingFrame | null,
): LiveMode => {
  if (isSimulated || frame?.source === 'simulated') {
    return 'SIM';
  }

  if (status === 'connected') {
    return 'LIVE';
  }

  return 'RSSI';
};

const dispatchWebViewMessage = (webViewRef: { current: WebView | null }, message: unknown) => {
  const webView = webViewRef.current;
  if (!webView) {
    return;
  }

  const payload = JSON.stringify(message);
  webView.injectJavaScript(
    `window.dispatchEvent(new MessageEvent('message', { data: ${JSON.stringify(payload)} })); true;`,
  );
};

export const LiveScreen = () => {
  const webViewRef = useRef<WebView | null>(null);
  const { lastFrame, connectionStatus, isSimulated } = usePoseStream();
  const bridge = useGaussianBridge(webViewRef);

  const [webError, setWebError] = useState<string | null>(null);
  const [viewerKey, setViewerKey] = useState(0);
  const sendTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingFrameRef = useRef<SensingFrame | null>(null);
  const lastSentAtRef = useRef(0);

  const clearSendTimeout = useCallback(() => {
    if (!sendTimeoutRef.current) {
      return;
    }
    clearTimeout(sendTimeoutRef.current);
    sendTimeoutRef.current = null;
  }, []);

  useEffect(() => {
    if (!lastFrame) {
      return;
    }

    pendingFrameRef.current = lastFrame;
    const now = Date.now();

    const flush = () => {
      if (!bridge.isReady || !pendingFrameRef.current) {
        return;
      }

      bridge.sendFrame(pendingFrameRef.current);
      lastSentAtRef.current = Date.now();
      pendingFrameRef.current = null;
    };

    const waitMs = Math.max(0, 500 - (now - lastSentAtRef.current));

    if (waitMs <= 0) {
      flush();
      return;
    }

    clearSendTimeout();
    sendTimeoutRef.current = setTimeout(() => {
      sendTimeoutRef.current = null;
      flush();
    }, waitMs);

    return () => {
      clearSendTimeout();
    };
  }, [bridge.isReady, lastFrame, bridge.sendFrame, clearSendTimeout]);

  useEffect(() => {
    return () => {
      dispatchWebViewMessage(webViewRef, { type: 'DISPOSE' });
      clearSendTimeout();
      pendingFrameRef.current = null;
    };
  }, [clearSendTimeout]);

  const onMessage = useCallback(
    (event: WebViewMessageEvent) => {
      bridge.onMessage(event);
    },
    [bridge],
  );

  const onLayout = useCallback((event: LayoutChangeEvent) => {
    const { width, height } = event.nativeEvent.layout;
    if (width <= 0 || height <= 0 || Number.isNaN(width) || Number.isNaN(height)) {
      return;
    }

    dispatchWebViewMessage(webViewRef, {
      type: 'RESIZE',
      payload: {
        width: Math.max(1, Math.floor(width)),
        height: Math.max(1, Math.floor(height)),
      },
    });
  }, []);

  const handleWebError = useCallback(() => {
    setWebError('Live renderer failed to initialize');
  }, []);

  const handleRetry = useCallback(() => {
    setWebError(null);
    bridge.reset();
    setViewerKey((value) => value + 1);
  }, [bridge]);

  const rssi = lastFrame?.features?.mean_rssi;
  const personCount = lastFrame?.classification?.presence ? 1 : 0;
  const mode = getMode(connectionStatus, isSimulated, lastFrame);

  if (webError || bridge.error) {
    return (
      <ThemedView style={styles.fallbackWrap}>
        <ThemedText preset="bodyLg">Live visualization failed</ThemedText>
        <ThemedText preset="bodySm" color="textSecondary" style={styles.errorText}>
          {webError ?? bridge.error}
        </ThemedText>
        <Button title="Retry" onPress={handleRetry} />
      </ThemedView>
    );
  }

  return (
    <ErrorBoundary>
      <View style={styles.container}>
        <GaussianSplatWebView
          key={viewerKey}
          webViewRef={webViewRef}
          onMessage={onMessage}
          onError={handleWebError}
          onLayout={onLayout}
        />

        <LiveHUD
          connectionStatus={connectionStatus}
          fps={bridge.fps}
          rssi={rssi}
          confidence={lastFrame?.classification?.confidence ?? 0}
          personCount={personCount}
          mode={mode}
        />

        {!bridge.isReady && (
          <View style={styles.loadingWrap}>
            <LoadingSpinner />
            <ThemedText preset="bodyMd" style={styles.loadingText}>
              Loading live renderer
            </ThemedText>
          </View>
        )}
      </View>
    </ErrorBoundary>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: colors.bg,
  },
  loadingWrap: {
    ...StyleSheet.absoluteFillObject,
    backgroundColor: colors.bg,
    alignItems: 'center',
    justifyContent: 'center',
    gap: spacing.md,
  },
  loadingText: {
    color: colors.textSecondary,
  },
  fallbackWrap: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    gap: spacing.md,
    padding: spacing.lg,
  },
  errorText: {
    textAlign: 'center',
  },
});
