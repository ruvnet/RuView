import { useState, useEffect, useCallback } from "react";
import type { AppSettings } from "../types";

const DEFAULT_SETTINGS: AppSettings = {
  server_http_port: 8080,
  server_ws_port: 8765,
  server_udp_port: 5005,
  bind_address: "127.0.0.1",
  ui_path: "",
  ota_psk: "",
  auto_discover: true,
  discover_interval_ms: 10_000,
  theme: "dark",
};

export function Settings() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [saved, setSaved] = useState(false);
  const [showPsk, setShowPsk] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const persisted = await invoke<AppSettings | null>("get_settings");
        if (persisted) setSettings(persisted);
      } catch {
        // Settings command may not exist yet
      }
    })();
  }, []);

  const update = useCallback(
    <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
      setSettings((prev) => ({ ...prev, [key]: value }));
      setSaved(false);
    },
    []
  );

  const save = async () => {
    setError(null);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_settings", { settings });
      setSaved(true);
      setTimeout(() => setSaved(false), 2500);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const reset = () => {
    setSettings(DEFAULT_SETTINGS);
    setSaved(false);
  };

  return (
    <div style={{ padding: "var(--space-5)", maxWidth: 600 }}>
      <h1 className="heading-lg" style={{ margin: "0 0 var(--space-1)" }}>Settings</h1>
      <p style={{ fontSize: 13, color: "var(--text-secondary)", marginBottom: "var(--space-5)" }}>
        Configure server, network, and application preferences
      </p>

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

      {saved && (
        <div
          style={{
            background: "rgba(63, 185, 80, 0.1)",
            border: "1px solid rgba(63, 185, 80, 0.3)",
            borderRadius: 6,
            padding: "var(--space-3) var(--space-4)",
            marginBottom: "var(--space-4)",
            fontSize: 13,
            color: "var(--status-online)",
          }}
        >
          Settings saved.
        </div>
      )}

      {/* Sensing Server */}
      <Section title="Sensing Server">
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-4)" }}>
          <Field label="HTTP Port">
            <NumberInput value={settings.server_http_port} onChange={(v) => update("server_http_port", v)} min={1} max={65535} />
          </Field>
          <Field label="WebSocket Port">
            <NumberInput value={settings.server_ws_port} onChange={(v) => update("server_ws_port", v)} min={1} max={65535} />
          </Field>
          <Field label="UDP Port">
            <NumberInput value={settings.server_udp_port} onChange={(v) => update("server_udp_port", v)} min={1} max={65535} />
          </Field>
          <Field label="Bind Address">
            <input
              type="text"
              value={settings.bind_address}
              onChange={(e) => update("bind_address", e.target.value)}
              placeholder="127.0.0.1"
              style={{ fontFamily: "var(--font-mono)" }}
            />
          </Field>
        </div>
        <div style={{ marginTop: "var(--space-4)" }}>
          <Field label="UI Static Files Path">
            <input
              type="text"
              value={settings.ui_path}
              onChange={(e) => update("ui_path", e.target.value)}
              placeholder="Leave empty for default"
            />
          </Field>
        </div>
      </Section>

      {/* Security */}
      <Section title="Security">
        <Field label="OTA Pre-Shared Key (PSK)">
          <div style={{ display: "flex", gap: "var(--space-2)" }}>
            <input
              type={showPsk ? "text" : "password"}
              value={settings.ota_psk}
              onChange={(e) => update("ota_psk", e.target.value)}
              placeholder="Enter PSK for OTA authentication"
              style={{ flex: 1, fontFamily: "var(--font-mono)" }}
            />
            <button onClick={() => setShowPsk((prev) => !prev)} style={secondaryBtn}>
              {showPsk ? "Hide" : "Show"}
            </button>
          </div>
          <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: "var(--space-1)" }}>
            Used for authenticating OTA firmware updates to nodes.
          </p>
        </Field>
      </Section>

      {/* Discovery */}
      <Section title="Network Discovery">
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-4)" }}>
          <Field label="Auto-Discover">
            <label style={{ display: "flex", alignItems: "center", gap: "var(--space-2)", cursor: "pointer" }}>
              <input
                type="checkbox"
                checked={settings.auto_discover}
                onChange={(e) => update("auto_discover", e.target.checked)}
                style={{ accentColor: "var(--accent)" }}
              />
              <span style={{ fontSize: 13, color: "var(--text-secondary)" }}>Enable periodic scanning</span>
            </label>
          </Field>
          <Field label="Scan Interval (ms)">
            <NumberInput
              value={settings.discover_interval_ms}
              onChange={(v) => update("discover_interval_ms", v)}
              min={1000}
              max={120_000}
              step={1000}
              disabled={!settings.auto_discover}
            />
          </Field>
        </div>
      </Section>

      {/* Actions */}
      <div style={{ display: "flex", justifyContent: "space-between", marginTop: "var(--space-5)" }}>
        <button onClick={reset} style={secondaryBtn}>Reset to Defaults</button>
        <button onClick={save} style={primaryBtn}>Save Settings</button>
      </div>
    </div>
  );
}

// --- Sub-components ---

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div
      style={{
        background: "var(--bg-surface)",
        border: "1px solid var(--border)",
        borderRadius: 8,
        padding: "var(--space-5)",
        marginBottom: "var(--space-4)",
      }}
    >
      <h2
        style={{
          fontSize: 14,
          fontWeight: 600,
          color: "var(--text-primary)",
          margin: "0 0 var(--space-4)",
          fontFamily: "var(--font-sans)",
        }}
      >
        {title}
      </h2>
      {children}
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <label
        style={{
          display: "block",
          fontSize: 12,
          fontWeight: 600,
          color: "var(--text-secondary)",
          marginBottom: 6,
          fontFamily: "var(--font-sans)",
        }}
      >
        {label}
      </label>
      {children}
    </div>
  );
}

function NumberInput({
  value, onChange, min, max, step = 1, disabled = false,
}: {
  value: number; onChange: (v: number) => void; min?: number; max?: number; step?: number; disabled?: boolean;
}) {
  return (
    <input
      type="number"
      value={value}
      onChange={(e) => { const n = parseInt(e.target.value, 10); if (!isNaN(n)) onChange(n); }}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
    />
  );
}

// --- Shared styles ---

const primaryBtn: React.CSSProperties = {
  padding: "var(--space-2) 20px",
  border: "none",
  borderRadius: 6,
  background: "var(--accent)",
  color: "#fff",
  fontSize: 13,
  fontWeight: 600,
};

const secondaryBtn: React.CSSProperties = {
  padding: "var(--space-2) var(--space-4)",
  border: "1px solid var(--border)",
  borderRadius: 6,
  background: "transparent",
  color: "var(--text-secondary)",
  fontSize: 13,
  fontWeight: 500,
};
