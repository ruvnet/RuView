import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { StatusBadge } from "../components/StatusBadge";
import type { HealthStatus, Chip, MeshRole, DiscoveryMethod } from "../types";

interface DiscoveredNode {
  ip: string;
  mac: string | null;
  hostname: string | null;
  node_id: number;
  firmware_version: string | null;
  health: HealthStatus;
  last_seen: string;
  chip: Chip;
  mesh_role: MeshRole;
  discovery_method: DiscoveryMethod;
  tdm_slot: number | null;
  tdm_total: number | null;
  edge_tier: number | null;
  uptime_secs: number | null;
  capabilities: { wasm: boolean; ota: boolean; csi: boolean } | null;
  friendly_name: string | null;
  notes: string | null;
}

interface SerialPortInfo {
  name: string;
  vid: number | null;
  pid: number | null;
  manufacturer: string | null;
  serial_number: string | null;
  is_esp32_compatible: boolean;
}

type DiscoveryTab = "network" | "serial" | "manual";

const NetworkDiscovery: React.FC = () => {
  const [activeTab, setActiveTab] = useState<DiscoveryTab>("network");
  const [nodes, setNodes] = useState<DiscoveredNode[]>([]);
  const [serialPorts, setSerialPorts] = useState<SerialPortInfo[]>([]);
  const [isScanning, setIsScanning] = useState(false);
  const [scanDuration, setScanDuration] = useState(3000);
  const [error, setError] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<DiscoveredNode | null>(null);
  const [filterOnline, setFilterOnline] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  // Manual add state
  const [manualIp, setManualIp] = useState("");
  const [manualMac, setManualMac] = useState("");
  const [addingManual, setAddingManual] = useState(false);

  const scanNetwork = useCallback(async () => {
    setIsScanning(true);
    setError(null);
    try {
      const found = await invoke<DiscoveredNode[]>("discover_nodes", {
        timeoutMs: scanDuration,
      });
      setNodes(found);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsScanning(false);
    }
  }, [scanDuration]);

  const scanSerialPorts = useCallback(async () => {
    setIsScanning(true);
    setError(null);
    try {
      const ports = await invoke<SerialPortInfo[]>("list_serial_ports");
      setSerialPorts(ports);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsScanning(false);
    }
  }, []);

  const addManualNode = useCallback(async () => {
    if (!manualIp.trim()) return;
    setAddingManual(true);
    setError(null);
    try {
      // Try to ping or probe the node
      const newNode: DiscoveredNode = {
        ip: manualIp.trim(),
        mac: manualMac.trim() || null,
        hostname: null,
        node_id: 0,
        firmware_version: null,
        health: "unknown" as HealthStatus,
        last_seen: new Date().toISOString(),
        chip: "esp32" as Chip,
        mesh_role: "node" as MeshRole,
        discovery_method: "manual" as DiscoveryMethod,
        tdm_slot: null,
        tdm_total: null,
        edge_tier: null,
        uptime_secs: null,
        capabilities: null,
        friendly_name: null,
        notes: "Manually added",
      };
      setNodes((prev) => [...prev.filter((n) => n.ip !== newNode.ip), newNode]);
      setManualIp("");
      setManualMac("");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setAddingManual(false);
    }
  }, [manualIp, manualMac]);

  useEffect(() => {
    scanNetwork();
  }, []);

  useEffect(() => {
    if (activeTab === "serial") {
      scanSerialPorts();
    }
  }, [activeTab, scanSerialPorts]);

  const filteredNodes = nodes.filter((node) => {
    if (filterOnline && node.health !== "online") return false;
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      return (
        node.ip.toLowerCase().includes(q) ||
        (node.mac?.toLowerCase().includes(q) ?? false) ||
        (node.hostname?.toLowerCase().includes(q) ?? false) ||
        (node.friendly_name?.toLowerCase().includes(q) ?? false)
      );
    }
    return true;
  });

  const onlineCount = nodes.filter((n) => n.health === "online").length;

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
          <h1 className="heading-lg" style={{ margin: 0 }}>
            Network Discovery
          </h1>
          <p
            style={{
              fontSize: 13,
              color: "var(--text-secondary)",
              marginTop: 4,
            }}
          >
            Discover and manage ESP32 CSI nodes on your network
          </p>
        </div>
      </div>

      {/* Stats Row */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(4, 1fr)",
          gap: "var(--space-4)",
          marginBottom: "var(--space-5)",
        }}
      >
        <StatCard label="Total Nodes" value={nodes.length} />
        <StatCard label="Online" value={onlineCount} color="var(--status-online)" />
        <StatCard
          label="Offline"
          value={nodes.length - onlineCount}
          color={nodes.length - onlineCount > 0 ? "var(--status-error)" : "var(--text-muted)"}
        />
        <StatCard label="Serial Ports" value={serialPorts.filter((p) => p.is_esp32_compatible).length} />
      </div>

      {/* Tabs */}
      <div
        style={{
          display: "flex",
          gap: "var(--space-2)",
          borderBottom: "1px solid var(--border)",
          marginBottom: "var(--space-4)",
        }}
      >
        <TabButton active={activeTab === "network"} onClick={() => setActiveTab("network")}>
          Network Discovery
        </TabButton>
        <TabButton active={activeTab === "serial"} onClick={() => setActiveTab("serial")}>
          Serial Ports
        </TabButton>
        <TabButton active={activeTab === "manual"} onClick={() => setActiveTab("manual")}>
          Manual Add
        </TabButton>
      </div>

      {/* Error Display */}
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

      {/* Network Tab */}
      {activeTab === "network" && (
        <>
          {/* Controls */}
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              marginBottom: "var(--space-4)",
              gap: "var(--space-4)",
            }}
          >
            <div style={{ display: "flex", gap: "var(--space-3)", alignItems: "center" }}>
              <input
                type="text"
                placeholder="Search nodes..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                style={{
                  padding: "8px 12px",
                  borderRadius: 6,
                  border: "1px solid var(--border)",
                  background: "var(--bg-surface)",
                  color: "var(--text-primary)",
                  fontSize: 13,
                  width: 200,
                }}
              />
              <label
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  fontSize: 13,
                  color: "var(--text-secondary)",
                  cursor: "pointer",
                }}
              >
                <input
                  type="checkbox"
                  checked={filterOnline}
                  onChange={(e) => setFilterOnline(e.target.checked)}
                />
                Online only
              </label>
            </div>
            <div style={{ display: "flex", gap: "var(--space-3)", alignItems: "center" }}>
              <select
                value={scanDuration}
                onChange={(e) => setScanDuration(Number(e.target.value))}
                style={{
                  padding: "8px 12px",
                  borderRadius: 6,
                  border: "1px solid var(--border)",
                  background: "var(--bg-surface)",
                  color: "var(--text-primary)",
                  fontSize: 13,
                }}
              >
                <option value={1000}>1s scan</option>
                <option value={3000}>3s scan</option>
                <option value={5000}>5s scan</option>
                <option value={10000}>10s scan</option>
              </select>
              <button
                onClick={scanNetwork}
                disabled={isScanning}
                className="btn-gradient"
                style={{ opacity: isScanning ? 0.6 : 1 }}
              >
                {isScanning ? "Scanning..." : "Scan Network"}
              </button>
            </div>
          </div>

          {/* Nodes Grid */}
          {filteredNodes.length === 0 ? (
            <div className="card empty-state">
              <div className="empty-state-icon">{"◉"}</div>
              <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-secondary)" }}>
                {isScanning ? "Scanning for nodes..." : "No nodes discovered"}
              </div>
              <div
                style={{
                  fontSize: 13,
                  color: "var(--text-muted)",
                  maxWidth: 300,
                  textAlign: "center",
                  lineHeight: 1.5,
                }}
              >
                {isScanning
                  ? "Please wait while we search for ESP32 devices on your network."
                  : "Click 'Scan Network' to discover ESP32 devices using mDNS and UDP broadcast."}
              </div>
            </div>
          ) : (
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(auto-fill, minmax(340px, 1fr))",
                gap: "var(--space-4)",
              }}
            >
              {filteredNodes.map((node, i) => (
                <NodeCard
                  key={node.mac || node.ip || i}
                  node={node}
                  onClick={() => setSelectedNode(node)}
                />
              ))}
            </div>
          )}
        </>
      )}

      {/* Serial Tab */}
      {activeTab === "serial" && (
        <>
          <div
            style={{
              display: "flex",
              justifyContent: "flex-end",
              marginBottom: "var(--space-4)",
            }}
          >
            <button
              onClick={scanSerialPorts}
              disabled={isScanning}
              className="btn-gradient"
              style={{ opacity: isScanning ? 0.6 : 1 }}
            >
              {isScanning ? "Scanning..." : "Refresh Ports"}
            </button>
          </div>

          {serialPorts.length === 0 ? (
            <div className="card empty-state">
              <div className="empty-state-icon">{"⌁"}</div>
              <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-secondary)" }}>
                No serial ports found
              </div>
              <div style={{ fontSize: 13, color: "var(--text-muted)" }}>
                Connect an ESP32 device via USB to see available ports.
              </div>
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
                    <Th>Port</Th>
                    <Th>Manufacturer</Th>
                    <Th>VID:PID</Th>
                    <Th>Compatible</Th>
                  </tr>
                </thead>
                <tbody>
                  {serialPorts.map((port) => (
                    <tr
                      key={port.name}
                      style={{ borderBottom: "1px solid var(--border)" }}
                    >
                      <Td mono>{port.name}</Td>
                      <Td>{port.manufacturer || "--"}</Td>
                      <Td mono>
                        {port.vid && port.pid
                          ? `${port.vid.toString(16).padStart(4, "0").toUpperCase()}:${port.pid.toString(16).padStart(4, "0").toUpperCase()}`
                          : "--"}
                      </Td>
                      <Td>
                        {port.is_esp32_compatible ? (
                          <span
                            style={{
                              background: "rgba(63, 185, 80, 0.15)",
                              color: "var(--status-online)",
                              padding: "2px 8px",
                              borderRadius: 4,
                              fontSize: 11,
                              fontWeight: 600,
                            }}
                          >
                            ESP32 Compatible
                          </span>
                        ) : (
                          <span style={{ color: "var(--text-muted)" }}>--</span>
                        )}
                      </Td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}

      {/* Manual Tab */}
      {activeTab === "manual" && (
        <div className="card" style={{ maxWidth: 500 }}>
          <h3 className="heading-sm" style={{ marginBottom: "var(--space-4)" }}>
            Add Node Manually
          </h3>
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-3)" }}>
            <div>
              <label
                style={{
                  display: "block",
                  fontSize: 12,
                  fontWeight: 600,
                  color: "var(--text-secondary)",
                  marginBottom: 4,
                }}
              >
                IP Address *
              </label>
              <input
                type="text"
                placeholder="192.168.1.100"
                value={manualIp}
                onChange={(e) => setManualIp(e.target.value)}
                style={{
                  width: "100%",
                  padding: "10px 12px",
                  borderRadius: 6,
                  border: "1px solid var(--border)",
                  background: "var(--bg-base)",
                  color: "var(--text-primary)",
                  fontSize: 13,
                  fontFamily: "var(--font-mono)",
                }}
              />
            </div>
            <div>
              <label
                style={{
                  display: "block",
                  fontSize: 12,
                  fontWeight: 600,
                  color: "var(--text-secondary)",
                  marginBottom: 4,
                }}
              >
                MAC Address (optional)
              </label>
              <input
                type="text"
                placeholder="AA:BB:CC:DD:EE:FF"
                value={manualMac}
                onChange={(e) => setManualMac(e.target.value)}
                style={{
                  width: "100%",
                  padding: "10px 12px",
                  borderRadius: 6,
                  border: "1px solid var(--border)",
                  background: "var(--bg-base)",
                  color: "var(--text-primary)",
                  fontSize: 13,
                  fontFamily: "var(--font-mono)",
                }}
              />
            </div>
            <button
              onClick={addManualNode}
              disabled={!manualIp.trim() || addingManual}
              className="btn-gradient"
              style={{ marginTop: "var(--space-2)", opacity: !manualIp.trim() ? 0.5 : 1 }}
            >
              {addingManual ? "Adding..." : "Add Node"}
            </button>
          </div>
        </div>
      )}

      {/* Node Detail Modal */}
      {selectedNode && (
        <NodeDetailModal node={selectedNode} onClose={() => setSelectedNode(null)} />
      )}
    </div>
  );
};

function StatCard({
  label,
  value,
  color,
}: {
  label: string;
  value: number;
  color?: string;
}) {
  return (
    <div className="card-glow" style={{ padding: "var(--space-4)" }}>
      <div
        style={{
          fontSize: 10,
          textTransform: "uppercase",
          letterSpacing: "0.06em",
          color: "var(--text-muted)",
          marginBottom: "var(--space-2)",
          fontWeight: 600,
        }}
      >
        {label}
      </div>
      <div
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 28,
          fontWeight: 600,
          color: color || "var(--text-primary)",
          letterSpacing: "-0.02em",
        }}
      >
        {value}
      </div>
    </div>
  );
}

