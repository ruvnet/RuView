import { useState, useRef, useEffect, useCallback } from "react";
import type { HealthStatus } from "../types";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface DiscoveredNode {
  ip: string;
  mac: string | null;
  hostname: string | null;
  node_id: number;
  firmware_version: string | null;
  health: HealthStatus;
  last_seen: string;
}

interface SimNode {
  id: number;
  label: string;
  ip: string;
  mac: string | null;
  firmware: string | null;
  health: HealthStatus;
  isCoordinator: boolean;
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  tdmSlot: number;
}

interface SimEdge {
  source: number; // index into nodes
  target: number;
  strength: number; // 0.3 - 1.0 opacity
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const CANVAS_HEIGHT = 500;
const REPULSION = 8000;
const SPRING_K = 0.005;
const SPRING_REST = 120;
const DAMPING = 0.92;
const VELOCITY_THRESHOLD = 0.15;
const DT = 1;

const HEALTH_COLORS: Record<HealthStatus, string> = {
  online: "#3fb950",
  offline: "#f85149",
  degraded: "#d29922",
  unknown: "#8b949e",
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function buildGraph(
  rawNodes: DiscoveredNode[],
  canvasWidth: number,
): { nodes: SimNode[]; edges: SimEdge[] } {
  const cx = canvasWidth / 2;
  const cy = CANVAS_HEIGHT / 2;

  const nodes: SimNode[] = rawNodes.map((n, i) => {
    const isCoord = n.node_id === 0 || i === 0;
    const angle = (2 * Math.PI * i) / Math.max(rawNodes.length, 1);
    const spread = Math.min(canvasWidth, CANVAS_HEIGHT) * 0.3;
    return {
      id: n.node_id,
      label: n.hostname || `Node ${n.node_id}`,
      ip: n.ip,
      mac: n.mac,
      firmware: n.firmware_version,
      health: n.health,
      isCoordinator: isCoord,
      x: cx + Math.cos(angle) * spread + (Math.random() - 0.5) * 20,
      y: cy + Math.sin(angle) * spread + (Math.random() - 0.5) * 20,
      vx: 0,
      vy: 0,
      radius: isCoord ? 30 : 20,
      tdmSlot: i,
    };
  });

  const edges: SimEdge[] = [];
  const coordIdx = 0;

  for (let i = 1; i < nodes.length; i++) {
    // Connect every node to coordinator
    edges.push({
      source: coordIdx,
      target: i,
      strength: 0.3 + Math.random() * 0.7,
    });
    // Connect to next neighbor (ring)
    if (i < nodes.length - 1) {
      edges.push({
        source: i,
        target: i + 1,
        strength: 0.3 + Math.random() * 0.7,
      });
    }
  }
  // Close the ring if 3+ non-coordinator nodes
  if (nodes.length > 3) {
    edges.push({
      source: nodes.length - 1,
      target: 1,
      strength: 0.3 + Math.random() * 0.7,
    });
  }

  return { nodes, edges };
}

function hitTest(
  mx: number,
  my: number,
  nodes: SimNode[],
): SimNode | null {
  // Iterate in reverse so topmost (last-drawn) wins
  for (let i = nodes.length - 1; i >= 0; i--) {
    const n = nodes[i];
    const dx = mx - n.x;
    const dy = my - n.y;
    if (dx * dx + dy * dy <= n.radius * n.radius) {
      return n;
    }
  }
  return null;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function MeshView() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [canvasWidth, setCanvasWidth] = useState(800);
  const [nodes, setNodes] = useState<DiscoveredNode[]>([]);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<SimNode | null>(null);

  // Track simulation data in a ref so the animation loop can read it without
  // re-renders triggering a new effect.
  const simRef = useRef<{ nodes: SimNode[]; edges: SimEdge[] }>({
    nodes: [],
    edges: [],
  });
  const animRef = useRef<number>(0);

  // -----------------------------------------------------------------------
  // Fetch nodes from Rust backend
  // -----------------------------------------------------------------------
  const fetchNodes = useCallback(async () => {
    setScanning(true);
    setError(null);
    setSelectedNode(null);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const found = await invoke<DiscoveredNode[]>("discover_nodes", {
        timeoutMs: 3000,
      });
      setNodes(found);
    } catch (err) {
      console.error("Discovery failed:", err);
      setError(String(err));
    } finally {
      setScanning(false);
    }
  }, []);

  useEffect(() => {
    fetchNodes();
  }, [fetchNodes]);

  // -----------------------------------------------------------------------
  // Measure container width
  // -----------------------------------------------------------------------
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const measure = () => {
      const w = el.clientWidth;
      if (w > 0) setCanvasWidth(w);
    };
    measure();

    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // -----------------------------------------------------------------------
  // Build graph + run force simulation whenever nodes or width change
  // -----------------------------------------------------------------------
  useEffect(() => {
    if (nodes.length === 0) {
      simRef.current = { nodes: [], edges: [] };
      // Clear canvas
      const ctx = canvasRef.current?.getContext("2d");
      if (ctx) {
        ctx.clearRect(0, 0, canvasWidth, CANVAS_HEIGHT);
      }
      return;
    }

    const { nodes: simNodes, edges } = buildGraph(nodes, canvasWidth);
    simRef.current = { nodes: simNodes, edges };

    let settled = false;

    const step = () => {
      const sn = simRef.current.nodes;
      const se = simRef.current.edges;

      // Coulomb repulsion
      for (let i = 0; i < sn.length; i++) {
        for (let j = i + 1; j < sn.length; j++) {
          let dx = sn[j].x - sn[i].x;
          let dy = sn[j].y - sn[i].y;
          let dist = Math.sqrt(dx * dx + dy * dy);
          if (dist < 1) dist = 1;
          const force = REPULSION / (dist * dist);
          const fx = (dx / dist) * force;
          const fy = (dy / dist) * force;
          sn[i].vx -= fx;
          sn[i].vy -= fy;
          sn[j].vx += fx;
          sn[j].vy += fy;
        }
      }

      // Spring attraction along edges
      for (const e of se) {
        const a = sn[e.source];
        const b = sn[e.target];
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        let dist = Math.sqrt(dx * dx + dy * dy);
        if (dist < 1) dist = 1;
        const displacement = dist - SPRING_REST;
        const force = SPRING_K * displacement;
        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;
        a.vx += fx;
        a.vy += fy;
        b.vx -= fx;
        b.vy -= fy;
      }

      // Integrate + damp + clamp to canvas bounds
      let maxV = 0;
      for (const n of sn) {
        n.vx *= DAMPING;
        n.vy *= DAMPING;
        n.x += n.vx * DT;
        n.y += n.vy * DT;

        // Keep nodes within canvas with padding
        const pad = n.radius + 10;
        if (n.x < pad) { n.x = pad; n.vx = 0; }
        if (n.x > canvasWidth - pad) { n.x = canvasWidth - pad; n.vx = 0; }
        if (n.y < pad) { n.y = pad; n.vy = 0; }
        if (n.y > CANVAS_HEIGHT - pad) { n.y = CANVAS_HEIGHT - pad; n.vy = 0; }

        const v = Math.sqrt(n.vx * n.vx + n.vy * n.vy);
        if (v > maxV) maxV = v;
      }

      if (maxV < VELOCITY_THRESHOLD) settled = true;
    };

    const draw = () => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;

      const sn = simRef.current.nodes;
      const se = simRef.current.edges;

      ctx.clearRect(0, 0, canvasWidth, CANVAS_HEIGHT);

      // Edges
      for (const e of se) {
        const a = sn[e.source];
        const b = sn[e.target];
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.strokeStyle = `rgba(139, 148, 158, ${e.strength * 0.6})`;
        ctx.lineWidth = 1.5;
        ctx.stroke();
      }

      // Nodes
      for (const n of sn) {
        const color = HEALTH_COLORS[n.health] || HEALTH_COLORS.unknown;

        // Coordinator ring
        if (n.isCoordinator) {
          ctx.beginPath();
          ctx.arc(n.x, n.y, n.radius + 5, 0, Math.PI * 2);
          ctx.strokeStyle = color;
          ctx.lineWidth = 2;
          ctx.stroke();
        }

        // Node circle
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius, 0, Math.PI * 2);
        ctx.fillStyle = color;
        ctx.globalAlpha = n.health === "offline" ? 0.45 : 0.85;
        ctx.fill();
        ctx.globalAlpha = 1;

        // Selected highlight
        if (selectedNode && selectedNode.id === n.id) {
          ctx.beginPath();
          ctx.arc(n.x, n.y, n.radius + 3, 0, Math.PI * 2);
          ctx.strokeStyle = "#ffffff";
          ctx.lineWidth = 2;
          ctx.stroke();
        }

        // Node ID text inside circle
        ctx.fillStyle = "#ffffff";
        ctx.font = "bold 11px sans-serif";
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(String(n.id), n.x, n.y);

        // Label below
        ctx.fillStyle = "#8b949e";
        ctx.font = "11px sans-serif";
        ctx.textBaseline = "top";
        ctx.fillText(n.label, n.x, n.y + n.radius + 6);
      }
    };

    const tick = () => {
      if (!settled) step();
      draw();
      if (!settled) {
        animRef.current = requestAnimationFrame(tick);
      }
    };

    cancelAnimationFrame(animRef.current);
    animRef.current = requestAnimationFrame(tick);

    return () => cancelAnimationFrame(animRef.current);
    // selectedNode is intentionally excluded from deps so clicking doesn't
    // restart the simulation. We redraw via the click handler instead.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nodes, canvasWidth]);

  // Redraw when selectedNode changes (without restarting simulation)
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || simRef.current.nodes.length === 0) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const sn = simRef.current.nodes;
    const se = simRef.current.edges;

