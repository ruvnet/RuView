import { useState, useEffect, useCallback } from "react";
import Dashboard from "./pages/Dashboard";
import { Nodes } from "./pages/Nodes";
import NetworkDiscovery from "./pages/NetworkDiscovery";
import { FlashFirmware } from "./pages/FlashFirmware";
import { OtaUpdate } from "./pages/OtaUpdate";
import { EdgeModules } from "./pages/EdgeModules";
import { Sensing } from "./pages/Sensing";
import { MeshView } from "./pages/MeshView";
import { Settings } from "./pages/Settings";

type Page =
  | "dashboard"
  | "discovery"
  | "nodes"
  | "flash"
  | "ota"
  | "wasm"
  | "sensing"
  | "mesh"
  | "settings";

interface NavItem {
  id: Page;
  label: string;
  icon: string;
}

const NAV_ITEMS: NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: "\u25A6" },
  { id: "discovery", label: "Discovery", icon: "\u25CE" },
  { id: "nodes", label: "Nodes", icon: "\u25C9" },
  { id: "flash", label: "Flash", icon: "\u26A1" },
  { id: "ota", label: "OTA", icon: "\u2B06" },
  { id: "wasm", label: "Edge Modules", icon: "\u2B21" },
  { id: "sensing", label: "Sensing", icon: "\u2248" },
  { id: "mesh", label: "Mesh View", icon: "\u2B2F" },
  { id: "settings", label: "Settings", icon: "\u2699" },
];

interface LiveStatus {
  nodeCount: number;
  onlineCount: number;
  serverRunning: boolean;
  serverPort: number | null;
}

