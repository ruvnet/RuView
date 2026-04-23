import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

interface TrainingConfig {
  dataset_id: string;
  model_id: string;
  epochs: number;
  batch_size: number;
  learning_rate: number;
  optimizer: "adam" | "sgd" | "adamw";
  weight_decay: number;
  use_augmentation: boolean;
  checkpoint_every: number;
}

interface TrainingProgress {
  epoch: number;
  total_epochs: number;
  batch: number;
  total_batches: number;
  train_loss: number;
  val_loss: number | null;
  learning_rate: number;
  eta_secs: number;
  gpu_memory_mb: number | null;
}

interface TrainingJob {
  id: string;
  status: "running" | "paused" | "completed" | "failed";
  started_at: string;
  progress: TrainingProgress;
}

const DEFAULT_CONFIG: TrainingConfig = {
  dataset_id: "mmfi",
  model_id: "csi-encoder-cnn",
  epochs: 100,
  batch_size: 32,
  learning_rate: 0.001,
  optimizer: "adam",
  weight_decay: 0.0001,
  use_augmentation: true,
  checkpoint_every: 10,
};

interface TrainingTabProps {
  gpuAvailable: boolean;
}

const TrainingTab: React.FC<TrainingTabProps> = ({ gpuAvailable }) => {
  const [config, setConfig] = useState<TrainingConfig>(DEFAULT_CONFIG);
  const [currentJob, setCurrentJob] = useState<TrainingJob | null>(null);
  const [lossHistory, setLossHistory] = useState<{ epoch: number; train: number; val: number }[]>(
    []
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    const setupListener = async () => {
      try {
        unlisten = await listen<TrainingProgress>("training:progress", (event) => {
          const progress = event.payload;
          setCurrentJob((prev) =>
            prev
              ? { ...prev, progress }
              : {
                  id: "job-1",
                  status: "running",
                  started_at: new Date().toISOString(),
                  progress,
                }
          );

          if (progress.val_loss !== null && progress.batch === progress.total_batches) {
            setLossHistory((prev) => [
              ...prev,
              {
                epoch: progress.epoch,
                train: progress.train_loss,
                val: progress.val_loss!,
              },
            ]);
          }
        });
      } catch (err) {
        console.error("Failed to setup training listener:", err);
      }
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const handleStartTraining = async () => {
    setError(null);
    try {
      await invoke("start_training", { config });
      setCurrentJob({
        id: `job-${Date.now()}`,
        status: "running",
        started_at: new Date().toISOString(),
        progress: {
          epoch: 0,
          total_epochs: config.epochs,
          batch: 0,
          total_batches: 0,
          train_loss: 0,
          val_loss: null,
          learning_rate: config.learning_rate,
          eta_secs: 0,
          gpu_memory_mb: null,
        },
      });
    } catch (err) {
      setError(`Failed to start training: ${err}`);
    }
  };

  const handleStopTraining = async () => {
    try {
      await invoke("stop_training");
      setCurrentJob((prev) => (prev ? { ...prev, status: "paused" } : null));
    } catch (err) {
      setError(`Failed to stop training: ${err}`);
    }
  };

  const formatEta = (seconds: number) => {
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
    const hours = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    return `${hours}h ${mins}m`;
  };

  const progress = currentJob?.progress;
  const epochProgress = progress ? (progress.epoch / progress.total_epochs) * 100 : 0;
  const batchProgress = progress && progress.total_batches > 0
    ? (progress.batch / progress.total_batches) * 100
    : 0;

  return (
    <div>
      {/* GPU Warning */}
      {!gpuAvailable && (
        <div
          style={{
            background: "rgba(245, 158, 11, 0.1)",
            border: "1px solid rgba(245, 158, 11, 0.3)",
            borderRadius: 6,
            padding: "var(--space-3)",
            marginBottom: "var(--space-4)",
            display: "flex",
            alignItems: "center",
            gap: "var(--space-3)",
          }}
        >
          <span style={{ fontSize: 18 }}>⚠️</span>
          <div>
            <div style={{ fontWeight: 600, fontSize: 13, color: "#f59e0b" }}>
              GPU Not Available
            </div>
            <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
              Training will use CPU, which is significantly slower. Consider using a
              machine with CUDA or Metal support.
            </div>
          </div>
        </div>
      )}

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
            Training Configuration
          </h3>

          <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-3)" }}>
            <div>
              <label style={labelStyle}>Dataset</label>
              <select
                value={config.dataset_id}
                onChange={(e) => setConfig({ ...config, dataset_id: e.target.value })}
                style={inputStyle}
              >
                <option value="mmfi">MM-Fi Dataset</option>
                <option value="wipose">Wi-Pose Dataset</option>
                <option value="wiar">WiAR Dataset</option>
              </select>
            </div>

            <div>
              <label style={labelStyle}>Model Architecture</label>
              <select
                value={config.model_id}
                onChange={(e) => setConfig({ ...config, model_id: e.target.value })}
                style={inputStyle}
              >
                <option value="csi-encoder-cnn">CSI Encoder (CNN)</option>
                <option value="csi-encoder-transformer">CSI Encoder (Transformer)</option>
                <option value="pose-decoder-lstm">Pose Decoder (LSTM)</option>
                <option value="pose-decoder-gru">Pose Decoder (GRU)</option>
              </select>
            </div>

            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-3)" }}>
              <div>
                <label style={labelStyle}>Epochs</label>
                <input
                  type="number"
                  value={config.epochs}
                  onChange={(e) => setConfig({ ...config, epochs: parseInt(e.target.value) || 1 })}
                  min={1}
                  max={1000}
                  style={inputStyle}
                />
              </div>
              <div>
                <label style={labelStyle}>Batch Size</label>
                <input
                  type="number"
                  value={config.batch_size}
                  onChange={(e) =>
                    setConfig({ ...config, batch_size: parseInt(e.target.value) || 1 })
                  }
                  min={1}
                  max={512}
                  style={inputStyle}
                />
              </div>
            </div>

            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-3)" }}>
              <div>
                <label style={labelStyle}>Learning Rate</label>
                <input
                  type="number"
                  value={config.learning_rate}
                  onChange={(e) =>
                    setConfig({ ...config, learning_rate: parseFloat(e.target.value) || 0.001 })
                  }
                  step={0.0001}
                  min={0.00001}
                  max={1}
                  style={inputStyle}
                />
              </div>
              <div>
                <label style={labelStyle}>Optimizer</label>
                <select
                  value={config.optimizer}
                  onChange={(e) =>
                    setConfig({ ...config, optimizer: e.target.value as TrainingConfig["optimizer"] })
                  }
                  style={inputStyle}
                >
                  <option value="adam">Adam</option>
                  <option value="adamw">AdamW</option>
                  <option value="sgd">SGD</option>
                </select>
              </div>
            </div>

            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-3)" }}>
              <div>
                <label style={labelStyle}>Weight Decay</label>
                <input
                  type="number"
                  value={config.weight_decay}
                  onChange={(e) =>
                    setConfig({ ...config, weight_decay: parseFloat(e.target.value) || 0 })
                  }
                  step={0.0001}
                  min={0}
                  max={1}
                  style={inputStyle}
                />
              </div>
              <div>
                <label style={labelStyle}>Checkpoint Every</label>
                <input
                  type="number"
                  value={config.checkpoint_every}
                  onChange={(e) =>
                    setConfig({ ...config, checkpoint_every: parseInt(e.target.value) || 1 })
                  }
                  min={1}
                  max={100}
                  style={inputStyle}
                />
              </div>
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
              <input
                type="checkbox"
                id="augmentation"
                checked={config.use_augmentation}
                onChange={(e) => setConfig({ ...config, use_augmentation: e.target.checked })}
                style={{ width: 16, height: 16 }}
              />
              <label htmlFor="augmentation" style={{ fontSize: 13, cursor: "pointer" }}>
                Enable Data Augmentation
              </label>
            </div>

            <div style={{ marginTop: "var(--space-3)" }}>
              {currentJob?.status === "running" ? (
                <button
                  onClick={handleStopTraining}
                  style={{
                    width: "100%",
                    padding: "12px",
                    background: "rgba(248, 81, 73, 0.1)",
                    border: "1px solid rgba(248, 81, 73, 0.3)",
                    borderRadius: 6,
                    color: "var(--status-error)",
                    fontSize: 13,
                    fontWeight: 600,
                    cursor: "pointer",
                  }}
                >
                  Stop Training
                </button>
              ) : (
                <button
                  onClick={handleStartTraining}
                  className="btn-gradient"
                  style={{ width: "100%", padding: "12px", fontSize: 13 }}
                >
                  Start Training
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Progress Panel */}
        <div className="card" style={{ padding: "var(--space-4)" }}>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 600, marginBottom: "var(--space-4)" }}>
            Training Progress
          </h3>

          {!currentJob ? (
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                height: 300,
                color: "var(--text-muted)",
              }}
            >
              <div style={{ fontSize: 48, marginBottom: "var(--space-3)" }}>🎯</div>
              <p style={{ fontSize: 13 }}>No training job running</p>
              <p style={{ fontSize: 12 }}>Configure and start training to begin</p>
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-4)" }}>
              {/* Status */}
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
                  <span
                    style={{
                      width: 8,
                      height: 8,
                      borderRadius: "50%",
                      background:
                        currentJob.status === "running"
                          ? "var(--status-online)"
                          : currentJob.status === "paused"
                            ? "#f59e0b"
                            : "var(--status-error)",
                      animation: currentJob.status === "running" ? "pulse 1.5s infinite" : "none",
                    }}
                  />
                  <span style={{ fontSize: 13, fontWeight: 600, textTransform: "capitalize" }}>
                    {currentJob.status}
                  </span>
                </div>
                <span style={{ fontSize: 12, color: "var(--text-muted)" }}>
                  ETA: {formatEta(progress?.eta_secs ?? 0)}
                </span>
              </div>

              {/* Epoch Progress */}
              <div>
                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    fontSize: 12,
                    marginBottom: 4,
                  }}
                >
                  <span>Epoch</span>
                  <span>
                    {progress?.epoch ?? 0} / {progress?.total_epochs ?? config.epochs}
                  </span>
                </div>
                <div
                  style={{
                    height: 6,
                    background: "var(--border)",
                    borderRadius: 3,
                    overflow: "hidden",
                  }}
                >
                  <div
                    style={{
                      width: `${epochProgress}%`,
                      height: "100%",
                      background: "var(--accent)",
                      transition: "width 0.3s",
                    }}
                  />
                </div>
              </div>

              {/* Batch Progress */}
              <div>
                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    fontSize: 12,
                    marginBottom: 4,
                  }}
                >
                  <span>Batch</span>
                  <span>
                    {progress?.batch ?? 0} / {progress?.total_batches ?? 0}
                  </span>
                </div>
                <div
                  style={{
                    height: 4,
                    background: "var(--border)",
                    borderRadius: 2,
                    overflow: "hidden",
                  }}
                >
                  <div
                    style={{
                      width: `${batchProgress}%`,
                      height: "100%",
                      background: "rgba(56, 139, 253, 0.5)",
                      transition: "width 0.1s",
                    }}
                  />
                </div>
              </div>

              {/* Stats Grid */}
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(2, 1fr)",
                  gap: "var(--space-3)",
                }}
              >
                <div className="card" style={{ padding: "var(--space-3)" }}>
                  <div style={{ fontSize: 10, color: "var(--text-muted)", marginBottom: 4 }}>
                    Train Loss
                  </div>
                  <div style={{ fontFamily: "var(--font-mono)", fontSize: 20, fontWeight: 600 }}>
                    {progress?.train_loss.toFixed(4) ?? "—"}
                  </div>
                </div>
                <div className="card" style={{ padding: "var(--space-3)" }}>
                  <div style={{ fontSize: 10, color: "var(--text-muted)", marginBottom: 4 }}>
                    Val Loss
                  </div>
                  <div
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: 20,
                      fontWeight: 600,
                      color: "var(--status-online)",
                    }}
                  >
                    {progress?.val_loss?.toFixed(4) ?? "—"}
                  </div>
                </div>
                <div className="card" style={{ padding: "var(--space-3)" }}>
                  <div style={{ fontSize: 10, color: "var(--text-muted)", marginBottom: 4 }}>
                    Learning Rate
                  </div>
                  <div style={{ fontFamily: "var(--font-mono)", fontSize: 14, fontWeight: 600 }}>
                    {progress?.learning_rate.toExponential(2) ?? "—"}
                  </div>
                </div>
                <div className="card" style={{ padding: "var(--space-3)" }}>
                  <div style={{ fontSize: 10, color: "var(--text-muted)", marginBottom: 4 }}>
                    GPU Memory
                  </div>
                  <div style={{ fontFamily: "var(--font-mono)", fontSize: 14, fontWeight: 600 }}>
                    {progress?.gpu_memory_mb ? `${progress.gpu_memory_mb} MB` : "N/A"}
                  </div>
                </div>
              </div>

              {/* Mini Loss Chart */}
              {lossHistory.length > 0 && (
                <div>
                  <div style={{ fontSize: 12, fontWeight: 600, marginBottom: "var(--space-2)" }}>
                    Loss History
                  </div>
                  <div
                    style={{
                      height: 80,
                      display: "flex",
                      alignItems: "flex-end",
                      gap: 2,
                      padding: "var(--space-2)",
                      background: "var(--bg-secondary)",
                      borderRadius: 4,
                    }}
                  >
                    {lossHistory.slice(-20).map((h, i) => (
                      <div
                        key={i}
                        style={{
                          flex: 1,
                          height: `${Math.max(5, Math.min(100, (1 - h.train) * 100))}%`,
                          background: "var(--accent)",
                          borderRadius: 2,
                          opacity: 0.6 + (i / 20) * 0.4,
                        }}
                        title={`Epoch ${h.epoch}: Train=${h.train.toFixed(4)}, Val=${h.val.toFixed(4)}`}
                      />
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      <style>{`
        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.5; }
        }
      `}</style>
    </div>
  );
};

const labelStyle: React.CSSProperties = {
  display: "block",
  fontSize: 11,
  fontWeight: 600,
  color: "var(--text-muted)",
  marginBottom: 4,
  textTransform: "uppercase",
  letterSpacing: "0.04em",
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

export default TrainingTab;
