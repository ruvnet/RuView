import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface RuVectorConfig {
  // MinCut Parameters
  mincut_enabled: boolean;
  mincut_threshold: number;
  mincut_max_persons: number;

  // Attention Parameters
  attention_enabled: boolean;
  attention_heads: number;
  attention_dropout: number;

  // Temporal Parameters
  temporal_enabled: boolean;
  temporal_window_ms: number;
  temporal_compression_ratio: number;

  // Solver Parameters
  solver_enabled: boolean;
  solver_interpolation: "linear" | "cubic" | "sparse";
  solver_subcarrier_count: number;

  // BVP Parameters
  bvp_enabled: boolean;
  bvp_filter_hz: [number, number];
}

const DEFAULT_CONFIG: RuVectorConfig = {
  mincut_enabled: true,
  mincut_threshold: 0.5,
  mincut_max_persons: 5,
  attention_enabled: true,
  attention_heads: 4,
  attention_dropout: 0.1,
  temporal_enabled: true,
  temporal_window_ms: 500,
  temporal_compression_ratio: 4,
  solver_enabled: true,
  solver_interpolation: "sparse",
  solver_subcarrier_count: 56,
  bvp_enabled: false,
  bvp_filter_hz: [0.7, 4.0],
};

const MODULES = [
  {
    id: "mincut",
    name: "MinCut Segmentation",
    crate: "ruvector-mincut",
    description: "Graph-based person segmentation using DynamicPersonMatcher",
    icon: "✂️",
  },
  {
    id: "attention",
    name: "Spatial Attention",
    crate: "ruvector-attention",
    description: "Attention-weighted antenna selection and BVP extraction",
    icon: "🎯",
  },
  {
    id: "temporal",
    name: "Temporal Tensor",
    crate: "ruvector-temporal-tensor",
    description: "Temporal CSI compression and breathing detection",
    icon: "⏱️",
  },
  {
    id: "solver",
    name: "Sparse Solver",
    crate: "ruvector-solver",
    description: "Sparse interpolation (114→56 subcarriers) and triangulation",
    icon: "🧮",
  },
  {
    id: "attn-mincut",
    name: "Attention MinCut",
    crate: "ruvector-attn-mincut",
    description: "Combined attention-weighted graph segmentation",
    icon: "🔀",
  },
];

