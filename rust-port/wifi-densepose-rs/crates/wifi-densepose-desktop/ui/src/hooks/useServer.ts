import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ServerConfig, ServerStatus } from "../types";

const DEFAULT_CONFIG: ServerConfig = {
  http_port: 8080,
  ws_port: 8765,
  udp_port: 5005,
  static_dir: null,
  model_dir: null,
  log_level: "info",
  source: "simulate",
};

interface UseServerOptions {
  /** Poll interval for status checks in ms. Default: 5000 */
  pollInterval?: number;
}

interface UseServerReturn {
  status: ServerStatus | null;
  isRunning: boolean;
  error: string | null;
  start: (config?: Partial<ServerConfig>) => Promise<void>;
  stop: () => Promise<void>;
  refresh: () => Promise<void>;
}

export function useServer(options: UseServerOptions = {}): UseServerReturn {
  const { pollInterval = 5000 } = options;

  const [status, setStatus] = useState<ServerStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const refresh = useCallback(async () => {
    try {
      const s = await invoke<ServerStatus>("server_status");
      setStatus(s);
      setError(null);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : String(err);
      setError(message);
    }
  }, []);

  const start = useCallback(
    async (overrides: Partial<ServerConfig> = {}) => {
      setError(null);
      const config: ServerConfig = { ...DEFAULT_CONFIG, ...overrides };
      try {
        await invoke("start_server", { config });
        // Allow the server a moment to start, then refresh status
        await new Promise((r) => setTimeout(r, 500));
        await refresh();
      } catch (err) {
        const message =
          err instanceof Error ? err.message : String(err);
        setError(message);
      }
    },
    [refresh]
  );

  const stop = useCallback(async () => {
    setError(null);
    try {
      await invoke("stop_server");
      await new Promise((r) => setTimeout(r, 300));
      await refresh();
    } catch (err) {
      const message =
        err instanceof Error ? err.message : String(err);
      setError(message);
    }
  }, [refresh]);

  // Initial status check
  useEffect(() => {
    refresh();
  }, [refresh]);

  // Polling
  useEffect(() => {
    if (pollInterval <= 0) return;

    intervalRef.current = setInterval(refresh, pollInterval);

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [pollInterval, refresh]);

  const isRunning = status?.running ?? false;

  return {
    status,
    isRunning,
    error,
    start,
    stop,
    refresh,
  };
}
