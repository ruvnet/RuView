// RuView sensing-server (Rust+Axum) exposes the live stream at /ws/sensing on
// its dedicated WebSocket port (default 8765). The legacy wifi-densepose v1
// path (/api/v1/stream/pose) is kept as a fallback in case the mobile app is
// pointed at an old FastAPI backend.
export const WS_PATH = '/ws/sensing';
export const WS_PORT = 8765;
export const RECONNECT_DELAYS = [1000, 2000, 4000, 8000, 16000];
export const MAX_RECONNECT_ATTEMPTS = 10;
