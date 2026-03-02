import { useCallback, useEffect, useRef, useState } from 'react';
import { Button, Platform, StyleSheet, View } from 'react-native';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { LoadingSpinner } from '@/components/LoadingSpinner';
import { ThemedText } from '@/components/ThemedText';
import { ThemedView } from '@/components/ThemedView';
import { usePoseStream } from '@/hooks/usePoseStream';
import { colors, spacing } from '@/theme';
import type { ConnectionStatus, SensingFrame } from '@/types/sensing';
import { LiveHUD } from './LiveHUD';

type LiveMode = 'LIVE' | 'SIM' | 'RSSI';

const getMode = (
  status: ConnectionStatus,
  isSimulated: boolean,
  frame: SensingFrame | null,
): LiveMode => {
  if (isSimulated || frame?.source === 'simulated') return 'SIM';
  if (status === 'connected') return 'LIVE';
  return 'RSSI';
};

const isWeb = Platform.OS === 'web';

type ViewerProps = {
  frame: SensingFrame | null;
  onReady: () => void;
  onFps: (fps: number) => void;
  onError: (msg: string) => void;
};

const WebLiveViewer = ({ frame, onReady, onFps, onError }: ViewerProps) => {
  const [Viewer, setViewer] = useState<React.ComponentType<any> | null>(null);

  useEffect(() => {
    import('./GaussianSplatWebView.web').then((mod) => {
      setViewer(() => mod.GaussianSplatWebViewWeb);
    }).catch(() => onError('Failed to load web viewer'));
  }, [onError]);

  if (!Viewer) return null;
  return <Viewer frame={frame} onReady={onReady} onFps={onFps} onError={onError} />;
};

const NativeLiveViewer = ({ frame, onReady, onFps, onError }: ViewerProps) => {
  const webViewRef = useRef(null);
  const [WVComponent, setWVComponent] = useState<React.ComponentType<any> | null>(null);

  useEffect(() => {
    try {
      const { GaussianSplatWebView } = require('./GaussianSplatWebView');
      setWVComponent(() => GaussianSplatWebView);
    } catch {
      onError('WebView not available on this platform');
    }
  }, [onError]);

  if (!WVComponent) return null;

  return (
    <WVComponent
      webViewRef={webViewRef}
      onMessage={(event: any) => {
        try {
          const data = typeof event.nativeEvent.data === 'string'
            ? JSON.parse(event.nativeEvent.data)
            : event.nativeEvent.data;
          if (data.type === 'READY') onReady();
          else if (data.type === 'FPS_TICK') onFps(data.payload?.fps ?? 0);
          else if (data.type === 'ERROR') onError(data.payload?.message ?? 'Unknown error');
        } catch { /* ignore */ }
      }}
      onError={() => onError('WebView renderer failed')}
    />
  );
};

export const LiveScreen = () => {
  const { lastFrame, connectionStatus, isSimulated } = usePoseStream();
  const [ready, setReady] = useState(false);
  const [fps, setFps] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [viewerKey, setViewerKey] = useState(0);

  const handleReady = useCallback(() => { setReady(true); setError(null); }, []);
  const handleFps = useCallback((f: number) => setFps(Math.max(0, Math.floor(f))), []);
  const handleError = useCallback((msg: string) => { setError(msg); setReady(false); }, []);
  const handleRetry = useCallback(() => { setError(null); setReady(false); setFps(0); setViewerKey((v) => v + 1); }, []);

  const rssi = lastFrame?.features?.mean_rssi;
  const personCount = lastFrame?.classification?.presence ? 1 : 0;
  const mode = getMode(connectionStatus, isSimulated, lastFrame);

  if (error) {
    return (
      <ThemedView style={styles.fallbackWrap}>
        <ThemedText preset="bodyLg">Live visualization failed</ThemedText>
        <ThemedText preset="bodySm" color="textSecondary" style={styles.errorText}>{error}</ThemedText>
        <Button title="Retry" onPress={handleRetry} />
      </ThemedView>
    );
  }

  return (
    <ErrorBoundary>
      <View style={styles.container}>
        {isWeb ? (
          <WebLiveViewer key={viewerKey} frame={lastFrame} onReady={handleReady} onFps={handleFps} onError={handleError} />
        ) : (
          <NativeLiveViewer key={viewerKey} frame={lastFrame} onReady={handleReady} onFps={handleFps} onError={handleError} />
        )}

        <LiveHUD
          connectionStatus={connectionStatus}
          fps={fps}
          rssi={rssi}
          confidence={lastFrame?.classification?.confidence ?? 0}
          personCount={personCount}
          mode={mode}
        />

        {!ready && (
          <View style={styles.loadingWrap}>
            <LoadingSpinner />
            <ThemedText preset="bodyMd" style={styles.loadingText}>Loading live renderer</ThemedText>
          </View>
        )}
      </View>
    </ErrorBoundary>
  );
};

export default LiveScreen;

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: colors.bg },
  loadingWrap: { ...StyleSheet.absoluteFillObject, backgroundColor: colors.bg, alignItems: 'center', justifyContent: 'center', gap: spacing.md },
  loadingText: { color: colors.textSecondary },
  fallbackWrap: { flex: 1, alignItems: 'center', justifyContent: 'center', gap: spacing.md, padding: spacing.lg },
  errorText: { textAlign: 'center' },
});
