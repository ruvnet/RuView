import React, { useEffect, useState, useRef, useCallback } from "react";
import { useServer } from "../hooks/useServer";
import type { SensingUpdate, DataSource } from "../types";

// ---------------------------------------------------------------------------
// Log entry model
// ---------------------------------------------------------------------------

type LogLevel = "INFO" | "WARN" | "ERROR";

interface LogEntry {
  id: number;
  timestamp: string; // HH:MM:SS.mmm
  level: LogLevel;
  source: string;
  message: string;
}

// ---------------------------------------------------------------------------
// WebSocket message types from sensing server
// ---------------------------------------------------------------------------

interface WsNodeInfo {
  node_id: number;
  rssi_dbm: number;
  position: [number, number, number];
  amplitude: number[];
  subcarrier_count: number;
}

interface WsClassification {
  motion_level: string;
  presence: boolean;
  confidence: number;
}

interface WsFeatures {
  mean_rssi: number;
  variance: number;
  motion_band_power: number;
  breathing_band_power: number;
  dominant_freq_hz: number;
  change_points: number;
  spectral_power: number;
}

interface WsVitalSigns {
  breathing_rate_hz?: number;
  heart_rate_bpm?: number;
  confidence?: number;
}

interface WsSensingUpdate {
  type: string;
  timestamp: number;
  source: string;
  tick: number;
  nodes: WsNodeInfo[];
  features: WsFeatures;
  classification: WsClassification;
  vital_signs?: WsVitalSigns;
  posture?: string;
  signal_quality_score?: number;
  quality_verdict?: string;
  bssid_count?: number;
  estimated_persons?: number;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatTimestamp(d: Date): string {
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  const ms = String(d.getMilliseconds()).padStart(3, "0");
  return `${hh}:${mm}:${ss}.${ms}`;
}

let nextLogId = 1;

function createLogFromWsUpdate(update: WsSensingUpdate): LogEntry[] {
  const entries: LogEntry[] = [];
  const ts = formatTimestamp(new Date(update.timestamp * 1000));

  // Log each node's CSI data
  for (const node of update.nodes) {
    entries.push({
      id: nextLogId++,
      timestamp: ts,
      level: "INFO",
      source: "csi_receiver",
      message: `Node ${node.node_id}: RSSI ${node.rssi_dbm.toFixed(1)} dBm, ${node.subcarrier_count} subcarriers`,
    });
  }

  // Log classification
  if (update.classification) {
    const level: LogLevel = update.classification.confidence < 0.5 ? "WARN" : "INFO";
    entries.push({
      id: nextLogId++,
      timestamp: ts,
      level,
      source: "classifier",
      message: `Motion: ${update.classification.motion_level} (presence=${update.classification.presence}, conf=${(update.classification.confidence * 100).toFixed(0)}%)`,
    });
  }

  // Log vital signs if present
  if (update.vital_signs) {
    const vs = update.vital_signs;
    const level: LogLevel = (vs.confidence ?? 0) < 0.5 ? "WARN" : "INFO";
    entries.push({
      id: nextLogId++,
      timestamp: ts,
      level,
      source: "vital_signs",
      message: `Breathing: ${vs.breathing_rate_hz?.toFixed(2) ?? "--"} Hz, HR: ${vs.heart_rate_bpm?.toFixed(0) ?? "--"} bpm`,
    });
  }

  // Log quality verdict if present
  if (update.quality_verdict && update.quality_verdict !== "Permit") {
    entries.push({
      id: nextLogId++,
      timestamp: ts,
      level: update.quality_verdict === "Deny" ? "ERROR" : "WARN",
      source: "quality_gate",
      message: `Signal quality: ${update.quality_verdict} (score=${(update.signal_quality_score ?? 0).toFixed(2)})`,
    });
  }

  return entries;
}

function createActivityFromWsUpdate(update: WsSensingUpdate): SensingUpdate | null {
  if (!update.classification) return null;

  const node = update.nodes[0];
  return {
    timestamp: new Date(update.timestamp * 1000).toISOString(),
    node_id: node?.node_id ?? 1,
    subcarrier_count: node?.subcarrier_count ?? 52,
    rssi: node?.rssi_dbm ?? -50,
    activity: update.posture ?? update.classification.motion_level,
    confidence: update.classification.confidence,
  };
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_LOG_ENTRIES = 200;
const WS_RECONNECT_DELAY_MS = 3000;

// ---------------------------------------------------------------------------
// LogViewer component (ADR-053)
// ---------------------------------------------------------------------------

const LEVEL_COLOR: Record<LogLevel, string> = {
  INFO: "var(--text-secondary)",
  WARN: "var(--status-warning)",
  ERROR: "var(--status-error)",
};

function LogViewer({
  entries,
  onClear,
  paused,
  onTogglePause,
}: {
  entries: LogEntry[];
  onClear: () => void;
  paused: boolean;
  onTogglePause: () => void;
}) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Scroll to bottom within the container only (not the page)
    if (!paused && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [entries, paused]);

  return (
    <div
      style={{
        background: "var(--bg-surface)",
        border: "1px solid var(--border)",
        borderRadius: 8,
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      }}
    >
      {/* Header bar */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          padding: "var(--space-2) var(--space-4)",
          borderBottom: "1px solid var(--border)",
          background: "var(--bg-elevated)",
          flexShrink: 0,
        }}
      >
        <span
          style={{
            fontSize: 12,
            fontWeight: 600,
            textTransform: "uppercase",
            letterSpacing: "0.05em",
            color: "var(--text-muted)",
          }}
        >
          Server Log
        </span>
        <div style={{ display: "flex", gap: "var(--space-2)" }}>
          <button
            onClick={onTogglePause}
            style={{
              padding: "var(--space-1) var(--space-3)",
              fontSize: 12,
              borderRadius: 4,
              background: paused ? "var(--status-warning)" : "var(--bg-hover)",
              color: paused ? "#000" : "var(--text-secondary)",
              border: "1px solid var(--border)",
              cursor: "pointer",
              fontWeight: 500,
            }}
          >
            {paused ? "Resume" : "Pause"}
          </button>
          <button
            onClick={onClear}
            style={{
              padding: "var(--space-1) var(--space-3)",
              fontSize: 12,
              borderRadius: 4,
              background: "var(--bg-hover)",
              color: "var(--text-secondary)",
              border: "1px solid var(--border)",
              cursor: "pointer",
              fontWeight: 500,
            }}
          >
            Clear
          </button>
        </div>
      </div>

      {/* Log entries */}
      <div
        ref={containerRef}
        style={{
          height: 320,
          overflowY: "auto",
          padding: "var(--space-2) var(--space-3)",
          fontFamily: "var(--font-mono)",
          fontSize: 12,
          lineHeight: 1.7,
        }}
      >
        {entries.length === 0 ? (
          <div style={{ color: "var(--text-muted)", padding: "var(--space-4)", textAlign: "center" }}>
            No log entries yet.
          </div>
        ) : (
          entries.map((entry) => (
            <div key={entry.id} style={{ whiteSpace: "nowrap" }}>
              <span style={{ color: "var(--text-muted)" }}>{entry.timestamp}</span>{" "}
              <span
                style={{
                  color: LEVEL_COLOR[entry.level],
                  fontWeight: entry.level === "ERROR" ? 700 : 500,
                  display: "inline-block",
                  minWidth: 40,
                }}
              >
                {entry.level}
              </span>{" "}
              <span style={{ color: "var(--accent)" }}>{entry.source}</span>{" "}
              <span style={{ color: LEVEL_COLOR[entry.level] }}>{entry.message}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sensing page
// ---------------------------------------------------------------------------

export const Sensing: React.FC = () => {
  const { status, isRunning, error, start, stop } = useServer({ pollInterval: 5000 });
  const [starting, setStarting] = useState(false);
  const [stopping, setStopping] = useState(false);

  // Data source selection
  const [dataSource, setDataSource] = useState<DataSource>("simulate");

  // Log viewer state
  const [logEntries, setLogEntries] = useState<LogEntry[]>([]);
  const [paused, setPaused] = useState(false);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  // Activity feed state
  const [activities, setActivities] = useState<SensingUpdate[]>([]);

  // WebSocket connection state
  const [wsConnected, setWsConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);

  // Connect to real WebSocket when server is running
  useEffect(() => {
    if (!isRunning || !status?.ws_port) {
      // Server not running, disconnect if connected
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
        setWsConnected(false);
      }
      return;
    }

    const connect = () => {
      const wsUrl = `ws://127.0.0.1:${status.ws_port}/ws/sensing`;
      const ws = new WebSocket(wsUrl);

      ws.onopen = () => {
        setWsConnected(true);
        setLogEntries((prev) => [
          ...prev,
          {
            id: nextLogId++,
            timestamp: formatTimestamp(new Date()),
            level: "INFO",
            source: "desktop",
            message: `WebSocket connected to ${wsUrl}`,
          },
        ]);
      };

      ws.onmessage = (event) => {
        if (pausedRef.current) return;

        try {
          const update = JSON.parse(event.data) as WsSensingUpdate;

          // Create log entries from the update
          const entries = createLogFromWsUpdate(update);
          if (entries.length > 0) {
            setLogEntries((prev) => {
              const next = [...prev, ...entries];
              return next.length > MAX_LOG_ENTRIES ? next.slice(next.length - MAX_LOG_ENTRIES) : next;
            });
          }

          // Create activity update
          const activity = createActivityFromWsUpdate(update);
          if (activity) {
            setActivities((prev) => {
              const next = [activity, ...prev];
              return next.slice(0, 5);
            });
          }
        } catch (err) {
          console.error("Failed to parse WebSocket message:", err);
        }
      };

      ws.onclose = () => {
        setWsConnected(false);
        wsRef.current = null;

        // Only add disconnect log if server is still supposed to be running
        if (isRunning) {
          setLogEntries((prev) => [
            ...prev,
            {
              id: nextLogId++,
              timestamp: formatTimestamp(new Date()),
              level: "WARN",
              source: "desktop",
              message: "WebSocket disconnected, reconnecting...",
            },
          ]);

          // Attempt reconnect
          reconnectTimeoutRef.current = window.setTimeout(connect, WS_RECONNECT_DELAY_MS);
        }
      };

      ws.onerror = () => {
        setLogEntries((prev) => [
          ...prev,
          {
            id: nextLogId++,
            timestamp: formatTimestamp(new Date()),
            level: "ERROR",
            source: "desktop",
            message: "WebSocket connection error",
          },
        ]);
      };

      wsRef.current = ws;
    };

    connect();

    return () => {
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [isRunning, status?.ws_port]);

  const handleClearLog = useCallback(() => setLogEntries([]), []);
  const handleTogglePause = useCallback(() => setPaused((p) => !p), []);

  const handleStart = async () => {
    setStarting(true);
    try {
      await start({ source: dataSource });
    } finally {
      setStarting(false);
    }
  };

  const handleStop = async () => {
    setStopping(true);
    try {
      await stop();
    } finally {
      setStopping(false);
    }
  };

  return (
    <div style={{ padding: "var(--space-5)" }}>
      {/* Page header */}
      <h2 className="heading-lg" style={{ marginBottom: "var(--space-5)" }}>
        Sensing
      </h2>

      {/* ----------------------------------------------------------------- */}
      {/* Section 1: Server Control                                         */}
      {/* ----------------------------------------------------------------- */}
      <div
        style={{
          background: "var(--bg-surface)",
          border: "1px solid var(--border)",
          borderRadius: 8,
          padding: "var(--space-4)",
          marginBottom: "var(--space-5)",
        }}
      >
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}
        >
          {/* Left: status info */}
          <div style={{ display: "flex", alignItems: "center", gap: "var(--space-3)" }}>
            {/* Status dot */}
            <span
              style={{
                width: 10,
                height: 10,
                borderRadius: "50%",
                background: isRunning ? "var(--status-online)" : "var(--status-error)",
                boxShadow: isRunning ? "0 0 6px var(--status-online)" : "none",
                flexShrink: 0,
              }}
            />

            <div>
              <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>
                Sensing Server
              </div>
              <div style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 2 }}>
                {isRunning ? "Running" : "Stopped"}
              </div>
            </div>

            {/* Running details */}
            {isRunning && status && (
              <div
                style={{
                  display: "flex",
                  gap: "var(--space-4)",
                  marginLeft: "var(--space-3)",
                  fontFamily: "var(--font-mono)",
                  fontSize: 12,
                  color: "var(--text-muted)",
                }}
              >
                {status.pid != null && <span>PID {status.pid}</span>}
                {status.http_port != null && <span>HTTP :{status.http_port}</span>}
                {status.ws_port != null && <span>WS :{status.ws_port}</span>}
                <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
                  <span
                    style={{
                      width: 6,
                      height: 6,
                      borderRadius: "50%",
                      background: wsConnected ? "var(--status-online)" : "var(--status-warning)",
                    }}
                  />
                  {wsConnected ? "Live" : "Connecting..."}
                </span>
              </div>
            )}
          </div>

          {/* Right: data source + action button */}
          <div style={{ display: "flex", alignItems: "center", gap: "var(--space-3)" }}>
            {/* Data source selector */}
            <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
              <label
                style={{
                  fontSize: 12,
                  color: "var(--text-muted)",
                  fontWeight: 500,
                }}
              >
                Source:
              </label>
              <select
                value={dataSource}
                onChange={(e) => setDataSource(e.target.value as DataSource)}
                disabled={isRunning}
                style={{
                  padding: "var(--space-1) var(--space-2)",
                  borderRadius: 4,
                  fontSize: 12,
                  fontWeight: 500,
                  border: "1px solid var(--border)",
                  background: isRunning ? "var(--bg-hover)" : "var(--bg-surface)",
                  color: "var(--text-primary)",
                  cursor: isRunning ? "not-allowed" : "pointer",
                  opacity: isRunning ? 0.6 : 1,
                }}
              >
                <option value="simulate">Simulate</option>
                <option value="esp32">ESP32 (Real)</option>
                <option value="wifi">WiFi (RSSI)</option>
                <option value="auto">Auto Detect</option>
              </select>
            </div>

            {/* Action button */}
            <button
              onClick={isRunning ? handleStop : handleStart}
              disabled={starting || stopping}
              style={{
                padding: "var(--space-2) var(--space-4)",
                borderRadius: 6,
                fontSize: 13,
                fontWeight: 600,
                cursor: starting || stopping ? "not-allowed" : "pointer",
                border: "none",
                background: isRunning ? "var(--status-error)" : "var(--accent)",
                color: "#fff",
                opacity: starting || stopping ? 0.6 : 1,
              }}
            >
              {starting ? "Starting..." : stopping ? "Stopping..." : isRunning ? "Stop Server" : "Start Server"}
            </button>
          </div>
        </div>

        {/* Error display */}
        {error && (
          <div
            style={{
              marginTop: "var(--space-3)",
              padding: "var(--space-2) var(--space-3)",
              background: "rgba(255,59,48,0.1)",
              borderRadius: 4,
              fontSize: 12,
              color: "var(--status-error)",
              fontFamily: "var(--font-mono)",
            }}
          >
            {error}
          </div>
        )}
      </div>

      {/* ----------------------------------------------------------------- */}
      {/* Section 2: Log Viewer (ADR-053)                                   */}
      {/* ----------------------------------------------------------------- */}
      <div style={{ marginBottom: "var(--space-5)" }}>
        <LogViewer
          entries={logEntries}
          onClear={handleClearLog}
          paused={paused}
          onTogglePause={handleTogglePause}
        />
      </div>

      {/* ----------------------------------------------------------------- */}
      {/* Section 3: Activity Feed                                          */}
      {/* ----------------------------------------------------------------- */}
      <div
        style={{
          background: "var(--bg-surface)",
          border: "1px solid var(--border)",
          borderRadius: 8,
          padding: "var(--space-4)",
        }}
      >
        <h3
          style={{
            fontSize: 12,
            fontWeight: 600,
            textTransform: "uppercase",
            letterSpacing: "0.05em",
            color: "var(--text-muted)",
            marginBottom: "var(--space-3)",
          }}
        >
          Activity Feed
        </h3>

        {activities.length === 0 ? (
          <div style={{ fontSize: 13, color: "var(--text-muted)", textAlign: "center", padding: "var(--space-4)" }}>
            Waiting for sensing data...
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-3)" }}>
            {activities.map((update, i) => {
              const ts = new Date(update.timestamp);
              const conf = update.confidence ?? 0;
              return (
                <div
                  key={`${update.timestamp}-${i}`}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "var(--space-3)",
                    padding: "var(--space-2) var(--space-3)",
                    background: "var(--bg-base)",
                    borderRadius: 6,
                    border: "1px solid var(--border)",
                  }}
                >
                  {/* Timestamp */}
                  <span
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: 11,
                      color: "var(--text-muted)",
                      flexShrink: 0,
                      minWidth: 72,
                    }}
                  >
                    {formatTimestamp(ts)}
                  </span>

                  {/* Node ID */}
                  <span
                    style={{
                      fontSize: 11,
                      color: "var(--text-muted)",
                      flexShrink: 0,
                      minWidth: 48,
                    }}
                  >
                    Node {update.node_id}
                  </span>

                  {/* Activity */}
                  <span
                    style={{
                      fontSize: 13,
                      fontWeight: 600,
                      color: "var(--text-primary)",
                      flexShrink: 0,
                      minWidth: 80,
                      textTransform: "capitalize",
                    }}
                  >
                    {update.activity ?? "unknown"}
                  </span>

                  {/* Confidence bar */}
                  <div
                    style={{
                      flex: 1,
                      height: 6,
                      background: "var(--bg-hover)",
                      borderRadius: 3,
                      overflow: "hidden",
                      minWidth: 60,
                    }}
                  >
                    <div
                      style={{
                        width: `${Math.round(conf * 100)}%`,
                        height: "100%",
                        background: conf >= 0.8 ? "var(--status-online)" : conf >= 0.6 ? "var(--status-warning)" : "var(--status-error)",
                        borderRadius: 3,
                        transition: "width 0.3s ease",
                      }}
                    />
                  </div>

                  {/* Confidence value */}
                  <span
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: 11,
                      color: "var(--text-secondary)",
                      flexShrink: 0,
                      minWidth: 36,
                      textAlign: "right",
                    }}
                  >
                    {Math.round(conf * 100)}%
                  </span>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};

export default Sensing;
