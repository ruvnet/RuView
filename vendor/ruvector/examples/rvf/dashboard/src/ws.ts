export interface LiveEvent {
  event_type: string;
  timestamp: number;
  data: Record<string, unknown>;
}

type EventCallback = (event: LiveEvent) => void;

const listeners: EventCallback[] = [];
let socket: WebSocket | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let reconnectDelay = 1000;
let intentionalClose = false;

const MAX_RECONNECT_DELAY = 30000;
const RECONNECT_BACKOFF = 2;

function handleMessage(raw: MessageEvent): void {
  try {
    const event = JSON.parse(raw.data as string) as LiveEvent;
    for (const cb of listeners) {
      cb(event);
    }
  } catch {
    // Ignore malformed messages
  }
}

function scheduleReconnect(): void {
  if (intentionalClose) return;
  if (reconnectTimer) return;

  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    openSocket();
  }, reconnectDelay);

  reconnectDelay = Math.min(reconnectDelay * RECONNECT_BACKOFF, MAX_RECONNECT_DELAY);
}

function openSocket(): void {
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
    return;
  }

  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const url = `${protocol}//${location.host}/ws/live`;

  try {
    socket = new WebSocket(url);
  } catch {
    scheduleReconnect();
    return;
  }

  socket.addEventListener('open', () => {
    reconnectDelay = 1000;
  });

  socket.addEventListener('message', handleMessage);

  socket.addEventListener('close', () => {
    socket = null;
    scheduleReconnect();
  });

  socket.addEventListener('error', () => {
    socket?.close();
  });
}

export function onEvent(callback: EventCallback): () => void {
  listeners.push(callback);
  return () => {
    const idx = listeners.indexOf(callback);
    if (idx >= 0) listeners.splice(idx, 1);
  };
}

export function connect(): void {
  intentionalClose = false;
  reconnectDelay = 1000;
  openSocket();
}

export function disconnect(): void {
  intentionalClose = true;
  if (reconnectTimer) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  if (socket) {
    socket.close();
    socket = null;
  }
}
