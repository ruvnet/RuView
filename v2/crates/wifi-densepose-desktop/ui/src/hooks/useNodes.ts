import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Node } from "../types";

interface UseNodesOptions {
  /** Auto-poll interval in milliseconds. Set to 0 to disable. Default: 10000 */
  pollInterval?: number;
  /** Whether to start scanning on mount. Default: false */
  autoScan?: boolean;
}

interface UseNodesReturn {
  nodes: Node[];
  isScanning: boolean;
  error: string | null;
  scan: () => Promise<void>;
  /** Total nodes discovered */
  total: number;
  /** Nodes currently online */
  onlineCount: number;
  /** Nodes currently offline */
  offlineCount: number;
}

export function useNodes(options: UseNodesOptions = {}): UseNodesReturn {
  const { pollInterval = 10_000, autoScan = false } = options;

  const [nodes, setNodes] = useState<Node[]>([]);
  const [isScanning, setIsScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const scan = useCallback(async () => {
    if (isScanning) return;
    setIsScanning(true);
    setError(null);

    try {
      const discovered = await invoke<Node[]>("discover_nodes", {
        timeoutMs: 5000,
      });
      setNodes(discovered);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : String(err);
      setError(message);
    } finally {
      setIsScanning(false);
    }
  }, [isScanning]);

  // Auto-scan on mount if requested
  useEffect(() => {
    if (autoScan) {
      scan();
    }
  }, [autoScan]); // eslint-disable-line react-hooks/exhaustive-deps

  // Polling interval
  useEffect(() => {
    if (pollInterval <= 0) return;

    intervalRef.current = setInterval(() => {
      scan();
    }, pollInterval);

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [pollInterval]); // eslint-disable-line react-hooks/exhaustive-deps

  const onlineCount = nodes.filter(
    (n) => n.health === "online"
  ).length;
  const offlineCount = nodes.filter(
    (n) => n.health === "offline"
  ).length;

  return {
    nodes,
    isScanning,
    error,
    scan,
    total: nodes.length,
    onlineCount,
    offlineCount,
  };
}