const RuVectorTab: React.FC = () => {
  const [config, setConfig] = useState<RuVectorConfig>(DEFAULT_CONFIG);
  const [testingLive, setTestingLive] = useState(false);
  const [liveMetrics, setLiveMetrics] = useState<{
    fps: number;
    latency_ms: number;
    persons_detected: number;
  } | null>(null);
  const [saved, setSaved] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    try {
      const loaded = await invoke<RuVectorConfig>("get_ruvector_config");
      setConfig(loaded);
    } catch (err) {
      // Use defaults if not implemented
    }
  };

  const saveConfig = async () => {
    setError(null);
    try {
      await invoke("set_ruvector_config", { config });
      setSaved(true);
    } catch (err) {
      setError(`Failed to save: ${err}`);
    }
  };

  const handleChange = <K extends keyof RuVectorConfig>(
    key: K,
    value: RuVectorConfig[K]
  ) => {
    setConfig((prev) => ({ ...prev, [key]: value }));
    setSaved(false);
  };

  const startLiveTest = async () => {
    setTestingLive(true);
    setError(null);
    try {
      // Simulate live testing metrics
      const interval = setInterval(() => {
        setLiveMetrics({
          fps: 25 + Math.random() * 10,
          latency_ms: 15 + Math.random() * 10,
          persons_detected: Math.floor(Math.random() * 3) + 1,
        });
      }, 500);

      // Stop after 10 seconds for demo
      setTimeout(() => {
        clearInterval(interval);
        setTestingLive(false);
        setLiveMetrics(null);
      }, 10000);
    } catch (err) {
      setError(`Live test failed: ${err}`);
      setTestingLive(false);
    }
  };

  const exportConfig = () => {
    const blob = new Blob([JSON.stringify(config, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "ruvector-config.json";
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div>
      {/* Module Cards */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))",
          gap: "var(--space-3)",
          marginBottom: "var(--space-5)",
        }}
      >
        {MODULES.map((mod) => {
          const isEnabled =
            config[`${mod.id.replace("-", "_")}_enabled` as keyof RuVectorConfig] ?? true;
          return (
            <div
              key={mod.id}
              className="card"
              style={{
                padding: "var(--space-3)",
                opacity: isEnabled ? 1 : 0.5,
                transition: "opacity 0.2s",
              }}
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "start",
                }}
              >
                <span style={{ fontSize: 24 }}>{mod.icon}</span>
                <span
                  style={{
                    fontSize: 9,
                    padding: "2px 6px",
                    borderRadius: 3,
                    background: isEnabled
                      ? "rgba(63, 185, 80, 0.15)"
                      : "rgba(139, 148, 158, 0.15)",
                    color: isEnabled ? "var(--status-online)" : "var(--text-muted)",
                    fontWeight: 600,
                  }}
                >
                  {isEnabled ? "ON" : "OFF"}
                </span>
              </div>
              <h4 style={{ margin: "var(--space-2) 0 4px", fontSize: 13, fontWeight: 600 }}>
                {mod.name}
              </h4>
              <p
                style={{
                  fontSize: 11,
                  color: "var(--text-muted)",
                  margin: 0,
                  lineHeight: 1.4,
                }}
              >
                {mod.description}
              </p>
              <div
                style={{
                  marginTop: "var(--space-2)",
                  fontFamily: "var(--font-mono)",
                  fontSize: 10,
                  color: "var(--text-secondary)",
                }}
              >
                {mod.crate}
              </div>
            </div>
          );
        })}
      </div>

      {error && (
        <div
          style={{
            background: "rgba(248, 81, 73, 0.1)",
            border: "1px solid rgba(248, 81, 73, 0.3)",
            borderRadius: 6,
            padding: "var(--space-3)",
            marginBottom: "var(--space-4)",
            fontSize: 13,
            color: "var(--status-error)",
          }}
        >
          {error}
        </div>
      )}

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-5)" }}>
        {/* Configuration Panel */}
        <div className="card" style={{ padding: "var(--space-4)" }}>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 600, marginBottom: "var(--space-4)" }}>
            Module Configuration
          </h3>

          {/* MinCut Section */}
          <ConfigSection title="MinCut Segmentation">
            <ToggleRow
              label="Enable MinCut"
              checked={config.mincut_enabled}
              onChange={(v) => handleChange("mincut_enabled", v)}
            />
            <SliderRow
              label="Threshold"
              value={config.mincut_threshold}
              min={0.1}
              max={1.0}
              step={0.05}
              onChange={(v) => handleChange("mincut_threshold", v)}
              disabled={!config.mincut_enabled}
            />
            <NumberRow
              label="Max Persons"
              value={config.mincut_max_persons}
              min={1}
              max={10}
              onChange={(v) => handleChange("mincut_max_persons", v)}
              disabled={!config.mincut_enabled}
            />
          </ConfigSection>

          {/* Attention Section */}
          <ConfigSection title="Spatial Attention">
            <ToggleRow
              label="Enable Attention"
              checked={config.attention_enabled}
              onChange={(v) => handleChange("attention_enabled", v)}
            />
            <NumberRow
              label="Attention Heads"
              value={config.attention_heads}
              min={1}
              max={16}
              onChange={(v) => handleChange("attention_heads", v)}
              disabled={!config.attention_enabled}
            />
            <SliderRow
              label="Dropout"
              value={config.attention_dropout}
              min={0}
              max={0.5}
              step={0.05}
              onChange={(v) => handleChange("attention_dropout", v)}
              disabled={!config.attention_enabled}
            />
          </ConfigSection>

          {/* Temporal Section */}
          <ConfigSection title="Temporal Processing">
            <ToggleRow
              label="Enable Temporal"
              checked={config.temporal_enabled}
              onChange={(v) => handleChange("temporal_enabled", v)}
            />
            <NumberRow
              label="Window (ms)"
              value={config.temporal_window_ms}
              min={100}
              max={2000}
              step={100}
              onChange={(v) => handleChange("temporal_window_ms", v)}
              disabled={!config.temporal_enabled}
            />
            <NumberRow
              label="Compression Ratio"
              value={config.temporal_compression_ratio}
              min={1}
              max={16}
              onChange={(v) => handleChange("temporal_compression_ratio", v)}
              disabled={!config.temporal_enabled}
            />
          </ConfigSection>

          {/* Solver Section */}
          <ConfigSection title="Sparse Solver">
            <ToggleRow
              label="Enable Solver"
              checked={config.solver_enabled}
              onChange={(v) => handleChange("solver_enabled", v)}
            />
            <div style={{ marginBottom: "var(--space-2)" }}>
              <label style={labelStyle}>Interpolation</label>
              <select
                value={config.solver_interpolation}
                onChange={(e) =>
                  handleChange(
                    "solver_interpolation",
                    e.target.value as RuVectorConfig["solver_interpolation"]
                  )
                }
                disabled={!config.solver_enabled}
                style={{
                  ...inputStyle,
                  opacity: config.solver_enabled ? 1 : 0.5,
                }}
              >
                <option value="linear">Linear</option>
                <option value="cubic">Cubic</option>
                <option value="sparse">Sparse (L1)</option>
              </select>
            </div>
            <NumberRow
              label="Subcarrier Count"
              value={config.solver_subcarrier_count}
              min={28}
              max={114}
              onChange={(v) => handleChange("solver_subcarrier_count", v)}
              disabled={!config.solver_enabled}
            />
          </ConfigSection>

          {/* Action Buttons */}
          <div
            style={{
              display: "flex",
              gap: "var(--space-2)",
              marginTop: "var(--space-4)",
            }}
          >
            <button
              onClick={saveConfig}
              className="btn-gradient"
              style={{
                flex: 1,
                padding: "10px",
                fontSize: 12,
                opacity: saved ? 0.6 : 1,
              }}
              disabled={saved}
            >
              {saved ? "Saved" : "Save Configuration"}
            </button>
            <button
              onClick={exportConfig}
              style={{
                padding: "10px 16px",
                background: "transparent",
                border: "1px solid var(--border)",
                borderRadius: 6,
                color: "var(--text-secondary)",
                fontSize: 12,
                fontWeight: 600,
                cursor: "pointer",
              }}
            >
              Export
            </button>
          </div>
        </div>

        {/* Live Testing Panel */}
        <div className="card" style={{ padding: "var(--space-4)" }}>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 600, marginBottom: "var(--space-4)" }}>
            Live Testing
          </h3>

          <div
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              minHeight: 200,
              background: "var(--bg-secondary)",
              borderRadius: 8,
              marginBottom: "var(--space-4)",
            }}
          >
            {testingLive ? (
              <>
                <div
                  style={{
                    fontSize: 48,
                    animation: "pulse 1s infinite",
                  }}
                >
                  📡
                </div>
                <p style={{ fontSize: 13, color: "var(--text-secondary)", marginTop: "var(--space-2)" }}>
                  Processing live CSI stream...
                </p>
              </>
            ) : (
              <>
                <div style={{ fontSize: 48, opacity: 0.5 }}>📡</div>
                <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: "var(--space-2)" }}>
                  Start live test to apply config to real CSI data
                </p>
              </>
            )}
          </div>

          {liveMetrics && (
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(3, 1fr)",
                gap: "var(--space-3)",
                marginBottom: "var(--space-4)",
              }}
            >
              <MetricCard label="FPS" value={liveMetrics.fps.toFixed(1)} />
              <MetricCard label="Latency" value={`${liveMetrics.latency_ms.toFixed(0)}ms`} />
              <MetricCard label="Persons" value={liveMetrics.persons_detected.toString()} />
            </div>
          )}

          <button
            onClick={testingLive ? () => setTestingLive(false) : startLiveTest}
            style={{
              width: "100%",
              padding: "12px",
              background: testingLive
                ? "rgba(248, 81, 73, 0.1)"
                : "rgba(56, 139, 253, 0.1)",
              border: `1px solid ${testingLive ? "rgba(248, 81, 73, 0.3)" : "rgba(56, 139, 253, 0.3)"}`,
              borderRadius: 6,
              color: testingLive ? "var(--status-error)" : "var(--accent)",
              fontSize: 13,
              fontWeight: 600,
              cursor: "pointer",
            }}
          >
            {testingLive ? "Stop Test" : "Start Live Test"}
          </button>

          {/* Presets */}
          <div style={{ marginTop: "var(--space-5)" }}>
            <h4 style={{ fontSize: 12, fontWeight: 600, marginBottom: "var(--space-3)" }}>
              Quick Presets
            </h4>
            <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
              <PresetButton
                label="Low Latency"
                description="Minimal processing for real-time"
                onClick={() => {
                  setConfig({
                    ...DEFAULT_CONFIG,
                    attention_heads: 2,
                    temporal_compression_ratio: 8,
                    solver_subcarrier_count: 28,
                  });
                  setSaved(false);
                }}
              />
              <PresetButton
                label="High Accuracy"
                description="Maximum quality, higher latency"
                onClick={() => {
                  setConfig({
                    ...DEFAULT_CONFIG,
                    attention_heads: 8,
                    temporal_compression_ratio: 2,
                    solver_subcarrier_count: 114,
                    solver_interpolation: "cubic",
                  });
                  setSaved(false);
                }}
              />
              <PresetButton
                label="Balanced"
                description="Default recommended settings"
                onClick={() => {
                  setConfig(DEFAULT_CONFIG);
                  setSaved(false);
                }}
              />
            </div>
          </div>
        </div>
      </div>

      <style>{`
        @keyframes pulse {
          0%, 100% { transform: scale(1); }
          50% { transform: scale(1.1); }
        }
      `}</style>
    </div>
  );
};

