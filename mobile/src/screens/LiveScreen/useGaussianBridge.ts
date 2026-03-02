import { useCallback, useState } from 'react';
import type { RefObject } from 'react';
import type { WebViewMessageEvent } from 'react-native-webview';
import { WebView } from 'react-native-webview';
import type { SensingFrame } from '@/types/sensing';

export type GaussianBridgeMessageType = 'READY' | 'FPS_TICK' | 'ERROR';

type BridgeMessage = {
  type: GaussianBridgeMessageType;
  payload?: {
    fps?: number;
    message?: string;
  };
};

const toJsonScript = (message: unknown): string => {
  const serialized = JSON.stringify(message);
  return `window.dispatchEvent(new MessageEvent('message', { data: ${JSON.stringify(serialized)} })); true;`;
};

export const useGaussianBridge = (webViewRef: RefObject<WebView | null>) => {
  const [isReady, setIsReady] = useState(false);
  const [fps, setFps] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const send = useCallback((message: unknown) => {
    const webView = webViewRef.current;
    if (!webView) {
      return;
    }

    webView.injectJavaScript(toJsonScript(message));
  }, [webViewRef]);

  const sendFrame = useCallback(
    (frame: SensingFrame) => {
      send({
        type: 'FRAME_UPDATE',
        payload: frame,
      });
    },
    [send],
  );

  const onMessage = useCallback((event: WebViewMessageEvent) => {
    let parsed: BridgeMessage | null = null;
    const raw = event.nativeEvent.data;

    if (typeof raw === 'string') {
      try {
        parsed = JSON.parse(raw) as BridgeMessage;
      } catch {
        setError('Invalid bridge message format');
        return;
      }
    } else if (typeof raw === 'object' && raw !== null) {
      parsed = raw as BridgeMessage;
    }

    if (!parsed) {
      return;
    }

    if (parsed.type === 'READY') {
      setIsReady(true);
      setError(null);
      return;
    }

    if (parsed.type === 'FPS_TICK') {
      const fpsValue = parsed.payload?.fps;
      if (typeof fpsValue === 'number' && Number.isFinite(fpsValue)) {
        setFps(Math.max(0, Math.floor(fpsValue)));
      }
      return;
    }

    if (parsed.type === 'ERROR') {
      setError(parsed.payload?.message ?? 'Unknown bridge error');
      setIsReady(false);
    }
  }, []);

  return {
    sendFrame,
    onMessage,
    isReady,
    fps,
    error,
    reset: () => {
      setIsReady(false);
      setFps(0);
      setError(null);
    },
  };
};
