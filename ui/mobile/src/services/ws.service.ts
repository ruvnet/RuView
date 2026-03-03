import { SIMULATION_TICK_INTERVAL_MS } from '@/constants/simulation';
import { MAX_RECONNECT_ATTEMPTS, RECONNECT_DELAYS, WS_PATH } from '@/constants/websocket';
import { usePoseStore } from '@/stores/poseStore';
import { generateSimulatedData } from '@/services/simulation.service';
import type { ConnectionStatus, SensingFrame } from '@/types/sensing';

type FrameListener = (frame: SensingFrame) => void;

class WsService {
  private ws: WebSocket | null = null;
  private listeners = new Set<FrameListener>();
  private reconnectAttempt = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private simulationTimer: ReturnType<typeof setInterval> | null = null;
  private targetUrl = '';
  private active = false;
  private status: ConnectionStatus = 'disconnected';

  connect(url: string): void {
    this.targetUrl = url;
    this.active = true;
    this.reconnectAttempt = 0;

    if (!url) {
      this.handleStatusChange('simulated');
      this.startSimulation();
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
        this.stopSimulation();
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
    this.stopSimulation();
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
    // The /ws/sensing endpoint is served on the same HTTP port (no separate WS port needed).
    return `${proto}//${parsed.host}/ws/sensing`;
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
      this.handleStatusChange('simulated');
      this.startSimulation();
      return;
    }

    const delay = RECONNECT_DELAYS[Math.min(this.reconnectAttempt, RECONNECT_DELAYS.length - 1)];
    this.reconnectAttempt += 1;
    this.clearReconnectTimer();
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect(this.targetUrl);
    }, delay);
    this.startSimulation();
  }

  private startSimulation(): void {
    if (this.simulationTimer) {
      return;
    }
    this.simulationTimer = setInterval(() => {
      this.handleStatusChange('simulated');
      const frame = generateSimulatedData();
      this.listeners.forEach((listener) => {
        listener(frame);
      });
    }, SIMULATION_TICK_INTERVAL_MS);
  }

  private stopSimulation(): void {
    if (this.simulationTimer) {
      clearInterval(this.simulationTimer);
      this.simulationTimer = null;
    }
  }

  private clearReconnectTimer(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }
}

export const wsService = new WsService();