// Helper Components
function ConfigSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: "var(--space-4)" }}>
      <h4
        style={{
          fontSize: 11,
          fontWeight: 600,
          color: "var(--text-muted)",
          textTransform: "uppercase",
          letterSpacing: "0.04em",
          marginBottom: "var(--space-2)",
        }}
      >
        {title}
      </h4>
      {children}
    </div>
  );
}

function ToggleRow({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div
      style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        marginBottom: "var(--space-2)",
      }}
    >
      <span style={{ fontSize: 12 }}>{label}</span>
      <button
        onClick={() => onChange(!checked)}
        style={{
          width: 40,
          height: 22,
          borderRadius: 11,
          border: "none",
          background: checked ? "var(--accent)" : "var(--border)",
          position: "relative",
          cursor: "pointer",
          transition: "background 0.2s",
        }}
      >
        <span
          style={{
            position: "absolute",
            top: 2,
            left: checked ? 20 : 2,
            width: 18,
            height: 18,
            borderRadius: "50%",
            background: "white",
            transition: "left 0.2s",
          }}
        />
      </button>
    </div>
  );
}

function SliderRow({
  label,
  value,
  min,
  max,
  step,
  onChange,
  disabled,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
  disabled?: boolean;
}) {
  return (
    <div style={{ marginBottom: "var(--space-2)", opacity: disabled ? 0.5 : 1 }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 4 }}>
        <span style={{ fontSize: 12 }}>{label}</span>
        <span style={{ fontSize: 11, fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>
          {value.toFixed(2)}
        </span>
      </div>
      <input
        type="range"
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        disabled={disabled}
        style={{ width: "100%", cursor: disabled ? "not-allowed" : "pointer" }}
      />
    </div>
  );
}

function NumberRow({
  label,
  value,
  min,
  max,
  step = 1,
  onChange,
  disabled,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (v: number) => void;
  disabled?: boolean;
}) {
  return (
    <div
      style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        marginBottom: "var(--space-2)",
        opacity: disabled ? 0.5 : 1,
      }}
    >
      <span style={{ fontSize: 12 }}>{label}</span>
      <input
        type="number"
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(e) => onChange(parseInt(e.target.value) || min)}
        disabled={disabled}
        style={{
          width: 70,
          padding: "4px 8px",
          background: "var(--bg-secondary)",
          border: "1px solid var(--border)",
          borderRadius: 4,
          color: "var(--text-primary)",
          fontSize: 12,
          textAlign: "right",
          cursor: disabled ? "not-allowed" : "text",
        }}
      />
    </div>
  );
}

function MetricCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="card" style={{ padding: "var(--space-3)", textAlign: "center" }}>
      <div style={{ fontSize: 10, color: "var(--text-muted)", marginBottom: 2 }}>{label}</div>
      <div style={{ fontFamily: "var(--font-mono)", fontSize: 18, fontWeight: 600 }}>{value}</div>
    </div>
  );
}

function PresetButton({
  label,
  description,
  onClick,
}: {
  label: string;
  description: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "start",
        padding: "var(--space-3)",
        background: "var(--bg-secondary)",
        border: "1px solid var(--border)",
        borderRadius: 6,
        cursor: "pointer",
        textAlign: "left",
      }}
    >
      <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)" }}>{label}</span>
      <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{description}</span>
    </button>
  );
}

const labelStyle: React.CSSProperties = {
  display: "block",
  fontSize: 11,
  fontWeight: 600,
  color: "var(--text-muted)",
  marginBottom: 4,
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "8px 12px",
  background: "var(--bg-secondary)",
  border: "1px solid var(--border)",
  borderRadius: 6,
  color: "var(--text-primary)",
  fontSize: 13,
};

export default RuVectorTab;
