import type { Node } from "../types";
import { StatusBadge } from "./StatusBadge";

interface NodeCardProps {
  node: Node;
  onClick?: (node: Node) => void;
}

function formatUptime(secs: number | null): string {
  if (secs == null) return "--";
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
  return `${Math.floor(secs / 86400)}d ${Math.floor((secs % 86400) / 3600)}h`;
}

function formatLastSeen(iso: string): string {
  try {
    const d = new Date(iso);
    const diffMs = Date.now() - d.getTime();
    if (diffMs < 60_000) return "just now";
    if (diffMs < 3_600_000) return `${Math.floor(diffMs / 60_000)}m ago`;
    if (diffMs < 86_400_000) return `${Math.floor(diffMs / 3_600_000)}h ago`;
    return d.toLocaleDateString();
  } catch {
    return "--";
  }
}

export function NodeCard({ node, onClick }: NodeCardProps) {
  const isOnline = node.health === "online";

  return (
    <div
      onClick={() => onClick?.(node)}
      style={{
        background: "var(--bg-elevated)",
        border: "1px solid var(--border)",
        borderRadius: 8,
        padding: "var(--space-4)",
        cursor: onClick ? "pointer" : "default",
        opacity: isOnline ? 1 : 0.6,
        transition: "border-color 0.15s, background 0.15s",
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.borderColor = "var(--accent)";
        e.currentTarget.style.background = "var(--bg-hover)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.borderColor = "var(--border)";
        e.currentTarget.style.background = "var(--bg-elevated)";
      }}
    >
      {/* Header */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "flex-start",
          marginBottom: "var(--space-3)",
        }}
      >
        <div>
          <div
            style={{
              fontSize: 14,
              fontWeight: 600,
              color: "var(--text-primary)",
              fontFamily: "var(--font-sans)",
              marginBottom: 2,
            }}
          >
            {node.friendly_name || node.hostname || `Node ${node.node_id}`}
          </div>
          <div
            style={{
              fontSize: 12,
              color: "var(--text-secondary)",
              fontFamily: "var(--font-mono)",
            }}
          >
            {node.ip}
          </div>
        </div>
        <StatusBadge status={node.health} />
      </div>

      {/* Details grid */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gap: "var(--space-2) var(--space-4)",
          fontSize: 12,
        }}
      >
        <DetailRow label="MAC" value={node.mac ?? "--"} mono />
        <DetailRow label="Firmware" value={node.firmware_version ?? "--"} mono />
        <DetailRow label="Chip" value={node.chip?.toUpperCase() ?? "--"} />
        <DetailRow label="Role" value={node.mesh_role} />
        <DetailRow
          label="TDM"
          value={
            node.tdm_slot != null && node.tdm_total != null
              ? `${node.tdm_slot}/${node.tdm_total}`
              : "--"
          }
          mono
        />
        <DetailRow
          label="Edge Tier"
          value={node.edge_tier != null ? String(node.edge_tier) : "--"}
        />
        <DetailRow label="Uptime" value={formatUptime(node.uptime_secs)} mono />
        <DetailRow label="Seen" value={formatLastSeen(node.last_seen)} />
      </div>
    </div>
  );
}

function DetailRow({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div>
      <div
        style={{
          color: "var(--text-muted)",
          fontSize: 10,
          textTransform: "uppercase",
          letterSpacing: "0.05em",
          marginBottom: 1,
          fontFamily: "var(--font-sans)",
        }}
      >
        {label}
      </div>
      <div
        style={{
          color: "var(--text-secondary)",
          fontFamily: mono ? "var(--font-mono)" : "var(--font-sans)",
          fontSize: 12,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {value}
      </div>
    </div>
  );
}