const App: React.FC = () => {
  const [activePage, setActivePage] = useState<Page>("dashboard");
  const [hoveredNav, setHoveredNav] = useState<Page | null>(null);
  const [pageKey, setPageKey] = useState(0);
  const [liveStatus, setLiveStatus] = useState<LiveStatus>({
    nodeCount: 0,
    onlineCount: 0,
    serverRunning: false,
    serverPort: null,
  });

  const navigateTo = useCallback((page: Page) => {
    setActivePage(page);
    setPageKey((k) => k + 1);
  }, []);

  // Poll live status every 5 seconds
  useEffect(() => {
    const poll = async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const [nodes, server] = await Promise.all([
          invoke<{ health: string }[]>("discover_nodes", { timeoutMs: 2000 }).catch(() => []),
          invoke<{ running: boolean; http_port: number | null }>("server_status").catch(() => ({
            running: false,
            http_port: null,
          })),
        ]);
        setLiveStatus({
          nodeCount: nodes.length,
          onlineCount: nodes.filter((n) => n.health === "online").length,
          serverRunning: server.running,
          serverPort: server.http_port,
        });
      } catch {
        // Tauri not available (browser preview) — leave defaults
      }
    };
    poll();
    const id = setInterval(poll, 8000);
    return () => clearInterval(id);
  }, []);

  const renderPage = () => {
    switch (activePage) {
      case "dashboard": return <Dashboard />;
      case "discovery": return <NetworkDiscovery />;
      case "nodes": return <Nodes />;
      case "flash": return <FlashFirmware />;
      case "ota": return <OtaUpdate />;
      case "wasm": return <EdgeModules />;
      case "sensing": return <Sensing />;
      case "mesh": return <MeshView />;
      case "settings": return <Settings />;
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100vh", overflow: "hidden" }}>
      <div style={{ display: "flex", flex: 1, overflow: "hidden" }}>
        {/* Sidebar */}
        <nav
          style={{
            width: 220,
            minWidth: 220,
            background: "var(--bg-surface)",
            borderRight: "1px solid var(--border)",
            display: "flex",
            flexDirection: "column",
            userSelect: "none",
          }}
        >
          {/* Brand */}
          <div
            style={{
              padding: "20px 16px 16px",
              borderBottom: "1px solid var(--border)",
            }}
          >
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 2 }}>
              <div
                style={{
                  width: 30,
                  height: 30,
                  borderRadius: 8,
                  background: "linear-gradient(135deg, var(--accent), #a855f7, #ec4899)",
                  backgroundSize: "200% 200%",
                  animation: "gradient-shift 4s ease infinite",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  fontSize: 15,
                  fontWeight: 800,
                  color: "#fff",
                  fontFamily: "var(--font-sans)",
                  boxShadow: "0 2px 12px rgba(124, 58, 237, 0.4)",
                }}
              >
                R
              </div>
              <div>
                <h1
                  style={{
                    fontSize: 17,
                    fontWeight: 700,
                    color: "var(--text-primary)",
                    fontFamily: "var(--font-sans)",
                    margin: 0,
                    letterSpacing: "-0.01em",
                    lineHeight: 1.2,
                  }}
                >
                  RuView
                </h1>
                <span
                  style={{
                    fontSize: 10,
                    color: "var(--text-muted)",
                    fontFamily: "var(--font-mono)",
                    letterSpacing: "0.02em",
                  }}
                >
                  v0.4.0
                </span>
              </div>
            </div>
          </div>

          {/* Nav items */}
          <div style={{ flex: 1, paddingTop: 6, paddingBottom: 6, overflowY: "auto" }}>
            {NAV_ITEMS.map((item) => {
              const isActive = activePage === item.id;
              const isHovered = hoveredNav === item.id && !isActive;
              return (
                <button
                  key={item.id}
                  onClick={() => navigateTo(item.id)}
                  onMouseEnter={() => setHoveredNav(item.id)}
                  onMouseLeave={() => setHoveredNav(null)}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 10,
                    width: "100%",
                    padding: "8px 16px",
                    background: isActive
                      ? "linear-gradient(90deg, rgba(124, 58, 237, 0.15), transparent)"
                      : isHovered
                        ? "var(--bg-hover)"
                        : "transparent",
                    color: isActive ? "var(--text-primary)" : "var(--text-secondary)",
                    fontSize: 13,
                    fontWeight: isActive ? 600 : 400,
                    textAlign: "left",
                    borderLeft: isActive
                      ? "3px solid transparent"
                      : "3px solid transparent",
                    fontFamily: "var(--font-sans)",
                    borderRadius: 0,
                    transition: "all 0.15s ease",
                    position: "relative",
                  }}
                >
                  {/* Active gradient indicator */}
                  {isActive && (
                    <span
                      style={{
                        position: "absolute",
                        left: 0,
                        top: 4,
                        bottom: 4,
                        width: 3,
                        borderRadius: "0 3px 3px 0",
                        background: "linear-gradient(180deg, var(--accent), #a855f7)",
                        boxShadow: "0 0 8px rgba(124, 58, 237, 0.5)",
                      }}
                    />
                  )}
                  <span
                    style={{
                      width: 24,
                      height: 24,
                      borderRadius: 6,
                      background: isActive
                        ? "linear-gradient(135deg, var(--accent), #a855f7)"
                        : isHovered
                          ? "var(--bg-active)"
                          : "var(--bg-elevated)",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontSize: 12,
                      color: isActive ? "#fff" : "var(--text-muted)",
                      transition: "all 0.15s ease",
                      flexShrink: 0,
                      boxShadow: isActive ? "0 2px 8px rgba(124, 58, 237, 0.3)" : "none",
                      transform: isHovered ? "scale(1.1)" : "scale(1)",
                    }}
                  >
                    {item.icon}
                  </span>
                  {item.label}
                </button>
              );
            })}
          </div>

          {/* Live connection footer */}
          <div
            style={{
              padding: "10px 16px",
              fontSize: 11,
              color: "var(--text-muted)",
              borderTop: "1px solid var(--border)",
              fontFamily: "var(--font-mono)",
              display: "flex",
              alignItems: "center",
              gap: 6,
            }}
          >
            <span className="status-dot status-dot--online" style={{ width: 6, height: 6 }} />
            <span>Connected</span>
            {liveStatus.nodeCount > 0 && (
              <span style={{ marginLeft: "auto", color: "var(--text-muted)" }}>
                {liveStatus.onlineCount}/{liveStatus.nodeCount}
              </span>
            )}
          </div>
        </nav>

        {/* Main content */}
        <main
          style={{
            flex: 1,
            overflow: "auto",
            background: "var(--bg-base)",
          }}
        >
          <div key={pageKey} className="page-transition">
            {renderPage()}
          </div>
        </main>
      </div>

      {/* Status Bar */}
      <footer
        style={{
          height: "var(--statusbar-height)",
          minHeight: "var(--statusbar-height)",
          background: "var(--bg-surface)",
          borderTop: "1px solid var(--border)",
          display: "flex",
          alignItems: "center",
          padding: "0 16px",
          gap: 16,
          fontSize: 11,
          fontFamily: "var(--font-sans)",
          color: "var(--text-muted)",
          userSelect: "none",
        }}
      >
        <span style={{ color: "var(--text-muted)", fontWeight: 500 }}>
          Powered by rUv
        </span>

        <span style={{ color: "var(--border)" }}>{"\u2502"}</span>

        <span style={{ display: "flex", alignItems: "center", gap: 5 }}>
          <span
            className={`status-dot ${liveStatus.onlineCount > 0 ? "status-dot--online" : "status-dot--error"}`}
            style={{ width: 6, height: 6 }}
          />
          {liveStatus.onlineCount > 0
            ? `${liveStatus.onlineCount} node${liveStatus.onlineCount !== 1 ? "s" : ""} online`
            : "No nodes"}
        </span>

        <span style={{ color: "var(--border)" }}>{"\u2502"}</span>

        <span style={{ display: "flex", alignItems: "center", gap: 5 }}>
          <span
            className={`status-dot ${liveStatus.serverRunning ? "status-dot--online" : "status-dot--error"}`}
            style={{ width: 6, height: 6 }}
          />
          Server: {liveStatus.serverRunning ? "running" : "stopped"}
        </span>

        <span style={{ flex: 1 }} />

        {liveStatus.serverPort && (
          <span style={{ fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>
            :{liveStatus.serverPort}
          </span>
        )}

        <span
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 10,
            color: "var(--text-muted)",
            opacity: 0.6,
          }}
        >
          {new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
        </span>
      </footer>
    </div>
  );
};

export default App;
