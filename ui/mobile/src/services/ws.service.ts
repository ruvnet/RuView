import { MAX_RECONNECT_ATTEMPTS, RECONNECT_DELAYS, WS_PATH, WS_PORT } from '@/constants/websocket';
import { usePoseStore } from '@/stores/poseStore';
import type { ConnectionStatus, SensingFrame } from '@/types/sensing';

type FrameListener = (frame: SensingFrame) => void;

class WsService {
  private ws: WebSocket | null = null;
  private listeners = new Set<FrameListener>();
  private reconnectAttempt = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private targetUrl = '';
  private active = false;
  private status: ConnectionStatus = 'disconnected';

  connect(url: string): void {
    this.targetUrl = url;
    this.active = true;
    this.reconnectAttempt = 0;

    if (!url) {
      // No server URL configured — stay disconnected. Production builds
      // never fall back to synthetic data.
      this.handleStatusChange('disconnected');
      return;
    }

    if (this.ws && (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING)) {
      return;
    }

    this.handleStatusChange('connecting');

    try {
      const endpoint = this.buildWsUrl(url);
      const socket = new WebSocket(endpoint);
      this.ws = socket;

      socket.onopen = () => {
        this.reconnectAttempt = 0;
        this.handleStatusChange('connected');
      };

      socket.onmessage = (evt) => {
        try {
          const raw = typeof evt.data === 'string' ? evt.data : JSON.stringify(evt.data);
          const frame = JSON.parse(raw) as SensingFrame;
          this.listeners.forEach((listener) => listener(frame));
        } catch {
          // ignore malformed frames
        }
      };

      socket.onerror = () => {
        // handled by onclose
      };

      socket.onclose = (evt) => {
        this.ws = null;
        if (!this.active) {
          this.handleStatusChange('disconnected');
          return;
        }
        if (evt.code === 1000) {
          this.handleStatusChange('disconnected');
          return;
        }
        this.scheduleReconnect();
      };
    } catch {
      this.scheduleReconnect();
    }
  }

  disconnect(): void {
    this.active = false;
    this.clearReconnectTimer();
    if (this.ws) {
      this.ws.close(1000, 'client disconnect');
      this.ws = null;
    }
    this.handleStatusChange('disconnected');
  }

  subscribe(listener: FrameListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  getStatus(): ConnectionStatus {
    return this.status;
  }

  private buildWsUrl(rawUrl: string): string {
    const parsed = new URL(rawUrl);
    const proto = parsed.protocol === 'https:' || parsed.protocol === 'wss:' ? 'wss:' : 'ws:';
    // RuView sensing-server runs WS on a separate port (WS_PORT, default 8765),
    // independent of the HTTP API port. Build the WS URL with that port.
    return `${proto}//${parsed.hostname}:${WS_PORT}${WS_PATH}`;
  }

  private handleStatusChange(status: ConnectionStatus): void {
    if (status === this.status) {
      return;
    }
    this.status = status;
    usePoseStore.getState().setConnectionStatus(status);
  }

  private scheduleReconnect(): void {
    if (!this.active) {
      this.handleStatusChange('disconnected');
      return;
    }

    if (this.reconnectAttempt >= MAX_RECONNECT_ATTEMPTS) {
      // Give up — stay disconnected. No synthetic fallback.
      this.handleStatusChange('disconnected');
      return;
    }

    const delay = RECONNECT_DELAYS[Math.min(this.reconnectAttempt, RECONNECT_DELAYS.length - 1)];
    this.reconnectAttempt += 1;
    this.clearReconnectTimer();
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect(this.targetUrl);
    }, delay);
  }

  private clearReconnectTimer(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }
}

export const wsService = new WsService();