function TabButton({
  children,
  active,
  onClick,
}: {
  children: React.ReactNode;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        padding: "10px 16px",
        border: "none",
        background: "transparent",
        color: active ? "var(--accent)" : "var(--text-secondary)",
        fontSize: 13,
        fontWeight: 600,
        cursor: "pointer",
        borderBottom: active ? "2px solid var(--accent)" : "2px solid transparent",
        marginBottom: -1,
        transition: "color 0.15s, border-color 0.15s",
      }}
    >
      {children}
    </button>
  );
}

function NodeCard({ node, onClick }: { node: DiscoveredNode; onClick: () => void }) {
  const chipColors: Record<string, string> = {
    esp32: "#4CAF50",
    esp32s2: "#2196F3",
    esp32s3: "#9C27B0",
    esp32c3: "#FF9800",
    esp32c6: "#E91E63",
  };

  return (
    <div
      className="card"
      onClick={onClick}
      style={{
        padding: "var(--space-4)",
        cursor: "pointer",
        opacity: node.health === "online" ? 1 : 0.7,
        transition: "transform 0.1s, box-shadow 0.1s",
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.transform = "translateY(-2px)";
        e.currentTarget.style.boxShadow = "0 4px 12px rgba(0,0,0,0.15)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.transform = "translateY(0)";
        e.currentTarget.style.boxShadow = "none";
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "start",
          marginBottom: "var(--space-3)",
        }}
      >
        <div>
          <div style={{ fontWeight: 600, fontSize: 14, marginBottom: 2 }}>
            {node.friendly_name || node.hostname || `Node ${node.node_id}`}
          </div>
          <div className="mono" style={{ fontSize: 12, color: "var(--text-muted)" }}>
            {node.ip}
          </div>
        </div>
        <StatusBadge status={node.health} />
      </div>

      <div
        style={{
          display: "flex",
          gap: "var(--space-2)",
          flexWrap: "wrap",
          marginBottom: "var(--space-3)",
        }}
      >
        <ChipBadge label={node.chip.toUpperCase()} color={chipColors[node.chip] || "#666"} />
        <ChipBadge label={node.mesh_role} color="var(--text-muted)" />
        {node.tdm_slot != null && node.tdm_total != null && (
          <ChipBadge label={`TDM ${node.tdm_slot}/${node.tdm_total}`} color="var(--accent)" />
        )}
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px 16px", fontSize: 12 }}>
        <KV label="MAC" value={node.mac || "--"} mono />
        <KV label="Firmware" value={node.firmware_version || "--"} mono />
        <KV label="Discovery" value={node.discovery_method} />
        {node.uptime_secs && (
          <KV label="Uptime" value={formatUptime(node.uptime_secs)} mono />
        )}
      </div>
    </div>
  );
}