    ctx.clearRect(0, 0, canvasWidth, CANVAS_HEIGHT);

    for (const e of se) {
      const a = sn[e.source];
      const b = sn[e.target];
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.strokeStyle = `rgba(139, 148, 158, ${e.strength * 0.6})`;
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }

    for (const n of sn) {
      const color = HEALTH_COLORS[n.health] || HEALTH_COLORS.unknown;

      if (n.isCoordinator) {
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius + 5, 0, Math.PI * 2);
        ctx.strokeStyle = color;
        ctx.lineWidth = 2;
        ctx.stroke();
      }

      ctx.beginPath();
      ctx.arc(n.x, n.y, n.radius, 0, Math.PI * 2);
      ctx.fillStyle = color;
      ctx.globalAlpha = n.health === "offline" ? 0.45 : 0.85;
      ctx.fill();
      ctx.globalAlpha = 1;

      if (selectedNode && selectedNode.id === n.id) {
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius + 3, 0, Math.PI * 2);
        ctx.strokeStyle = "#ffffff";
        ctx.lineWidth = 2;
        ctx.stroke();
      }

      ctx.fillStyle = "#ffffff";
      ctx.font = "bold 11px sans-serif";
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(String(n.id), n.x, n.y);

      ctx.fillStyle = "#8b949e";
      ctx.font = "11px sans-serif";
      ctx.textBaseline = "top";
      ctx.fillText(n.label, n.x, n.y + n.radius + 6);
    }
  }, [selectedNode, canvasWidth]);

  // -----------------------------------------------------------------------
  // Canvas click handler
  // -----------------------------------------------------------------------
  const handleCanvasClick = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      const hit = hitTest(mx, my, simRef.current.nodes);
      setSelectedNode(hit);
    },
    [],
  );

  // -----------------------------------------------------------------------
  // Derived stats
  // -----------------------------------------------------------------------
  const onlineCount = nodes.filter((n) => n.health === "online").length;

  // -----------------------------------------------------------------------
  // Render
  // -----------------------------------------------------------------------
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
            Mesh Topology
          </h1>
          <p
            style={{
              fontSize: 13,
              color: "var(--text-secondary)",
              marginTop: "var(--space-1)",
            }}
          >
            Force-directed view of the ESP32 mesh network
          </p>
        </div>
        <button
          onClick={fetchNodes}
          disabled={scanning}
          style={{
            padding: "var(--space-2) var(--space-4)",
            borderRadius: 6,
            background: scanning ? "var(--bg-active)" : "var(--accent)",
            color: scanning ? "var(--text-muted)" : "#fff",
            fontSize: 13,
            fontWeight: 600,
            border: "none",
            cursor: scanning ? "default" : "pointer",
          }}
        >
          {scanning ? "Scanning..." : "Refresh"}
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

      {/* Canvas container */}
      <div
        ref={containerRef}
        style={{
          background: "var(--bg-elevated)",
          border: "1px solid var(--border)",
          borderRadius: 8,
          overflow: "hidden",
          marginBottom: "var(--space-4)",
        }}
      >
        {nodes.length === 0 ? (
          <div
            style={{
              height: CANVAS_HEIGHT,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: "var(--text-muted)",
              fontSize: 13,
            }}
          >
            {scanning
              ? "Scanning for nodes..."
              : "No nodes found. Click Refresh to discover ESP32 devices."}
          </div>
        ) : (
          <canvas
            ref={canvasRef}
            width={canvasWidth}
            height={CANVAS_HEIGHT}
            onClick={handleCanvasClick}
            style={{
              display: "block",
              width: "100%",
              height: CANVAS_HEIGHT,
              cursor: "pointer",
            }}
          />
        )}
      </div>

      {/* Stats bar */}
      <div
        style={{
          display: "flex",
          gap: "var(--space-5)",
          background: "var(--bg-surface)",
          border: "1px solid var(--border)",
          borderRadius: 6,
          padding: "var(--space-3) var(--space-4)",
          marginBottom: "var(--space-4)",
          fontFamily: "var(--font-mono)",
          fontSize: 12,
          color: "var(--text-secondary)",
        }}
      >
        <span>
          <span style={{ color: "var(--text-muted)" }}>Nodes </span>
          <span style={{ color: "var(--status-online)" }}>{onlineCount}</span>
          <span style={{ color: "var(--text-muted)" }}>/{nodes.length} online</span>
        </span>
        <span>
          <span style={{ color: "var(--text-muted)" }}>Drift </span>
          &plusmn;0.3ms
        </span>
        <span>
          <span style={{ color: "var(--text-muted)" }}>Cycle </span>
          50ms
        </span>
      </div>

      {/* Selected node detail card */}
      {selectedNode && (
        <div
          style={{
            background: "var(--bg-surface)",
            border: "1px solid var(--border)",
            borderRadius: 8,
            padding: "var(--space-4)",
          }}
        >
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              marginBottom: "var(--space-3)",
            }}
          >
            <h3
              style={{
                margin: 0,
                fontSize: 14,
                fontWeight: 600,
                color: "var(--text-primary)",
              }}
            >
              {selectedNode.label}
            </h3>
            <span
              style={{
                fontSize: 11,
                fontWeight: 600,
                padding: "2px 8px",
                borderRadius: 10,
                background:
                  HEALTH_COLORS[selectedNode.health] + "22",
                color: HEALTH_COLORS[selectedNode.health],
                textTransform: "uppercase",
                letterSpacing: "0.04em",
              }}
            >
              {selectedNode.health}
            </span>
          </div>
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
              gap: "var(--space-3) var(--space-5)",
              fontSize: 12,
            }}
          >
            <DetailField label="IP Address" value={selectedNode.ip} mono />
            <DetailField label="MAC" value={selectedNode.mac ?? "--"} mono />
            <DetailField
              label="Firmware"
              value={selectedNode.firmware ?? "--"}
              mono
            />
            <DetailField
              label="Role"
              value={selectedNode.isCoordinator ? "Coordinator" : "Node"}
            />
            <DetailField
              label="TDM Slot"
              value={`${selectedNode.tdmSlot} / ${nodes.length}`}
              mono
            />
            <DetailField
              label="Node ID"
              value={String(selectedNode.id)}
              mono
            />
          </div>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function DetailField({
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
      <div
        style={{
          color: "var(--text-secondary)",
          fontFamily: mono ? "var(--font-mono)" : "var(--font-sans)",
        }}
      >
        {value}
      </div>
    </div>
  );
}
