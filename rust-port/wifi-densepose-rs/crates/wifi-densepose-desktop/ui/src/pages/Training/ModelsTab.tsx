import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ModelArchitecture {
  id: string;
  name: string;
  type: "encoder" | "decoder" | "embedding" | "adaptor";
  description: string;
  params_m: number;
  memory_mb: number;
  paper?: string;
}

interface Checkpoint {
  id: string;
  model_id: string;
  name: string;
  epoch: number;
  val_loss: number;
  created_at: string;
  path: string;
  size_mb: number;
}

const MODEL_ARCHITECTURES: ModelArchitecture[] = [
  {
    id: "csi-encoder-cnn",
    name: "CSI Encoder (CNN)",
    type: "encoder",
    description: "Convolutional encoder for CSI amplitude/phase features",
    params_m: 2.3,
    memory_mb: 128,
  },
  {
    id: "csi-encoder-transformer",
    name: "CSI Encoder (Transformer)",
    type: "encoder",
    description: "Self-attention based CSI feature extraction",
    params_m: 8.5,
    memory_mb: 384,
    paper: "WiFi-ViT 2024",
  },
  {
    id: "pose-decoder-lstm",
    name: "Pose Decoder (LSTM)",
    type: "decoder",
    description: "Recurrent decoder for temporal pose estimation",
    params_m: 1.8,
    memory_mb: 96,
  },
  {
    id: "pose-decoder-gru",
    name: "Pose Decoder (GRU)",
    type: "decoder",
    description: "Gated recurrent unit pose decoder (faster)",
    params_m: 1.2,
    memory_mb: 64,
  },
  {
    id: "aether-embedding",
    name: "AETHER Embedding",
    type: "embedding",
    description: "Contrastive CSI embedding for person re-identification (ADR-024)",
    params_m: 4.2,
    memory_mb: 192,
    paper: "AETHER 2025",
  },
  {
    id: "meridian-adaptor",
    name: "MERIDIAN Adaptor",
    type: "adaptor",
    description: "Cross-environment domain generalization module (ADR-027)",
    params_m: 3.1,
    memory_mb: 144,
    paper: "MERIDIAN 2025",
  },
];

