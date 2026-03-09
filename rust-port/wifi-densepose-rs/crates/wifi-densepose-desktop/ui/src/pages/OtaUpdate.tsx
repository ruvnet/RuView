import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  Node,
  OtaStrategy,
  BatchNodeState,
  OtaResult,
} from "../types";

type Mode = "single" | "batch";

interface DiscoveredNode {
  ip: string;
  mac: string | null;
  hostname: string | null;
  node_id: number;
  firmware_version: string | null;
  health: string;
  last_seen: string;
}

const STRATEGY_LABELS: Record<OtaStrategy, string> = {
  sequential: "Sequential",
  tdm_safe: "TDM-Safe",
  parallel: "Parallel",
};

const STATE_CONFIG: Record<BatchNodeState, { label: string; color: string }> = {
  queued: { label: "Queued", color: "var(--text-muted)" },
  uploading: { label: "Uploading", color: "var(--status-info)" },
  rebooting: { label: "Rebooting", color: "var(--status-warning)" },
  verifying: { label: "Verifying", color: "var(--status-info)" },
  done: { label: "Done", color: "var(--status-online)" },
  failed: { label: "Failed", color: "var(--status-error)" },
  skipped: { label: "Skipped", color: "var(--text-muted)" },
};

export function OtaUpdate() {
  const [mode, setMode] = useState<Mode>("single");
  const [nodes, setNodes] = useState<DiscoveredNode[]>([]);
  const [isDiscovering, setIsDiscovering] = useState(false);
  const [firmwarePath, setFirmwarePath] = useState("");
  const [psk, setPsk] = useState("");
  const [error, setError] = useState<string | null>(null);

  // Single mode state
  const [selectedNodeIp, setSelectedNodeIp] = useState("");
  const [isSingleUpdating, setIsSingleUpdating] = useState(false);
  const [singleResult, setSingleResult] = useState<OtaResult | null>(null);

  // Batch mode state
  const [selectedBatchIps, setSelectedBatchIps] = useState<Set<string>>(new Set());
  const [strategy, setStrategy] = useState<OtaStrategy>("sequential");
  const [isBatchUpdating, setIsBatchUpdating] = useState(false);
  const [batchResults, setBatchResults] = useState<OtaResult[]>([]);
  const [batchNodeStates, setBatchNodeStates] = useState<Map<string, BatchNodeState>>(new Map());

  const discoverNodes = useCallback(async () => {
    setIsDiscovering(true);
    setError(null);
    try {
      const result = await invoke<DiscoveredNode[]>("discover_nodes", { timeoutMs: 5000 });
      setNodes(result);
      if (result.length === 0) {
        setError("No nodes discovered. Ensure ESP32 nodes are online and reachable.");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsDiscovering(false);
    }
  }, []);

  const pickFirmware = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        multiple: false,
        filters: [
          { name: "Firmware Binary", extensions: ["bin"] },
          { name: "All Files", extensions: ["*"] },
        ],
      });
      if (selected && typeof selected === "string") setFirmwarePath(selected);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const startSingleOta = async () => {
    if (!selectedNodeIp || !firmwarePath) return;
    setIsSingleUpdating(true);
    setSingleResult(null);
    setError(null);
    try {
      const result = await invoke<OtaResult>("ota_update", {
        nodeIp: selectedNodeIp,
        firmwarePath,
        psk: psk || null,
      });
      setSingleResult(result);
    } catch (err) {
      setSingleResult({
        node_ip: selectedNodeIp,
        success: false,
        previous_version: null,
        new_version: null,
        duration_ms: 0,
        error: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setIsSingleUpdating(false);
    }
  };

  const startBatchOta = async () => {
    const ips = Array.from(selectedBatchIps);
    if (ips.length === 0 || !firmwarePath) return;
    setIsBatchUpdating(true);
    setBatchResults([]);
    setError(null);

    // Initialize all nodes as queued
    const initialStates = new Map<string, BatchNodeState>();
    ips.forEach((ip) => initialStates.set(ip, "queued"));
    setBatchNodeStates(new Map(initialStates));

    // Mark all as uploading while the batch runs
    ips.forEach((ip) => initialStates.set(ip, "uploading"));
    setBatchNodeStates(new Map(initialStates));

    try {
      const results = await invoke<OtaResult[]>("batch_ota_update", {
        nodeIps: ips,
        firmwarePath,
        psk: psk || null,
      });
      setBatchResults(results);

      // Update per-node states from results
      const finalStates = new Map<string, BatchNodeState>();
      results.forEach((r) => {
        finalStates.set(r.node_ip, r.success ? "done" : "failed");
      });
      setBatchNodeStates(finalStates);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      // Mark all as failed on total failure
      const failStates = new Map<string, BatchNodeState>();
      ips.forEach((ip) => failStates.set(ip, "failed"));
      setBatchNodeStates(failStates);
    } finally {
      setIsBatchUpdating(false);
    }
  };

  const toggleBatchNode = (ip: string) => {
    setSelectedBatchIps((prev) => {
      const next = new Set(prev);
      if (next.has(ip)) next.delete(ip);
      else next.add(ip);
      return next;
    });
  };

  const toggleAll = () => {
    if (selectedBatchIps.size === nodes.length) {
      setSelectedBatchIps(new Set());
    } else {
      setSelectedBatchIps(new Set(nodes.map((n) => n.ip)));
    }
  };

  const nodeLabel = (n: DiscoveredNode) => {
    const parts = [n.ip];
    if (n.hostname) parts.push(n.hostname);
    if (n.firmware_version) parts.push(`v${n.firmware_version}`);
    return parts.join(" - ");
  };

  const canStartSingle = selectedNodeIp !== "" && firmwarePath !== "" && !isSingleUpdating;
  const canStartBatch = selectedBatchIps.size > 0 && firmwarePath !== "" && !isBatchUpdating;

  return (
    <div style={{ padding: "var(--space-5)", maxWidth: 800 }}>
      <h1 className="heading-lg" style={{ margin: "0 0 var(--space-1)" }}>OTA Update</h1>
      <p style={{ fontSize: 13, color: "var(--text-secondary)", marginBottom: "var(--space-5)" }}>
        Push firmware updates to ESP32 nodes over the network
      </p>

      {/* Mode Tabs */}
      <div style={{ display: "flex", gap: 0, marginBottom: "var(--space-5)" }}>
        <TabButton label="Single Node" active={mode === "single"} onClick={() => setMode("single")} side="left" />
        <TabButton label="Batch OTA" active={mode === "batch"} onClick={() => setMode("batch")} side="right" />
      </div>

      {error && <div style={bannerStyle("var(--status-error)")}>{error}</div>}

      {/* Node Discovery Section */}
      <div style={{ ...cardStyle, marginBottom: "var(--space-4)" }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "var(--space-3)" }}>
          <h2 style={sectionTitleStyle}>Discovered Nodes</h2>
          <button onClick={discoverNodes} style={secondaryBtn} disabled={isDiscovering}>
            {isDiscovering ? "Scanning..." : nodes.length > 0 ? "Re-scan" : "Discover Nodes"}
          </button>
        </div>

        {nodes.length === 0 && !isDiscovering && (
          <p style={{ fontSize: 13, color: "var(--text-muted)", margin: 0 }}>
            No nodes discovered yet. Click Discover Nodes to scan the network.
          </p>
        )}

        {nodes.length > 0 && mode === "single" && (
          <div>
            <label style={labelStyle}>Target Node</label>
            <select
              value={selectedNodeIp}
              onChange={(e) => setSelectedNodeIp(e.target.value)}
              style={{ width: "100%" }}
            >
              <option value="">Select a node...</option>
              {nodes.map((n) => (
                <option key={n.ip} value={n.ip}>{nodeLabel(n)}</option>
              ))}
            </select>
          </div>
        )}

        {nodes.length > 0 && mode === "batch" && (
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)", marginBottom: "var(--space-2)" }}>
              <label style={{ ...labelStyle, marginBottom: 0 }}>Select Nodes</label>
              <button onClick={toggleAll} style={{ ...linkBtn, fontSize: 11 }}>
                {selectedBatchIps.size === nodes.length ? "Deselect All" : "Select All"}
              </button>
            </div>
            <div style={{ maxHeight: 200, overflowY: "auto", border: "1px solid var(--border)", borderRadius: 6 }}>
              {nodes.map((n) => (
                <label
                  key={n.ip}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "var(--space-3)",
                    padding: "var(--space-2) var(--space-3)",
                    borderBottom: "1px solid var(--border)",
                    cursor: "pointer",
                    background: selectedBatchIps.has(n.ip) ? "var(--bg-hover)" : "transparent",
                    fontSize: 13,
                  }}
                >
                  <input
                    type="checkbox"
                    checked={selectedBatchIps.has(n.ip)}
                    onChange={() => toggleBatchNode(n.ip)}
                    style={{ accentColor: "var(--accent)" }}
                  />
                  <span style={{ flex: 1, color: "var(--text-primary)", fontFamily: "var(--font-mono)", fontSize: 12 }}>
                    {n.ip}
                  </span>
                  <span style={{ color: "var(--text-secondary)", fontSize: 12 }}>
                    {n.hostname ?? "unknown"}
                  </span>
                  <span style={{ color: "var(--text-muted)", fontSize: 11, fontFamily: "var(--font-mono)" }}>
                    {n.firmware_version ? `v${n.firmware_version}` : ""}
                  </span>
                  <StatusDot health={n.health} />
                </label>
              ))}
            </div>
            <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: "var(--space-1)", marginBottom: 0 }}>
              {selectedBatchIps.size} of {nodes.length} nodes selected
            </p>
          </div>
        )}
      </div>

      {/* Firmware & Config Section */}
      <div style={{ ...cardStyle, marginBottom: "var(--space-4)" }}>
        <h2 style={{ ...sectionTitleStyle, marginBottom: "var(--space-3)" }}>Firmware & Configuration</h2>

        <div style={{ marginBottom: "var(--space-4)" }}>
          <label style={labelStyle}>Firmware Binary (.bin)</label>
          <div style={{ display: "flex", gap: "var(--space-2)" }}>
            <input type="text" value={firmwarePath} readOnly placeholder="No file selected" style={{ flex: 1 }} />
            <button onClick={pickFirmware} style={secondaryBtn}>Browse</button>
          </div>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: mode === "batch" ? "1fr 1fr" : "1fr", gap: "var(--space-4)", marginBottom: "var(--space-2)" }}>
          <div>
            <label style={labelStyle}>Pre-Shared Key (optional)</label>
            <input
              type="password"
              value={psk}
              onChange={(e) => setPsk(e.target.value)}
              placeholder="Leave blank if none"
              style={{ width: "100%" }}
            />
          </div>
          {mode === "batch" && (
            <div>
              <label style={labelStyle}>Update Strategy</label>
              <select value={strategy} onChange={(e) => setStrategy(e.target.value as OtaStrategy)} style={{ width: "100%" }}>
                {(Object.keys(STRATEGY_LABELS) as OtaStrategy[]).map((s) => (
                  <option key={s} value={s}>{STRATEGY_LABELS[s]}</option>
                ))}
              </select>
              <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4, marginBottom: 0 }}>
                {strategy === "sequential" && "Updates nodes one at a time."}
                {strategy === "tdm_safe" && "Respects TDM slots to avoid overlapping transmissions."}
                {strategy === "parallel" && "Updates all nodes simultaneously (fastest, highest network load)."}
              </p>
            </div>
          )}
        </div>
      </div>

      {/* Action */}
      <div style={{ display: "flex", justifyContent: "flex-end", marginBottom: "var(--space-5)" }}>
        {mode === "single" ? (
          <button onClick={startSingleOta} disabled={!canStartSingle} style={canStartSingle ? primaryBtn : disabledBtn}>
            {isSingleUpdating ? "Pushing Update..." : "Push Update"}
          </button>
        ) : (
          <button onClick={startBatchOta} disabled={!canStartBatch} style={canStartBatch ? primaryBtn : disabledBtn}>
            {isBatchUpdating ? "Updating..." : `Start Batch Update (${selectedBatchIps.size} node${selectedBatchIps.size !== 1 ? "s" : ""})`}
          </button>
        )}
      </div>

      {/* Single Result */}
      {mode === "single" && singleResult && (
        <div style={cardStyle}>
          <h2 style={{ ...sectionTitleStyle, marginBottom: "var(--space-3)" }}>Result</h2>
          <div style={bannerStyle(singleResult.success ? "var(--status-online)" : "var(--status-error)")}>
            <div style={{ fontWeight: 600, marginBottom: 4 }}>
              {singleResult.success ? "Update Successful" : "Update Failed"}
            </div>
            <div style={{ fontSize: 12 }}>
              Node: {singleResult.node_ip}
              {singleResult.previous_version && ` | Previous: v${singleResult.previous_version}`}
              {singleResult.new_version && ` | New: v${singleResult.new_version}`}
              {singleResult.duration_ms > 0 && ` | Duration: ${(singleResult.duration_ms / 1000).toFixed(1)}s`}
            </div>
            {singleResult.error && (
              <div style={{ marginTop: 4, fontSize: 12, fontFamily: "var(--font-mono)" }}>
                {singleResult.error}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Batch Progress & Results */}
      {mode === "batch" && batchNodeStates.size > 0 && (
        <div style={cardStyle}>
          <h2 style={{ ...sectionTitleStyle, marginBottom: "var(--space-3)" }}>
            {isBatchUpdating ? "Update Progress" : "Results"}
          </h2>
          <div style={{ border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
            {/* Table header */}
            <div style={tableHeaderRow}>
              <span style={{ ...tableCell, flex: 2 }}>Node IP</span>
              <span style={{ ...tableCell, flex: 2 }}>Status</span>
              <span style={{ ...tableCell, flex: 2 }}>Version</span>
              <span style={{ ...tableCell, flex: 1, textAlign: "right" }}>Duration</span>
            </div>
            {/* Table rows */}
            {Array.from(batchNodeStates.entries()).map(([ip, state]) => {
              const result = batchResults.find((r) => r.node_ip === ip);
              const cfg = STATE_CONFIG[state];
              return (
                <div key={ip} style={tableRow}>
                  <span style={{ ...tableCell, flex: 2, fontFamily: "var(--font-mono)" }}>{ip}</span>
                  <span style={{ ...tableCell, flex: 2 }}>
                    <NodeStateBadge state={state} />
                  </span>
                  <span style={{ ...tableCell, flex: 2, fontSize: 12, color: "var(--text-secondary)" }}>
                    {result?.previous_version && result?.new_version
                      ? `v${result.previous_version} -> v${result.new_version}`
                      : result?.error
                        ? <span style={{ color: "var(--status-error)", fontFamily: "var(--font-mono)", fontSize: 11 }}>{result.error}</span>
                        : "--"}
                  </span>
                  <span style={{ ...tableCell, flex: 1, textAlign: "right", fontFamily: "var(--font-mono)", fontSize: 12, color: "var(--text-muted)" }}>
                    {result && result.duration_ms > 0 ? `${(result.duration_ms / 1000).toFixed(1)}s` : "--"}
                  </span>
                </div>
              );
            })}
          </div>

          {/* Summary */}
          {!isBatchUpdating && batchResults.length > 0 && (
            <div style={{ marginTop: "var(--space-3)", display: "flex", gap: "var(--space-4)", fontSize: 12 }}>
              <span style={{ color: "var(--status-online)" }}>
                {batchResults.filter((r) => r.success).length} succeeded
              </span>
              <span style={{ color: "var(--status-error)" }}>
                {batchResults.filter((r) => !r.success).length} failed
              </span>
              <span style={{ color: "var(--text-muted)" }}>
                {batchResults.length} total
              </span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function TabButton({ label, active, onClick, side }: { label: string; active: boolean; onClick: () => void; side: "left" | "right" }) {
  return (
    <button
      onClick={onClick}
      style={{
        flex: 1,
        padding: "var(--space-2) var(--space-4)",
        fontSize: 13,
        fontWeight: active ? 600 : 400,
        color: active ? "var(--text-primary)" : "var(--text-muted)",
        background: active ? "var(--bg-surface)" : "transparent",
        border: `1px solid ${active ? "var(--border-active)" : "var(--border)"}`,
        borderRadius: side === "left" ? "6px 0 0 6px" : "0 6px 6px 0",
        cursor: "pointer",
        transition: "all 0.15s ease",
      }}
    >
      {label}
    </button>
  );
}

function StatusDot({ health }: { health: string }) {
  const color =
    health === "online" ? "var(--status-online)" :
    health === "degraded" ? "var(--status-warning)" :
    health === "offline" ? "var(--status-error)" :
    "var(--text-muted)";

  return (
    <span
      style={{
        display: "inline-block",
        width: 8,
        height: 8,
        borderRadius: "50%",
        background: color,
        flexShrink: 0,
      }}
    />
  );
}

function NodeStateBadge({ state }: { state: BatchNodeState }) {
  const cfg = STATE_CONFIG[state];
  const isAnimating = state === "uploading" || state === "rebooting" || state === "verifying";
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        fontSize: 12,
        fontWeight: 500,
        color: cfg.color,
      }}
    >
      <span
        style={{
          display: "inline-block",
          width: 8,
          height: 8,
          borderRadius: "50%",
          background: cfg.color,
          animation: isAnimating ? "pulse-accent 1.5s infinite" : "none",
          flexShrink: 0,
        }}
      />
      {cfg.label}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Shared styles
// ---------------------------------------------------------------------------

function bannerStyle(color: string): React.CSSProperties {
  return {
    background: `color-mix(in srgb, ${color} 10%, transparent)`,
    border: `1px solid color-mix(in srgb, ${color} 30%, transparent)`,
    borderRadius: 6,
    padding: "var(--space-3) var(--space-4)",
    marginBottom: "var(--space-4)",
    fontSize: 13,
    color,
  };
}

const cardStyle: React.CSSProperties = {
  background: "var(--bg-surface)",
  border: "1px solid var(--border)",
  borderRadius: 8,
  padding: "var(--space-5)",
};

const sectionTitleStyle: React.CSSProperties = {
  fontSize: 14,
  fontWeight: 600,
  color: "var(--text-primary)",
  margin: 0,
  fontFamily: "var(--font-sans)",
};

const labelStyle: React.CSSProperties = {
  display: "block",
  fontSize: 12,
  fontWeight: 600,
  color: "var(--text-secondary)",
  marginBottom: 6,
  fontFamily: "var(--font-sans)",
};

const primaryBtn: React.CSSProperties = {
  padding: "var(--space-2) 20px",
  borderRadius: 6,
  background: "var(--accent)",
  color: "#fff",
  fontSize: 13,
  fontWeight: 600,
  cursor: "pointer",
};

const secondaryBtn: React.CSSProperties = {
  padding: "var(--space-2) var(--space-4)",
  border: "1px solid var(--border)",
  borderRadius: 6,
  background: "transparent",
  color: "var(--text-secondary)",
  fontSize: 13,
  fontWeight: 500,
  cursor: "pointer",
};

const disabledBtn: React.CSSProperties = {
  ...primaryBtn,
  background: "var(--bg-active)",
  color: "var(--text-muted)",
  cursor: "not-allowed",
};

const linkBtn: React.CSSProperties = {
  background: "none",
  border: "none",
  color: "var(--accent)",
  cursor: "pointer",
  padding: 0,
  fontWeight: 500,
};

const tableHeaderRow: React.CSSProperties = {
  display: "flex",
  padding: "var(--space-2) var(--space-3)",
  background: "var(--bg-base)",
  borderBottom: "1px solid var(--border)",
  fontSize: 11,
  fontWeight: 600,
  color: "var(--text-muted)",
  textTransform: "uppercase",
  letterSpacing: "0.05em",
};

const tableRow: React.CSSProperties = {
  display: "flex",
  padding: "var(--space-2) var(--space-3)",
  borderBottom: "1px solid var(--border)",
  alignItems: "center",
};

const tableCell: React.CSSProperties = {
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  fontSize: 13,
  color: "var(--text-primary)",
};
