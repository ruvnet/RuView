import { useCallback, useRef, useState } from 'react';
import type { WebView, WebViewMessageEvent } from 'react-native-webview';
import type { Alert, Survivor } from '@/types/mat';
import type { SensingFrame } from '@/types/sensing';

type MatBridgeMessageType = 'CREATE_EVENT' | 'ADD_ZONE' | 'FRAME_UPDATE';

type MatIncomingType = 'READY' | 'SURVIVOR_DETECTED' | 'ALERT_GENERATED';

type MatIncomingMessage = {
  type: MatIncomingType;
  payload?: unknown;
};

type MatOutgoingMessage = {
  type: MatBridgeMessageType;
  payload?: unknown;
};

type UseMatBridgeOptions = {
  onSurvivorDetected?: (survivor: Survivor) => void;
  onAlertGenerated?: (alert: Alert) => void;
};

const safeParseJson = (value: string): unknown | null => {
  try {
    return JSON.parse(value);
  } catch {
    return null;
  }
};

export const useMatBridge = ({ onAlertGenerated, onSurvivorDetected }: UseMatBridgeOptions = {}) => {
  const webViewRef = useRef<WebView | null>(null);
  const isReadyRef = useRef(false);
  const queuedMessages = useRef<string[]>([]);
  const [ready, setReady] = useState(false);

  const flush = useCallback(() => {
    if (!webViewRef.current || !isReadyRef.current) {
      return;
    }

    while (queuedMessages.current.length > 0) {
      const payload = queuedMessages.current.shift();
      if (payload) {
        webViewRef.current.postMessage(payload);
      }
    }
  }, []);

  const sendMessage = useCallback(
    (message: MatOutgoingMessage) => {
      const payload = JSON.stringify(message);
      if (isReadyRef.current && webViewRef.current) {
        webViewRef.current.postMessage(payload);
        return;
      }
      queuedMessages.current.push(payload);
    },
    [],
  );

  const sendFrameUpdate = useCallback(
    (frame: SensingFrame) => {
      sendMessage({ type: 'FRAME_UPDATE', payload: frame });
    },
    [sendMessage],
  );

  const postEvent = useCallback(
    (type: 'CREATE_EVENT' | 'ADD_ZONE') => {
      return (payload: unknown) => {
        sendMessage({
          type,
          payload,
        });
      };
    },
    [sendMessage],
  );

  const onMessage = useCallback(
    (event: WebViewMessageEvent) => {
      const payload = safeParseJson(event.nativeEvent.data);
      if (!payload || typeof payload !== 'object') {
        return;
      }

      const message = payload as MatIncomingMessage;
      if (message.type === 'READY') {
        isReadyRef.current = true;
        setReady(true);
        flush();
        return;
      }

      if (message.type === 'SURVIVOR_DETECTED') {
        onSurvivorDetected?.(message.payload as Survivor);
        return;
      }

      if (message.type === 'ALERT_GENERATED') {
        onAlertGenerated?.(message.payload as Alert);
      }
    },
    [flush, onAlertGenerated, onSurvivorDetected],
  );

  return {
    webViewRef,
    ready,
    onMessage,
    sendMessage,
    sendFrameUpdate,
    postEvent,
  };
};