const ModelsTab: React.FC = () => {
  const [checkpoints, setCheckpoints] = useState<Checkpoint[]>([]);
  const [selectedModel, setSelectedModel] = useState<string | null>(null);
  const [exporting, setExporting] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadCheckpoints();
  }, []);

  const loadCheckpoints = async () => {
    try {
      const loaded = await invoke<Checkpoint[]>("list_checkpoints");
      setCheckpoints(loaded);
    } catch (err) {
      // Mock data if command not implemented
      setCheckpoints([
        {
          id: "ckpt-001",
          model_id: "csi-encoder-cnn",
          name: "CSI-CNN v1.2",
          epoch: 50,
          val_loss: 0.0234,
          created_at: "2026-03-08T14:30:00Z",
          path: "~/.ruview/models/csi-cnn-v1.2.pt",
          size_mb: 12.4,
        },
        {
          id: "ckpt-002",
          model_id: "pose-decoder-gru",
          name: "Pose-GRU v2.0",
          epoch: 100,
          val_loss: 0.0189,
          created_at: "2026-03-09T09:15:00Z",
          path: "~/.ruview/models/pose-gru-v2.pt",
          size_mb: 8.2,
        },
      ]);
    }
  };

  const handleExport = async (checkpointId: string, format: "onnx" | "torchscript") => {
    setExporting(checkpointId);
    setError(null);
    try {
      await invoke("export_model", { checkpointId, format });
      // Success notification would go here
    } catch (err) {
      setError(`Export failed: ${err}`);
    } finally {
      setExporting(null);
    }
  };

  const getTypeColor = (type: ModelArchitecture["type"]) => {
    switch (type) {
      case "encoder":
        return "var(--accent)";
      case "decoder":
        return "var(--status-online)";
      case "embedding":
        return "#a855f7";
      case "adaptor":
        return "#f59e0b";
    }
  };

  return (
    <div>
      {/* Stats Row */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(4, 1fr)",
          gap: "var(--space-4)",
          marginBottom: "var(--space-5)",
        }}
      >
        <StatCard label="Architectures" value={MODEL_ARCHITECTURES.length} />
        <StatCard
          label="Checkpoints"
          value={checkpoints.length}
          color="var(--status-online)"
        />
        <StatCard
          label="Total Params"
          value={`${MODEL_ARCHITECTURES.reduce((acc, m) => acc + m.params_m, 0).toFixed(1)}M`}
        />
        <StatCard
          label="Storage Used"
          value={`${checkpoints.reduce((acc, c) => acc + c.size_mb, 0).toFixed(1)} MB`}
        />
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

      {/* Model Architectures */}
      <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: "var(--space-3)" }}>
        Available Architectures
      </h3>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(300px, 1fr))",
          gap: "var(--space-3)",
          marginBottom: "var(--space-5)",
        }}
      >
        {MODEL_ARCHITECTURES.map((model) => (
          <div
            key={model.id}
            className="card"
            style={{
              padding: "var(--space-3)",
              cursor: "pointer",
              border:
                selectedModel === model.id
                  ? "1px solid var(--accent)"
                  : "1px solid transparent",
            }}
            onClick={() => setSelectedModel(model.id)}
          >
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "start",
                marginBottom: "var(--space-2)",
              }}
            >
              <div>
                <h4 style={{ margin: 0, fontSize: 14, fontWeight: 600 }}>
                  {model.name}
                </h4>
                <span
                  style={{
                    display: "inline-block",
                    marginTop: 4,
                    padding: "1px 6px",
                    borderRadius: 3,
                    fontSize: 10,
                    fontWeight: 600,
                    textTransform: "uppercase",
                    background: `${getTypeColor(model.type)}20`,
                    color: getTypeColor(model.type),
                  }}
                >
                  {model.type}
                </span>
              </div>
              {model.paper && (
                <span
                  style={{
                    fontSize: 10,
                    color: "var(--text-muted)",
                    fontStyle: "italic",
                  }}
                >
                  {model.paper}
                </span>
              )}
            </div>
            <p
              style={{
                fontSize: 11,
                color: "var(--text-muted)",
                margin: "var(--space-2) 0",
                lineHeight: 1.4,
              }}
            >
              {model.description}
            </p>
            <div
              style={{
                display: "flex",
                gap: "var(--space-3)",
                fontSize: 11,
                color: "var(--text-secondary)",
              }}
            >
              <span>🧮 {model.params_m}M params</span>
              <span>💾 {model.memory_mb} MB</span>
            </div>
          </div>
        ))}
      </div>

      {/* Checkpoints */}
      <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: "var(--space-3)" }}>
        Saved Checkpoints
      </h3>
      {checkpoints.length === 0 ? (
        <div
          className="card"
          style={{
            padding: "var(--space-5)",
            textAlign: "center",
            color: "var(--text-muted)",
          }}
        >
          <div style={{ fontSize: 32, marginBottom: "var(--space-2)" }}>📦</div>
          <p style={{ fontSize: 13 }}>No checkpoints saved yet</p>
          <p style={{ fontSize: 12 }}>Train a model to create checkpoints</p>
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
          {checkpoints.map((ckpt) => (
            <div
              key={ckpt.id}
              className="card"
              style={{
                padding: "var(--space-3)",
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
              }}
            >
              <div>
                <div style={{ fontWeight: 600, fontSize: 13 }}>{ckpt.name}</div>
                <div
                  style={{
                    fontSize: 11,
                    color: "var(--text-muted)",
                    marginTop: 2,
                  }}
                >
                  Epoch {ckpt.epoch} • Val Loss: {ckpt.val_loss.toFixed(4)} •{" "}
                  {ckpt.size_mb.toFixed(1)} MB
                </div>
              </div>
              <div style={{ display: "flex", gap: "var(--space-2)" }}>
                <button
                  onClick={() => handleExport(ckpt.id, "onnx")}
                  disabled={exporting === ckpt.id}
                  style={{
                    padding: "6px 12px",
                    background: "rgba(56, 139, 253, 0.1)",
                    border: "1px solid rgba(56, 139, 253, 0.3)",
                    borderRadius: 4,
                    color: "var(--accent)",
                    fontSize: 11,
                    fontWeight: 600,
                    cursor: exporting === ckpt.id ? "wait" : "pointer",
                    opacity: exporting === ckpt.id ? 0.6 : 1,
                  }}
                >
                  {exporting === ckpt.id ? "Exporting..." : "ONNX"}
                </button>
                <button
                  onClick={() => handleExport(ckpt.id, "torchscript")}
                  disabled={exporting === ckpt.id}
                  style={{
                    padding: "6px 12px",
                    background: "transparent",
                    border: "1px solid var(--border)",
                    borderRadius: 4,
                    color: "var(--text-secondary)",
                    fontSize: 11,
                    fontWeight: 600,
                    cursor: exporting === ckpt.id ? "wait" : "pointer",
                    opacity: exporting === ckpt.id ? 0.6 : 1,
                  }}
                >
                  TorchScript
                </button>
              </div>
            </div>
          ))}
        </div>
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
  value: number | string;
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

export default ModelsTab;
