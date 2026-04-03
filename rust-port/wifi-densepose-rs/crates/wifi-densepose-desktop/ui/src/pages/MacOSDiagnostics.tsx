import { useState, useEffect } from "react";

interface WifiInfo {
  ssid: string | null;
  bssid: string | null;
  channel: number | null;
  rssi: number | null;
  noise: number | null;
  tx_rate: number | null;
  security: string | null;
  phy_mode: string | null;
}

interface WifiScanResult {
  ssid: string;
  bssid: string;
  rssi: number;
  channel: number;
  security: string;
}

interface SystemInfo {
  os_version: string;
  arch: string;
  model: string | null;
  wifi_interface: string | null;
  wifi_power: boolean;
  serial_drivers: string[];
}

interface Permissions {
  network_access: boolean;
  usb_access: boolean;
  wifi_scan: boolean;
  location_services: boolean;
}

const isMacOS = navigator.userAgent.includes("Mac");

export const MacOSDiagnostics: React.FC = () => {
  const [wifiInfo, setWifiInfo] = useState<WifiInfo | null>(null);
  const [scanResults, setScanResults] = useState<WifiScanResult[]>([]);
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
  const [permissions, setPermissions] = useState<Permissions | null>(null);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isMacOS) return;
    loadSystemInfo();
    loadWifiInfo();
    loadPermissions();
  }, []);

  const loadSystemInfo = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const info = await invoke<SystemInfo>("macos_system_info");
      setSystemInfo(info);
    } catch (e) {
      console.warn("Failed to get system info:", e);
    }
  };

  const loadWifiInfo = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const info = await invoke<WifiInfo>("macos_wifi_info");
      setWifiInfo(info);
    } catch (e) {
      console.warn("Failed to get WiFi info:", e);
    }
  };

  const loadPermissions = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const perms = await invoke<Permissions>("macos_check_permissions");
      setPermissions(perms);
    } catch (e) {
      console.warn("Failed to check permissions:", e);
    }
  };

  const runWifiScan = async () => {
    setScanning(true);
    setError(null);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const results = await invoke<WifiScanResult[]>("macos_wifi_scan");
      setScanResults(results);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  };

  if (!isMacOS) {
    return (
      <div style={{ padding: 32 }}>
        <h2 style={{ color: "var(--text-primary)", margin: 0 }}>macOS Diagnostics</h2>
        <p style={{ color: "var(--text-muted)", marginTop: 12 }}>
          This page shows macOS-specific WiFi and system diagnostics. Only available on macOS.
        </p>
      </div>
    );
  }

  const rssiColor = (rssi: number) => {
    if (rssi >= -50) return "var(--accent-green, #22c55e)";
    if (rssi >= -70) return "var(--accent, #7c3aed)";
    return "var(--accent-red, #ef4444)";
  };

  return (
    <div style={{ padding: 32, maxWidth: 960 }}>
      <h2 style={{ color: "var(--text-primary)", margin: "0 0 24px 0", fontFamily: "var(--font-sans)" }}>
        macOS Diagnostics
      </h2>

      {/* System Info */}
      {systemInfo && (
        <div className="card" style={{ marginBottom: 20, padding: 20 }}>
          <h3 style={{ color: "var(--text-primary)", margin: "0 0 12px", fontSize: 14 }}>System</h3>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px 24px", fontSize: 13 }}>
            <div><span style={{ color: "var(--text-muted)" }}>macOS</span> <span style={{ color: "var(--text-primary)" }}>{systemInfo.os_version}</span></div>
            <div><span style={{ color: "var(--text-muted)" }}>Arch</span> <span style={{ color: "var(--text-primary)" }}>{systemInfo.arch}</span></div>
            <div><span style={{ color: "var(--text-muted)" }}>Model</span> <span style={{ color: "var(--text-primary)" }}>{systemInfo.model || "N/A"}</span></div>
            <div><span style={{ color: "var(--text-muted)" }}>WiFi</span> <span style={{ color: "var(--text-primary)" }}>{systemInfo.wifi_interface || "N/A"} ({systemInfo.wifi_power ? "On" : "Off"})</span></div>
            {systemInfo.serial_drivers.length > 0 && (
              <div style={{ gridColumn: "1 / -1" }}>
                <span style={{ color: "var(--text-muted)" }}>Serial Drivers</span>{" "}
                <span style={{ color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>
                  {systemInfo.serial_drivers.join(", ")}
                </span>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Permissions */}
      {permissions && (
        <div className="card" style={{ marginBottom: 20, padding: 20 }}>
          <h3 style={{ color: "var(--text-primary)", margin: "0 0 12px", fontSize: 14 }}>Permissions</h3>
          <div style={{ display: "flex", gap: 16, flexWrap: "wrap" }}>
            {[
              { label: "Network", ok: permissions.network_access },
              { label: "USB Serial", ok: permissions.usb_access },
              { label: "WiFi Scan", ok: permissions.wifi_scan },
              { label: "Location", ok: permissions.location_services },
            ].map((p) => (
              <span
                key={p.label}
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  gap: 6,
                  padding: "4px 10px",
                  borderRadius: 6,
                  fontSize: 12,
                  fontFamily: "var(--font-mono)",
                  background: p.ok ? "rgba(34,197,94,0.1)" : "rgba(239,68,68,0.1)",
                  color: p.ok ? "#22c55e" : "#ef4444",
                  border: `1px solid ${p.ok ? "rgba(34,197,94,0.2)" : "rgba(239,68,68,0.2)"}`,
                }}
              >
                {p.ok ? "\u2713" : "\u2717"} {p.label}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Current WiFi */}
      {wifiInfo && (
        <div className="card" style={{ marginBottom: 20, padding: 20 }}>
          <h3 style={{ color: "var(--text-primary)", margin: "0 0 12px", fontSize: 14 }}>Current WiFi Connection</h3>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: "8px 24px", fontSize: 13 }}>
            <div><span style={{ color: "var(--text-muted)" }}>SSID</span> <span style={{ color: "var(--text-primary)", fontWeight: 600 }}>{wifiInfo.ssid || "N/A"}</span></div>
            <div><span style={{ color: "var(--text-muted)" }}>Channel</span> <span style={{ color: "var(--text-primary)" }}>{wifiInfo.channel || "N/A"}</span></div>
            <div><span style={{ color: "var(--text-muted)" }}>RSSI</span> <span style={{ color: rssiColor(wifiInfo.rssi || -100), fontWeight: 600 }}>{wifiInfo.rssi || "N/A"} dBm</span></div>
            <div><span style={{ color: "var(--text-muted)" }}>BSSID</span> <span style={{ color: "var(--text-primary)", fontFamily: "var(--font-mono)", fontSize: 11 }}>{wifiInfo.bssid || "N/A"}</span></div>
            <div><span style={{ color: "var(--text-muted)" }}>Noise</span> <span style={{ color: "var(--text-primary)" }}>{wifiInfo.noise || "N/A"} dBm</span></div>
            <div><span style={{ color: "var(--text-muted)" }}>Tx Rate</span> <span style={{ color: "var(--text-primary)" }}>{wifiInfo.tx_rate || "N/A"} Mbps</span></div>
            <div><span style={{ color: "var(--text-muted)" }}>Security</span> <span style={{ color: "var(--text-primary)" }}>{wifiInfo.security || "N/A"}</span></div>
            <div><span style={{ color: "var(--text-muted)" }}>PHY</span> <span style={{ color: "var(--text-primary)" }}>{wifiInfo.phy_mode || "N/A"}</span></div>
          </div>
        </div>
      )}

      {/* WiFi Scan */}
      <div className="card" style={{ padding: 20 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
          <h3 style={{ color: "var(--text-primary)", margin: 0, fontSize: 14 }}>WiFi Site Survey</h3>
          <button
            onClick={runWifiScan}
            disabled={scanning}
            style={{
              padding: "6px 14px",
              borderRadius: 6,
              fontSize: 12,
              fontWeight: 600,
              background: "linear-gradient(135deg, var(--accent), #a855f7)",
              color: "#fff",
              border: "none",
              cursor: scanning ? "wait" : "pointer",
              opacity: scanning ? 0.7 : 1,
            }}
          >
            {scanning ? "Scanning..." : "Scan Networks"}
          </button>
        </div>

        {error && (
          <div style={{ color: "#ef4444", fontSize: 12, marginBottom: 12, padding: "8px 12px", background: "rgba(239,68,68,0.1)", borderRadius: 6 }}>
            {error}
          </div>
        )}

        {scanResults.length > 0 && (
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 12 }}>
            <thead>
              <tr style={{ borderBottom: "1px solid var(--border)" }}>
                {["SSID", "BSSID", "Ch", "RSSI", "Security"].map((h) => (
                  <th key={h} style={{ padding: "6px 8px", textAlign: "left", color: "var(--text-muted)", fontWeight: 500 }}>{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {scanResults.map((r, i) => (
                <tr key={i} style={{ borderBottom: "1px solid var(--border)" }}>
                  <td style={{ padding: "6px 8px", color: "var(--text-primary)", fontWeight: 500 }}>{r.ssid || "(hidden)"}</td>
                  <td style={{ padding: "6px 8px", color: "var(--text-muted)", fontFamily: "var(--font-mono)", fontSize: 10 }}>{r.bssid}</td>
                  <td style={{ padding: "6px 8px", color: "var(--text-primary)" }}>{r.channel}</td>
                  <td style={{ padding: "6px 8px", color: rssiColor(r.rssi), fontWeight: 600 }}>{r.rssi} dBm</td>
                  <td style={{ padding: "6px 8px", color: "var(--text-muted)", fontSize: 10 }}>{r.security}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        {scanResults.length === 0 && !scanning && (
          <p style={{ color: "var(--text-muted)", fontSize: 12, margin: 0 }}>
            Click "Scan Networks" to discover nearby WiFi access points. Useful for positioning ESP32 nodes.
          </p>
        )}
      </div>
    </div>
  );
};
