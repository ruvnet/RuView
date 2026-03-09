import React, { useEffect, useState, useRef, useCallback } from "react";
import { useServer } from "../hooks/useServer";
import type { SensingUpdate } from "../types";

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
// Mock data generators
// ---------------------------------------------------------------------------

const MOCK_LOG_TEMPLATES: { level: LogLevel; source: string; message: string }[] = [
  { level: "INFO", source: "sensing-server", message: "HTTP listening on 127.0.0.1:8080" },
  { level: "INFO", source: "udp_receiver", message: "CSI frame from 192.168.1.42" },
  { level: "WARN", source: "vital_signs", message: "Low signal quality on node 2" },
  { level: "INFO", source: "pose_engine", message: "Activity: walking (confidence: 0.87)" },
  { level: "ERROR", source: "ws_session", message: "Client disconnected unexpectedly" },
  { level: "INFO", source: "udp_receiver", message: "CSI frame from 192.168.1.15" },
  { level: "INFO", source: "pose_engine", message: "Activity: sitting (confidence: 0.93)" },
  { level: "INFO", source: "sensing-server", message: "WebSocket client connected from 127.0.0.1" },
  { level: "WARN", source: "mesh_sync", message: "Node 4 heartbeat delayed by 1200ms" },
  { level: "INFO", source: "pose_engine", message: "Activity: standing (confidence: 0.91)" },
  { level: "INFO", source: "udp_receiver", message: "CSI frame from 192.168.1.78" },
  { level: "ERROR", source: "udp_receiver", message: "Malformed CSI payload (len=0)" },
  { level: "INFO", source: "csi_pipeline", message: "Subcarrier FFT complete (52 bins)" },
  { level: "WARN", source: "vital_signs", message: "Breathing rate out of range on node 5" },
  { level: "INFO", source: "pose_engine", message: "Activity: sleeping (confidence: 0.78)" },
];

const MOCK_ACTIVITIES = [
  { activity: "walking", confidence: 0.87 },
  { activity: "sitting", confidence: 0.93 },
  { activity: "standing", confidence: 0.91 },
  { activity: "sleeping", confidence: 0.78 },
  { activity: "exercising", confidence: 0.65 },
];

function formatTimestamp(d: Date): string {
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  const ms = String(d.getMilliseconds()).padStart(3, "0");
  return `${hh}:${mm}:${ss}.${ms}`;
}

let nextLogId = 1;

function createMockLogEntry(): LogEntry {
  const template = MOCK_LOG_TEMPLATES[Math.floor(Math.random() * MOCK_LOG_TEMPLATES.length)];
  return {
    id: nextLogId++,
    timestamp: formatTimestamp(new Date()),
    level: template.level,
    source: template.source,
    message: template.message,
  };
}

function createMockSensingUpdate(): SensingUpdate {
  const act = MOCK_ACTIVITIES[Math.floor(Math.random() * MOCK_ACTIVITIES.length)];
  return {
    timestamp: new Date().toISOString(),
    node_id: Math.floor(Math.random() * 6) + 1,
    subcarrier_count: 52,
    rssi: -(Math.floor(Math.random() * 40) + 30),
    activity: act.activity,
    confidence: parseFloat((act.confidence + (Math.random() * 0.1 - 0.05)).toFixed(2)),
  };
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_LOG_ENTRIES = 200;
const LOG_INTERVAL_MS = 2000;

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
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!paused && bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: "smooth" });
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
        <div ref={bottomRef} />
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

  // Log viewer state
  const [logEntries, setLogEntries] = useState<LogEntry[]>([]);
  const [paused, setPaused] = useState(false);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  // Activity feed state
  const [activities, setActivities] = useState<SensingUpdate[]>([]);

  // Simulated log feed
  useEffect(() => {
    const interval = setInterval(() => {
      if (pausedRef.current) return;
      const entry = createMockLogEntry();
      setLogEntries((prev) => {
        const next = [...prev, entry];
        return next.length > MAX_LOG_ENTRIES ? next.slice(next.length - MAX_LOG_ENTRIES) : next;
      });

      // Also push an activity update every ~3rd tick
      if (Math.random() < 0.35) {
        setActivities((prev) => {
          const update = createMockSensingUpdate();
          const next = [update, ...prev];
          return next.slice(0, 5);
        });
      }
    }, LOG_INTERVAL_MS);

    return () => clearInterval(interval);
  }, []);

  const handleClearLog = useCallback(() => setLogEntries([]), []);
  const handleTogglePause = useCallback(() => setPaused((p) => !p), []);

  const handleStart = async () => {
    setStarting(true);
    try {
      await start();
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
              </div>
            )}
          </div>

          {/* Right: action button */}
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
