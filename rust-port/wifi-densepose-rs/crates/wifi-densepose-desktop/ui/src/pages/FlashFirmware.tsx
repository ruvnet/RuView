import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SerialPort, Chip, FlashProgress, FlashPhase } from "../types";

type WizardStep = 1 | 2 | 3;

export function FlashFirmware() {
  const [step, setStep] = useState<WizardStep>(1);
  const [ports, setPorts] = useState<SerialPort[]>([]);
  const [selectedPort, setSelectedPort] = useState("");
  const [firmwarePath, setFirmwarePath] = useState("");
  const [chip, setChip] = useState<Chip>("esp32s3");
  const [baud, setBaud] = useState(460800);
  const [isLoadingPorts, setIsLoadingPorts] = useState(false);
  const [progress, setProgress] = useState<FlashProgress | null>(null);
  const [isFlashing, setIsFlashing] = useState(false);
  const [flashResult, setFlashResult] = useState<{ success: boolean; message: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadPorts = useCallback(async () => {
    setIsLoadingPorts(true);
    setError(null);
    try {
      const result = await invoke<SerialPort[]>("list_serial_ports");
      setPorts(result);
      if (result.length === 1) setSelectedPort(result[0].name);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoadingPorts(false);
    }
  }, []);

  useEffect(() => { loadPorts(); }, [loadPorts]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<FlashProgress>("flash-progress", (event) => {
      setProgress(event.payload);
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
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

  const startFlash = async () => {
    if (!selectedPort || !firmwarePath) return;
    setIsFlashing(true);
    setFlashResult(null);
    setProgress(null);
    setError(null);
    try {
      await invoke("flash_firmware", { port: selectedPort, firmwarePath, chip, baud });
      setFlashResult({ success: true, message: "Firmware flashed successfully." });
    } catch (err) {
      setFlashResult({ success: false, message: err instanceof Error ? err.message : String(err) });
    } finally {
      setIsFlashing(false);
    }
  };

  const canProceed = (s: WizardStep): boolean => {
    if (s === 1) return selectedPort !== "";
    if (s === 2) return firmwarePath !== "";
    return false;
  };

  return (
    <div style={{ padding: "var(--space-5)", maxWidth: 700 }}>
      <h1 className="heading-lg" style={{ margin: "0 0 var(--space-1)" }}>Flash Firmware</h1>
      <p style={{ fontSize: 13, color: "var(--text-secondary)", marginBottom: "var(--space-5)" }}>
        Flash firmware to an ESP32 via serial connection
      </p>

      <StepIndicator current={step} />

      {error && (
        <div style={bannerStyle("var(--status-error)")}>
          {error}
        </div>
      )}

      {/* Step 1: Select Serial Port */}
      {step === 1 && (
        <div style={cardStyle}>
          <h2 style={stepTitleStyle}>Step 1: Select Serial Port</h2>
          <p style={stepDescStyle}>Connect your ESP32 via USB and select the serial port.</p>

          <div style={{ marginBottom: "var(--space-4)" }}>
            <label style={labelStyle}>Serial Port</label>
            <div style={{ display: "flex", gap: "var(--space-2)" }}>
              <select
                value={selectedPort}
                onChange={(e) => setSelectedPort(e.target.value)}
                style={{ flex: 1 }}
                disabled={isLoadingPorts}
              >
                <option value="">
                  {isLoadingPorts ? "Loading..." : ports.length === 0 ? "No ports detected" : "Select a port..."}
                </option>
                {ports.map((p) => (
                  <option key={p.name} value={p.name}>
                    {p.name}{p.description ? ` - ${p.description}` : ""}{p.chip ? ` (${p.chip.toUpperCase()})` : ""}
                  </option>
                ))}
              </select>
              <button onClick={loadPorts} style={secondaryBtn} disabled={isLoadingPorts}>Refresh</button>
            </div>
          </div>

          <div style={{ display: "flex", justifyContent: "flex-end" }}>
            <button onClick={() => setStep(2)} disabled={!canProceed(1)} style={canProceed(1) ? primaryBtn : disabledBtn}>
              Next
            </button>
          </div>
        </div>
      )}

      {/* Step 2: Select Firmware */}
      {step === 2 && (
        <div style={cardStyle}>
          <h2 style={stepTitleStyle}>Step 2: Select Firmware</h2>
          <p style={stepDescStyle}>Choose the firmware binary file and chip configuration.</p>

          <div style={{ marginBottom: "var(--space-4)" }}>
            <label style={labelStyle}>Firmware Binary (.bin)</label>
            <div style={{ display: "flex", gap: "var(--space-2)" }}>
              <input type="text" value={firmwarePath} readOnly placeholder="No file selected" style={{ flex: 1 }} />
              <button onClick={pickFirmware} style={secondaryBtn}>Browse</button>
            </div>
          </div>

          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-4)", marginBottom: "var(--space-4)" }}>
            <div>
              <label style={labelStyle}>Chip</label>
              <select value={chip} onChange={(e) => setChip(e.target.value as Chip)}>
                <option value="esp32">ESP32</option>
                <option value="esp32s3">ESP32-S3</option>
                <option value="esp32c3">ESP32-C3</option>
              </select>
            </div>
            <div>
              <label style={labelStyle}>Baud Rate</label>
              <select value={baud} onChange={(e) => setBaud(Number(e.target.value))}>
                <option value={115200}>115200</option>
                <option value={230400}>230400</option>
                <option value={460800}>460800</option>
                <option value={921600}>921600</option>
              </select>
            </div>
          </div>

          <div style={{ display: "flex", justifyContent: "space-between" }}>
            <button onClick={() => setStep(1)} style={secondaryBtn}>Back</button>
            <button onClick={() => setStep(3)} disabled={!canProceed(2)} style={canProceed(2) ? primaryBtn : disabledBtn}>
              Next
            </button>
          </div>
        </div>
      )}

      {/* Step 3: Flash */}
      {step === 3 && (
        <div style={cardStyle}>
          <h2 style={stepTitleStyle}>Step 3: Flash</h2>

          {/* Summary */}
          <div
            style={{
              background: "var(--bg-base)",
              borderRadius: 6,
              padding: "var(--space-3) var(--space-4)",
              marginBottom: "var(--space-4)",
              display: "grid",
              gridTemplateColumns: "1fr 1fr",
              gap: "var(--space-2)",
              fontSize: 12,
            }}
          >
            <SummaryField label="Port" value={selectedPort} />
            <SummaryField label="Firmware" value={firmwarePath.split(/[\\/]/).pop() ?? firmwarePath} />
            <SummaryField label="Chip" value={chip.toUpperCase()} />
            <SummaryField label="Baud" value={String(baud)} />
          </div>

          {/* Progress */}
          {(isFlashing || progress) && !flashResult && (
            <div style={{ marginBottom: "var(--space-4)" }}>
              <ProgressBar progress={progress} />
            </div>
          )}

          {/* Result */}
          {flashResult && (
            <div style={bannerStyle(flashResult.success ? "var(--status-online)" : "var(--status-error)")}>
              {flashResult.message}
            </div>
          )}

          <div style={{ display: "flex", justifyContent: "space-between" }}>
            <button
              onClick={() => { setStep(2); setFlashResult(null); setProgress(null); }}
              style={secondaryBtn}
              disabled={isFlashing}
            >
              Back
            </button>
            {flashResult ? (
              <button
                onClick={() => { setStep(1); setFlashResult(null); setProgress(null); setFirmwarePath(""); setSelectedPort(""); }}
                style={primaryBtn}
              >
                Flash Another
              </button>
            ) : (
              <button onClick={startFlash} disabled={isFlashing} style={isFlashing ? disabledBtn : primaryBtn}>
                {isFlashing ? "Flashing..." : "Start Flash"}
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// --- Sub-components ---

function StepIndicator({ current }: { current: WizardStep }) {
  const steps = [
    { n: 1, label: "Select Port" },
    { n: 2, label: "Select Firmware" },
    { n: 3, label: "Flash" },
  ];

  return (
    <div style={{ display: "flex", alignItems: "center", marginBottom: "var(--space-5)" }}>
      {steps.map(({ n, label }, i) => {
        const isActive = n === current;
        const isDone = n < current;
        return (
          <div key={n} style={{ display: "flex", alignItems: "center" }}>
            <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
              <div
                style={{
                  width: 28,
                  height: 28,
                  borderRadius: "50%",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  fontSize: 12,
                  fontWeight: 700,
                  fontFamily: "var(--font-mono)",
                  background: isActive ? "var(--accent)" : isDone ? "rgba(63, 185, 80, 0.2)" : "var(--border)",
                  color: isActive ? "#fff" : isDone ? "var(--status-online)" : "var(--text-muted)",
                }}
              >
                {isDone ? "\u2713" : n}
              </div>
              <span
                style={{
                  fontSize: 12,
                  fontWeight: isActive ? 600 : 400,
                  color: isActive ? "var(--text-primary)" : "var(--text-muted)",
                }}
              >
                {label}
              </span>
            </div>
            {i < steps.length - 1 && (
              <div style={{ width: 40, height: 1, background: "var(--border)", margin: "0 var(--space-3)" }} />
            )}
          </div>
        );
      })}
    </div>
  );
}

const PHASE_LABELS: Record<FlashPhase, string> = {
  connecting: "Connecting...",
  erasing: "Erasing flash...",
  writing: "Writing firmware...",
  verifying: "Verifying...",
  done: "Complete",
  error: "Error",
};

function ProgressBar({ progress }: { progress: FlashProgress | null }) {
  const pct = progress?.progress_pct ?? 0;
  const phase = progress?.phase ?? "connecting";
  const speed = progress?.speed_bps ?? 0;
  const speedKB = speed > 0 ? `${(speed / 1024).toFixed(1)} KB/s` : "";

  return (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12, marginBottom: 6 }}>
        <span style={{ color: "var(--text-secondary)" }}>{PHASE_LABELS[phase]}</span>
        <span style={{ color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
          {pct.toFixed(1)}%{speedKB && ` | ${speedKB}`}
        </span>
      </div>
      <div style={{ width: "100%", height: 8, background: "var(--border)", borderRadius: 4, overflow: "hidden" }}>
        <div
          style={{
            width: `${Math.min(pct, 100)}%`,
            height: "100%",
            background: phase === "error" ? "var(--status-error)" : phase === "done" ? "var(--status-online)" : "var(--accent)",
            borderRadius: 4,
            transition: "width 0.3s ease",
            animation: phase === "writing" ? "pulse-accent 2s infinite" : "none",
          }}
        />
      </div>
    </div>
  );
}

function SummaryField({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div style={{ fontSize: 10, textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--text-muted)", marginBottom: 1 }}>
        {label}
      </div>
      <div style={{ color: "var(--text-secondary)", fontFamily: "var(--font-mono)", fontSize: 12, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        {value}
      </div>
    </div>
  );
}

// --- Shared styles ---

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

const stepTitleStyle: React.CSSProperties = {
  fontSize: 16,
  fontWeight: 600,
  color: "var(--text-primary)",
  margin: "0 0 var(--space-1)",
  fontFamily: "var(--font-sans)",
};

const stepDescStyle: React.CSSProperties = {
  fontSize: 13,
  color: "var(--text-secondary)",
  marginBottom: "var(--space-4)",
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

const disabledBtn: React.CSSProperties = {
  ...primaryBtn,
  background: "var(--bg-active)",
  color: "var(--text-muted)",
};