function ChipBadge({ label, color }: { label: string; color: string }) {
  return (
    <span
      style={{
        padding: "2px 8px",
        borderRadius: 4,
        fontSize: 10,
        fontWeight: 600,
        background: `${color}20`,
        color: color,
        textTransform: "uppercase",
      }}
    >
      {label}
    </span>
  );
}

function KV({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
      <span style={{ color: "var(--text-muted)", fontSize: 11 }}>{label}</span>
      <span className={mono ? "mono" : ""} style={{ color: "var(--text-secondary)", fontSize: 12 }}>
        {value}
      </span>
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
        fontSize: 13,
      }}
    >
      {children}
    </td>
  );
}

function formatUptime(secs: number): string {
  const hours = Math.floor(secs / 3600);
  const mins = Math.floor((secs % 3600) / 60);
  if (hours > 0) return `${hours}h ${mins}m`;
  return `${mins}m`;
}

function NodeDetailModal({
  node,
  onClose,
}: {
  node: DiscoveredNode;
  onClose: () => void;
}) {
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.6)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
        padding: "var(--space-5)",
      }}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        style={{
          background: "var(--bg-surface)",
          borderRadius: 12,
          padding: "var(--space-5)",
          maxWidth: 600,
          width: "100%",
          maxHeight: "80vh",
          overflow: "auto",
          border: "1px solid var(--border)",
        }}
      >
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "start",
            marginBottom: "var(--space-4)",
          }}
        >
          <div>
            <h2 className="heading-md" style={{ margin: 0 }}>
              {node.friendly_name || node.hostname || `Node ${node.node_id}`}
            </h2>
            <p className="mono" style={{ color: "var(--text-muted)", marginTop: 4, fontSize: 13 }}>
              {node.ip}
            </p>
          </div>
          <button
            onClick={onClose}
            style={{
              background: "none",
              border: "none",
              fontSize: 20,
              cursor: "pointer",
              color: "var(--text-muted)",
              padding: 4,
            }}
          >
            ×
          </button>
        </div>

        <div style={{ display: "flex", gap: "var(--space-2)", marginBottom: "var(--space-4)" }}>
          <StatusBadge status={node.health} />
          <ChipBadge label={node.chip.toUpperCase()} color="#4CAF50" />
          <ChipBadge label={node.mesh_role} color="var(--accent)" />
        </div>

        <div
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 1fr",
            gap: "var(--space-4)",
          }}
        >
          <DetailSection title="Network">
            <DetailRow label="IP Address" value={node.ip} mono />
            <DetailRow label="MAC Address" value={node.mac || "--"} mono />
            <DetailRow label="Hostname" value={node.hostname || "--"} />
          </DetailSection>

          <DetailSection title="Hardware">
            <DetailRow label="Chip" value={node.chip.toUpperCase()} />
            <DetailRow label="Firmware" value={node.firmware_version || "--"} mono />
            <DetailRow label="Node ID" value={String(node.node_id)} mono />
          </DetailSection>

          <DetailSection title="Mesh Configuration">
            <DetailRow label="Role" value={node.mesh_role} />
            <DetailRow
              label="TDM Slot"
              value={
                node.tdm_slot != null && node.tdm_total != null
                  ? `${node.tdm_slot} / ${node.tdm_total}`
                  : "--"
              }
              mono
            />
            <DetailRow label="Edge Tier" value={node.edge_tier != null ? String(node.edge_tier) : "--"} mono />
          </DetailSection>

          <DetailSection title="Status">
            <DetailRow label="Discovery" value={node.discovery_method} />
            <DetailRow label="Uptime" value={node.uptime_secs ? formatUptime(node.uptime_secs) : "--"} mono />
            <DetailRow label="Last Seen" value={formatLastSeen(node.last_seen)} />
          </DetailSection>
        </div>

        {node.capabilities && (
          <div style={{ marginTop: "var(--space-4)" }}>
            <h4 style={{ fontSize: 12, textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--text-muted)", marginBottom: "var(--space-2)" }}>
              Capabilities
            </h4>
            <div style={{ display: "flex", gap: "var(--space-2)" }}>
              {node.capabilities.csi && <CapabilityBadge label="CSI" enabled />}
              {node.capabilities.ota && <CapabilityBadge label="OTA" enabled />}
              {node.capabilities.wasm && <CapabilityBadge label="WASM" enabled />}
            </div>
          </div>
        )}

        {node.notes && (
          <div style={{ marginTop: "var(--space-4)" }}>
            <h4 style={{ fontSize: 12, textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--text-muted)", marginBottom: "var(--space-2)" }}>
              Notes
            </h4>
            <p style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5 }}>
              {node.notes}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

function DetailSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <h4
        style={{
          fontSize: 12,
          textTransform: "uppercase",
          letterSpacing: "0.05em",
          color: "var(--text-muted)",
          marginBottom: "var(--space-2)",
        }}
      >
        {title}
      </h4>
      <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
        {children}
      </div>
    </div>
  );
}

function DetailRow({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", fontSize: 13 }}>
      <span style={{ color: "var(--text-muted)" }}>{label}</span>
      <span className={mono ? "mono" : ""} style={{ color: "var(--text-primary)" }}>
        {value}
      </span>
    </div>
  );
}

function CapabilityBadge({ label, enabled }: { label: string; enabled: boolean }) {
  return (
    <span
      style={{
        padding: "4px 10px",
        borderRadius: 4,
        fontSize: 11,
        fontWeight: 600,
        background: enabled ? "rgba(63, 185, 80, 0.15)" : "rgba(139, 148, 158, 0.15)",
        color: enabled ? "var(--status-online)" : "var(--text-muted)",
      }}
    >
      {label}
    </span>
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

export default NetworkDiscovery;
