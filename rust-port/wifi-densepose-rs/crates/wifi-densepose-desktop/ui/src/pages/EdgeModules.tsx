import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Node, WasmModule, WasmModuleState } from "../types";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STATE_STYLES: Record<WasmModuleState, { color: string; label: string }> = {
  running: { color: "var(--status-online)", label: "Running" },
  stopped: { color: "var(--status-warning)", label: "Stopped" },
  error: { color: "var(--status-error)", label: "Error" },
  loading: { color: "var(--status-info)", label: "Loading" },
};

// ---------------------------------------------------------------------------
// EdgeModules page
// ---------------------------------------------------------------------------

export function EdgeModules() {
  const [nodes, setNodes] = useState<Node[]>([]);
  const [selectedIp, setSelectedIp] = useState<string>("");
  const [modules, setModules] = useState<WasmModule[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isUploading, setIsUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  // ---- Discover nodes on mount ----
  useEffect(() => {
    (async () => {
      try {
        const discovered = await invoke<Node[]>("discover_nodes", {
          timeoutMs: 5000,
        });
        setNodes(discovered);
        if (discovered.length > 0) {
          setSelectedIp(discovered[0].ip);
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    })();
  }, []);

  // ---- Fetch modules when selected node changes ----
  const fetchModules = useCallback(async (ip: string) => {
    if (!ip) return;
    setIsLoading(true);
    setError(null);
    try {
      const list = await invoke<WasmModule[]>("wasm_list", { nodeIp: ip });
      setModules(list);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setModules([]);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    if (selectedIp) {
      fetchModules(selectedIp);
    }
  }, [selectedIp, fetchModules]);

  // ---- Upload .wasm file ----
  const handleUpload = async () => {
    if (!selectedIp) return;
    const filePath = await open({
      title: "Select WASM Module",
      filters: [{ name: "WASM Modules", extensions: ["wasm"] }],
      multiple: false,
      directory: false,
    });
    if (!filePath) return;

    setIsUploading(true);
    setError(null);
    setSuccess(null);
    try {
      const result = await invoke<{ success: boolean; module_id: string; message: string }>(
        "wasm_upload",
        { nodeIp: selectedIp, wasmPath: filePath },
      );
      if (result.success) {
        setSuccess(`Module uploaded: ${result.module_id}`);
        await fetchModules(selectedIp);
      } else {
        setError(result.message);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsUploading(false);
    }
  };

  // ---- Module actions ----
  const handleAction = async (moduleId: string, action: "start" | "stop" | "unload") => {
    setError(null);
    setSuccess(null);
    try {
      await invoke("wasm_control", {
        nodeIp: selectedIp,
        moduleId,
        action,
      });
      setSuccess(`Module ${moduleId} ${action === "unload" ? "unloaded" : action === "start" ? "started" : "stopped"}`);
      await fetchModules(selectedIp);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div style={{ padding: "var(--space-5)", maxWidth: 1200 }}>
      {/* Header */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: "var(--space-5)",
        }}
      >
        <div>
          <h1 className="heading-lg" style={{ margin: 0 }}>Edge Modules (WASM)</h1>
          <p style={{ fontSize: 13, color: "var(--text-secondary)", marginTop: "var(--space-1)" }}>
            Manage WASM modules deployed to ESP32 nodes
          </p>
        </div>
        <button
          onClick={handleUpload}
          disabled={!selectedIp || isUploading}
          style={{
            padding: "var(--space-2) var(--space-4)",
            borderRadius: 6,
            background: !selectedIp || isUploading ? "var(--bg-active)" : "var(--accent)",
            color: !selectedIp || isUploading ? "var(--text-muted)" : "#fff",
            fontSize: 13,
            fontWeight: 600,
            cursor: !selectedIp || isUploading ? "not-allowed" : "pointer",
            border: "none",
          }}
        >
          {isUploading ? "Uploading..." : "Upload Module"}
        </button>
      </div>

      {/* Node selector */}
      <div style={{ marginBottom: "var(--space-4)" }}>
        <label
          style={{
            fontSize: 10,
            textTransform: "uppercase",
            letterSpacing: "0.05em",
            color: "var(--text-muted)",
            fontFamily: "var(--font-sans)",
            display: "block",
            marginBottom: "var(--space-1)",
          }}
        >
          Target Node
        </label>
        <select
          value={selectedIp}
          onChange={(e) => setSelectedIp(e.target.value)}
          style={{
            padding: "var(--space-2) var(--space-3)",
            borderRadius: 6,
            background: "var(--bg-elevated)",
            color: "var(--text-primary)",
            border: "1px solid var(--border)",
            fontSize: 13,
            fontFamily: "var(--font-mono)",
            minWidth: 260,
            cursor: "pointer",
          }}
        >
          {nodes.length === 0 && <option value="">No nodes discovered</option>}
          {nodes.map((node) => (
            <option key={node.ip} value={node.ip}>
              {node.ip}{node.hostname ? ` (${node.hostname})` : ""}{node.friendly_name ? ` - ${node.friendly_name}` : ""}
            </option>
          ))}
        </select>
      </div>

      {/* Success banner */}
      {success && (
        <Banner
          type="success"
          message={success}
          onDismiss={() => setSuccess(null)}
        />
      )}

      {/* Error banner */}
      {error && (
        <Banner
          type="error"
          message={error}
          onDismiss={() => setError(null)}
        />
      )}

      {/* Module table */}
      {isLoading ? (
        <div
          style={{
            background: "var(--bg-surface)",
            border: "1px solid var(--border)",
            borderRadius: 8,
            padding: "var(--space-8)",
            textAlign: "center",
            color: "var(--text-muted)",
            fontSize: 13,
          }}
        >
          Loading modules...
        </div>
      ) : modules.length === 0 ? (
        <div
          style={{
            background: "var(--bg-surface)",
            border: "1px solid var(--border)",
            borderRadius: 8,
            padding: "var(--space-8)",
            textAlign: "center",
            color: "var(--text-muted)",
            fontSize: 13,
          }}
        >
          {selectedIp
            ? "No WASM modules loaded on this node. Use \"Upload Module\" to deploy one."
            : "Select a node to view its WASM modules."}
        </div>
      ) : (
        <div
          style={{
            background: "var(--bg-surface)",
            border: "1px solid var(--border)",
            borderRadius: 8,
            overflow: "hidden",
          }}
        >
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13 }}>
            <thead>
              <tr style={{ borderBottom: "1px solid var(--border)", textAlign: "left" }}>
                <Th>Name</Th>
                <Th>Size</Th>
                <Th>Status</Th>
                <Th>Loaded At</Th>
                <Th>Actions</Th>
              </tr>
            </thead>
            <tbody>
              {modules.map((mod) => (
                <ModuleRow
                  key={mod.module_id}
                  module={mod}
                  onAction={handleAction}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function Th({ children }: { children: React.ReactNode }) {
  return (
    <th
      style={{
        padding: "10px var(--space-4)",
        fontSize: 10,
        fontWeight: 600,
        textTransform: "uppercase",
        letterSpacing: "0.05em",
        color: "var(--text-muted)",
        fontFamily: "var(--font-sans)",
      }}
    >
      {children}
    </th>
  );
}

function Td({ children, mono = false }: { children: React.ReactNode; mono?: boolean }) {
  return (
    <td
      style={{
        padding: "10px var(--space-4)",
        color: "var(--text-secondary)",
        fontFamily: mono ? "var(--font-mono)" : "var(--font-sans)",
        whiteSpace: "nowrap",
        fontSize: 13,
      }}
    >
      {children}
    </td>
  );
}

function ModuleStateBadge({ state }: { state: WasmModuleState }) {
  const { color, label } = STATE_STYLES[state];
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        color,
        fontSize: 11,
        fontWeight: 600,
        fontFamily: "var(--font-sans)",
        padding: "2px 8px",
        borderRadius: 9999,
        lineHeight: 1,
        whiteSpace: "nowrap",
        background: "rgba(255, 255, 255, 0.04)",
      }}
    >
      <span
        style={{
          width: 6,
          height: 6,
          borderRadius: "50%",
          backgroundColor: color,
          flexShrink: 0,
        }}
      />
      {label}
    </span>
  );
}

function ActionButton({
  label,
  onClick,
  variant = "default",
}: {
  label: string;
  onClick: () => void;
  variant?: "default" | "danger";
}) {
  const isDanger = variant === "danger";
  return (
    <button
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      style={{
        padding: "3px 10px",
        borderRadius: 4,
        fontSize: 11,
        fontWeight: 600,
        fontFamily: "var(--font-sans)",
        border: `1px solid ${isDanger ? "var(--status-error)" : "var(--border)"}`,
        background: "transparent",
        color: isDanger ? "var(--status-error)" : "var(--text-secondary)",
        cursor: "pointer",
        transition: "background 0.1s",
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.background = isDanger
          ? "rgba(248, 81, 73, 0.1)"
          : "var(--bg-hover)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = "transparent";
      }}
    >
      {label}
    </button>
  );
}

function ModuleRow({
  module: mod,
  onAction,
}: {
  module: WasmModule;
  onAction: (moduleId: string, action: "start" | "stop" | "unload") => void;
}) {
  return (
    <tr
      style={{
        borderBottom: "1px solid var(--border)",
        transition: "background 0.1s",
      }}
      onMouseEnter={(e) => (e.currentTarget.style.background = "var(--bg-hover)")}
      onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
    >
      <Td mono>{mod.name}</Td>
      <Td mono>{formatBytes(mod.size_bytes)}</Td>
      <Td><ModuleStateBadge state={mod.state} /></Td>
      <Td>{formatLoadedAt(mod.loaded_at)}</Td>
      <td style={{ padding: "10px var(--space-4)", whiteSpace: "nowrap" }}>
        <div style={{ display: "flex", gap: "var(--space-2)" }}>
          {mod.state === "stopped" && (
            <ActionButton label="Start" onClick={() => onAction(mod.module_id, "start")} />
          )}
          {mod.state === "running" && (
            <ActionButton label="Stop" onClick={() => onAction(mod.module_id, "stop")} />
          )}
          <ActionButton
            label="Unload"
            onClick={() => onAction(mod.module_id, "unload")}
            variant="danger"
          />
        </div>
      </td>
    </tr>
  );
}

function Banner({
  type,
  message,
  onDismiss,
}: {
  type: "error" | "success";
  message: string;
  onDismiss: () => void;
}) {
  const isError = type === "error";
  const color = isError ? "var(--status-error)" : "var(--status-online)";
  const bgAlpha = isError ? "rgba(248, 81, 73, 0.1)" : "rgba(63, 185, 80, 0.1)";
  const borderAlpha = isError ? "rgba(248, 81, 73, 0.3)" : "rgba(63, 185, 80, 0.3)";

  return (
    <div
      style={{
        background: bgAlpha,
        border: `1px solid ${borderAlpha}`,
        borderRadius: 6,
        padding: "var(--space-3) var(--space-4)",
        marginBottom: "var(--space-4)",
        fontSize: 13,
        color,
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
      }}
    >
      <span>{message}</span>
      <button
        onClick={onDismiss}
        style={{
          background: "none",
          border: "none",
          color,
          cursor: "pointer",
          fontSize: 16,
          lineHeight: 1,
          padding: "0 0 0 var(--space-3)",
        }}
      >
        x
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(1)} KB`;
  const mb = kb / 1024;
  return `${mb.toFixed(2)} MB`;
}

function formatLoadedAt(iso: string | null): string {
  if (!iso) return "--";
  try {
    const d = new Date(iso);
    const diff = Date.now() - d.getTime();
    if (diff < 60_000) return "just now";
    if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
    if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
    return d.toLocaleDateString();
  } catch {
    return "--";
  }
}
