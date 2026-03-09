import { useState } from "react";
import { useNodes } from "../hooks/useNodes";
import { StatusBadge } from "../components/StatusBadge";
import type { Node } from "../types";

export function Nodes() {
  const { nodes, isScanning, scan, error } = useNodes({
    pollInterval: 10_000,
    autoScan: true,
  });
  const [expandedMac, setExpandedMac] = useState<string | null>(null);

  const toggleExpand = (node: Node) => {
    const key = node.mac ?? node.ip;
    setExpandedMac((prev) => (prev === key ? null : key));
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
          <h1 className="heading-lg" style={{ margin: 0 }}>Nodes</h1>
          <p style={{ fontSize: 13, color: "var(--text-secondary)", marginTop: "var(--space-1)" }}>
            {nodes.length} node{nodes.length !== 1 ? "s" : ""} in registry
          </p>
        </div>
        <button
          onClick={scan}
          disabled={isScanning}
          style={{
            padding: "var(--space-2) var(--space-4)",
            borderRadius: 6,
            background: isScanning ? "var(--bg-active)" : "var(--accent)",
            color: isScanning ? "var(--text-muted)" : "#fff",
            fontSize: 13,
            fontWeight: 600,
          }}
        >
          {isScanning ? "Scanning..." : "Refresh"}
        </button>
      </div>

      {/* Error */}
      {error && (
        <div
          style={{
            background: "rgba(248, 81, 73, 0.1)",
            border: "1px solid rgba(248, 81, 73, 0.3)",
            borderRadius: 6,
            padding: "var(--space-3) var(--space-4)",
            marginBottom: "var(--space-4)",
            fontSize: 13,
            color: "var(--status-error)",
          }}
        >
          {error}
        </div>
      )}

      {/* Table */}
      {nodes.length === 0 ? (
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
          {isScanning ? "Scanning for nodes..." : "No nodes found. Run a scan to discover ESP32 devices."}
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
                <Th>Status</Th>
                <Th>MAC</Th>
                <Th>IP</Th>
                <Th>Firmware</Th>
                <Th>Chip</Th>
                <Th>Last Seen</Th>
              </tr>
            </thead>
            <tbody>
              {nodes.map((node) => {
                const key = node.mac ?? node.ip;
                return (
                  <NodeRow
                    key={key}
                    node={node}
                    isExpanded={expandedMac === key}
                    onToggle={() => toggleExpand(node)}
                  />
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

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

function formatLastSeen(iso: string): string {
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

function NodeRow({
  node,
  isExpanded,
  onToggle,
}: {
  node: Node;
  isExpanded: boolean;
  onToggle: () => void;
}) {
  return (
    <>
      <tr
        onClick={onToggle}
        style={{
          borderBottom: isExpanded ? "none" : "1px solid var(--border)",
          cursor: "pointer",
          transition: "background 0.1s",
        }}
        onMouseEnter={(e) => (e.currentTarget.style.background = "var(--bg-hover)")}
        onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
      >
        <Td><StatusBadge status={node.health} /></Td>
        <Td mono>{node.mac ?? "--"}</Td>
        <Td mono>{node.ip}</Td>
        <Td mono>{node.firmware_version ?? "--"}</Td>
        <Td>{node.chip?.toUpperCase() ?? "--"}</Td>
        <Td>{formatLastSeen(node.last_seen)}</Td>
      </tr>
      {isExpanded && (
        <tr style={{ borderBottom: "1px solid var(--border)" }}>
          <td colSpan={6} style={{ padding: "0 var(--space-4) var(--space-4)" }}>
            <ExpandedDetails node={node} />
          </td>
        </tr>
      )}
    </>
  );
}

function ExpandedDetails({ node }: { node: Node }) {
  return (
    <div
      style={{
        background: "var(--bg-elevated)",
        borderRadius: 6,
        padding: "var(--space-4)",
        display: "grid",
        gridTemplateColumns: "repeat(auto-fill, minmax(160px, 1fr))",
        gap: "var(--space-3) var(--space-5)",
        fontSize: 12,
      }}
    >
      <DetailField label="Hostname" value={node.hostname ?? "--"} />
      <DetailField label="Node ID" value={String(node.node_id)} mono />
      <DetailField label="Mesh Role" value={node.mesh_role} />
      <DetailField
        label="TDM Slot"
        value={
          node.tdm_slot != null && node.tdm_total != null
            ? `${node.tdm_slot} / ${node.tdm_total}`
            : "--"
        }
        mono
      />
      <DetailField
        label="Edge Tier"
        value={node.edge_tier != null ? String(node.edge_tier) : "--"}
        mono
      />
      <DetailField
        label="Uptime"
        value={
          node.uptime_secs != null
            ? `${Math.floor(node.uptime_secs / 3600)}h ${Math.floor((node.uptime_secs % 3600) / 60)}m`
            : "--"
        }
        mono
      />
      <DetailField label="Discovery" value={node.discovery_method} />
      <DetailField
        label="Capabilities"
        value={
          node.capabilities
            ? Object.entries(node.capabilities)
                .filter(([, v]) => v)
                .map(([k]) => k)
                .join(", ") || "none"
            : "--"
        }
      />
      {node.friendly_name && <DetailField label="Name" value={node.friendly_name} />}
      {node.notes && <DetailField label="Notes" value={node.notes} />}
    </div>
  );
}

function DetailField({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div>
      <div
        style={{
          fontSize: 10,
          textTransform: "uppercase",
          letterSpacing: "0.05em",
          color: "var(--text-muted)",
          marginBottom: 2,
          fontFamily: "var(--font-sans)",
        }}
      >
        {label}
      </div>
      <div style={{ color: "var(--text-secondary)", fontFamily: mono ? "var(--font-mono)" : "var(--font-sans)" }}>
        {value}
      </div>
    </div>
  );
}
