import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface TrainingMetrics {
  epoch: number;
  train_loss: number;
  val_loss: number;
  train_acc: number;
  val_acc: number;
  learning_rate: number;
  timestamp: string;
}

interface EvaluationMetrics {
  pck_05: number;
  pck_10: number;
  pck_20: number;
  map_50: number;
  map_75: number;
  iou: number;
}

interface JointAccuracy {
  joint: string;
  accuracy: number;
}

const JOINT_NAMES = [
  "nose",
  "left_eye",
  "right_eye",
  "left_ear",
  "right_ear",
  "left_shoulder",
  "right_shoulder",
  "left_elbow",
  "right_elbow",
  "left_wrist",
  "right_wrist",
  "left_hip",
  "right_hip",
  "left_knee",
  "right_knee",
  "left_ankle",
  "right_ankle",
];

const MetricsTab: React.FC = () => {
  const [trainingHistory, setTrainingHistory] = useState<TrainingMetrics[]>([]);
  const [evaluation, setEvaluation] = useState<EvaluationMetrics | null>(null);
  const [jointAccuracies, setJointAccuracies] = useState<JointAccuracy[]>([]);
  const [selectedMetric, setSelectedMetric] = useState<"loss" | "accuracy">("loss");
  const [exporting, setExporting] = useState(false);

  useEffect(() => {
    loadMetrics();
  }, []);

  const loadMetrics = async () => {
    try {
      const metrics = await invoke<TrainingMetrics[]>("get_training_history");
      setTrainingHistory(metrics);
      const evalMetrics = await invoke<EvaluationMetrics>("get_evaluation_metrics");
      setEvaluation(evalMetrics);
      const joints = await invoke<JointAccuracy[]>("get_joint_accuracies");
      setJointAccuracies(joints);
    } catch (err) {
      // Generate mock data for demonstration
      const mockHistory: TrainingMetrics[] = [];
      for (let i = 1; i <= 50; i++) {
        mockHistory.push({
          epoch: i,
          train_loss: 0.5 * Math.exp(-i / 20) + 0.02 + Math.random() * 0.01,
          val_loss: 0.55 * Math.exp(-i / 18) + 0.025 + Math.random() * 0.015,
          train_acc: 1 - 0.5 * Math.exp(-i / 15) - Math.random() * 0.02,
          val_acc: 1 - 0.55 * Math.exp(-i / 15) - Math.random() * 0.025,
          learning_rate: 0.001 * Math.pow(0.95, Math.floor(i / 10)),
          timestamp: new Date(Date.now() - (50 - i) * 60000).toISOString(),
        });
      }
      setTrainingHistory(mockHistory);

      setEvaluation({
        pck_05: 0.72,
        pck_10: 0.89,
        pck_20: 0.96,
        map_50: 0.84,
        map_75: 0.71,
        iou: 0.78,
      });

      setJointAccuracies(
        JOINT_NAMES.map((joint) => ({
          joint,
          accuracy: 0.7 + Math.random() * 0.25,
        }))
      );
    }
  };

  const exportMetrics = async (format: "csv" | "json" | "tensorboard") => {
    setExporting(true);
    try {
      if (format === "json") {
        const data = {
          training: trainingHistory,
          evaluation,
          joints: jointAccuracies,
        };
        const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
        downloadBlob(blob, "metrics.json");
      } else if (format === "csv") {
        const headers = "epoch,train_loss,val_loss,train_acc,val_acc,learning_rate\n";
        const rows = trainingHistory
          .map(
            (m) =>
              `${m.epoch},${m.train_loss.toFixed(6)},${m.val_loss.toFixed(6)},${m.train_acc.toFixed(4)},${m.val_acc.toFixed(4)},${m.learning_rate.toExponential(2)}`
          )
          .join("\n");
        const blob = new Blob([headers + rows], { type: "text/csv" });
        downloadBlob(blob, "training_history.csv");
      } else {
        // TensorBoard format would require server-side handling
        alert("TensorBoard export requires running the backend server");
      }
    } finally {
      setExporting(false);
    }
  };

  const downloadBlob = (blob: Blob, filename: string) => {
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  };

  const maxLoss = Math.max(
    ...trainingHistory.map((m) => Math.max(m.train_loss, m.val_loss)),
    0.1
  );

  return (
    <div>
      {/* Summary Stats */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(4, 1fr)",
          gap: "var(--space-4)",
          marginBottom: "var(--space-5)",
        }}
      >
        <StatCard
          label="Epochs Trained"
          value={trainingHistory.length}
        />
        <StatCard
          label="Best Val Loss"
          value={
            trainingHistory.length > 0
              ? Math.min(...trainingHistory.map((m) => m.val_loss)).toFixed(4)
              : "—"
          }
          color="var(--status-online)"
        />
        <StatCard
          label="Best Val Acc"
          value={
            trainingHistory.length > 0
              ? `${(Math.max(...trainingHistory.map((m) => m.val_acc)) * 100).toFixed(1)}%`
              : "—"
          }
          color="var(--accent)"
        />
        <StatCard
          label="PCK@0.1"
          value={evaluation ? `${(evaluation.pck_10 * 100).toFixed(1)}%` : "—"}
        />
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "2fr 1fr", gap: "var(--space-5)" }}>
        {/* Loss/Accuracy Charts */}
        <div className="card" style={{ padding: "var(--space-4)" }}>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              marginBottom: "var(--space-4)",
            }}
          >
            <h3 style={{ margin: 0, fontSize: 14, fontWeight: 600 }}>Training Curves</h3>
            <div style={{ display: "flex", gap: "var(--space-2)" }}>
              <button
                onClick={() => setSelectedMetric("loss")}
                style={{
                  padding: "6px 12px",
                  background: selectedMetric === "loss" ? "var(--accent)" : "transparent",
                  border: `1px solid ${selectedMetric === "loss" ? "var(--accent)" : "var(--border)"}`,
                  borderRadius: 4,
                  color: selectedMetric === "loss" ? "white" : "var(--text-secondary)",
                  fontSize: 11,
                  fontWeight: 600,
                  cursor: "pointer",
                }}
              >
                Loss
              </button>
              <button
                onClick={() => setSelectedMetric("accuracy")}
                style={{
                  padding: "6px 12px",
                  background: selectedMetric === "accuracy" ? "var(--accent)" : "transparent",
                  border: `1px solid ${selectedMetric === "accuracy" ? "var(--accent)" : "var(--border)"}`,
                  borderRadius: 4,
                  color: selectedMetric === "accuracy" ? "white" : "var(--text-secondary)",
                  fontSize: 11,
                  fontWeight: 600,
                  cursor: "pointer",
                }}
              >
                Accuracy
              </button>
            </div>
          </div>

          {/* Chart Area */}
          <div
            style={{
              height: 250,
              position: "relative",
              background: "var(--bg-secondary)",
              borderRadius: 8,
              padding: "var(--space-3)",
            }}
          >
            {trainingHistory.length === 0 ? (
              <div
                style={{
                  display: "flex",
                  flexDirection: "column",
                  alignItems: "center",
                  justifyContent: "center",
                  height: "100%",
                  color: "var(--text-muted)",
                }}
              >
                <span style={{ fontSize: 32 }}>📊</span>
                <p style={{ fontSize: 13, marginTop: "var(--space-2)" }}>
                  No training data yet
                </p>
              </div>
            ) : (
              <svg width="100%" height="100%" viewBox="0 0 500 200" preserveAspectRatio="none">
                {/* Grid lines */}
                {[0, 0.25, 0.5, 0.75, 1].map((y) => (
                  <line
                    key={y}
                    x1="0"
                    y1={y * 180}
                    x2="500"
                    y2={y * 180}
                    stroke="var(--border)"
                    strokeWidth="0.5"
                    strokeDasharray="4"
                  />
                ))}

                {/* Train line */}
                <polyline
                  fill="none"
                  stroke="var(--accent)"
                  strokeWidth="2"
                  points={trainingHistory
                    .map((m, i) => {
                      const x = (i / (trainingHistory.length - 1)) * 500;
                      const value = selectedMetric === "loss" ? m.train_loss : m.train_acc;
                      const y =
                        selectedMetric === "loss"
                          ? (value / maxLoss) * 180
                          : (1 - value) * 180;
                      return `${x},${y}`;
                    })
                    .join(" ")}
                />

                {/* Val line */}
                <polyline
                  fill="none"
                  stroke="var(--status-online)"
                  strokeWidth="2"
                  points={trainingHistory
                    .map((m, i) => {
                      const x = (i / (trainingHistory.length - 1)) * 500;
                      const value = selectedMetric === "loss" ? m.val_loss : m.val_acc;
                      const y =
                        selectedMetric === "loss"
                          ? (value / maxLoss) * 180
                          : (1 - value) * 180;
                      return `${x},${y}`;
                    })
                    .join(" ")}
                />
              </svg>
            )}

            {/* Legend */}
            <div
              style={{
                position: "absolute",
                top: "var(--space-2)",
                right: "var(--space-2)",
                display: "flex",
                gap: "var(--space-3)",
                fontSize: 11,
              }}
            >
              <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
                <span
                  style={{
                    width: 12,
                    height: 3,
                    background: "var(--accent)",
                    borderRadius: 2,
                  }}
                />
                Train
              </span>
              <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
                <span
                  style={{
                    width: 12,
                    height: 3,
                    background: "var(--status-online)",
                    borderRadius: 2,
                  }}
                />
                Validation
              </span>
            </div>
          </div>
        </div>

        {/* Evaluation Metrics */}
        <div className="card" style={{ padding: "var(--space-4)" }}>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 600, marginBottom: "var(--space-4)" }}>
            Evaluation Metrics
          </h3>

          {!evaluation ? (
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                height: 200,
                color: "var(--text-muted)",
              }}
            >
              <span style={{ fontSize: 32 }}>📏</span>
              <p style={{ fontSize: 13, marginTop: "var(--space-2)" }}>
                Run evaluation to see metrics
              </p>
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-3)" }}>
              <MetricBar label="PCK@0.05" value={evaluation.pck_05} color="#f59e0b" />
              <MetricBar label="PCK@0.10" value={evaluation.pck_10} color="var(--accent)" />
              <MetricBar label="PCK@0.20" value={evaluation.pck_20} color="var(--status-online)" />
              <div style={{ height: 1, background: "var(--border)", margin: "var(--space-2) 0" }} />
              <MetricBar label="mAP@0.50" value={evaluation.map_50} color="#a855f7" />
              <MetricBar label="mAP@0.75" value={evaluation.map_75} color="#ec4899" />
              <MetricBar label="IoU" value={evaluation.iou} color="#06b6d4" />
            </div>
          )}
        </div>
      </div>

      {/* Joint-wise Accuracy */}
      <div className="card" style={{ marginTop: "var(--space-5)", padding: "var(--space-4)" }}>
        <h3 style={{ margin: 0, fontSize: 14, fontWeight: 600, marginBottom: "var(--space-4)" }}>
          Per-Joint Accuracy
        </h3>

        {jointAccuracies.length === 0 ? (
          <div
            style={{
              textAlign: "center",
              padding: "var(--space-5)",
              color: "var(--text-muted)",
            }}
          >
            No joint accuracy data available
          </div>
        ) : (
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fill, minmax(140px, 1fr))",
              gap: "var(--space-3)",
            }}
          >
            {jointAccuracies.map((ja) => (
              <div
                key={ja.joint}
                style={{
                  padding: "var(--space-3)",
                  background: "var(--bg-secondary)",
                  borderRadius: 6,
                  textAlign: "center",
                }}
              >
                <div
                  style={{
                    fontSize: 11,
                    color: "var(--text-muted)",
                    marginBottom: 4,
                    textTransform: "capitalize",
                  }}
                >
                  {ja.joint.replace("_", " ")}
                </div>
                <div
                  style={{
                    fontFamily: "var(--font-mono)",
                    fontSize: 18,
                    fontWeight: 600,
                    color:
                      ja.accuracy > 0.9
                        ? "var(--status-online)"
                        : ja.accuracy > 0.8
                          ? "var(--accent)"
                          : ja.accuracy > 0.7
                            ? "#f59e0b"
                            : "var(--status-error)",
                  }}
                >
                  {(ja.accuracy * 100).toFixed(1)}%
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Export Section */}
      <div
        className="card"
        style={{
          marginTop: "var(--space-5)",
          padding: "var(--space-4)",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <div>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 600 }}>Export Metrics</h3>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>
            Download training history and evaluation results
          </p>
        </div>
        <div style={{ display: "flex", gap: "var(--space-2)" }}>
          <button
            onClick={() => exportMetrics("csv")}
            disabled={exporting || trainingHistory.length === 0}
            style={{
              padding: "8px 16px",
              background: "rgba(56, 139, 253, 0.1)",
              border: "1px solid rgba(56, 139, 253, 0.3)",
              borderRadius: 6,
              color: "var(--accent)",
              fontSize: 12,
              fontWeight: 600,
              cursor: trainingHistory.length === 0 ? "not-allowed" : "pointer",
              opacity: trainingHistory.length === 0 ? 0.5 : 1,
            }}
          >
            CSV
          </button>
          <button
            onClick={() => exportMetrics("json")}
            disabled={exporting || trainingHistory.length === 0}
            style={{
              padding: "8px 16px",
              background: "rgba(56, 139, 253, 0.1)",
              border: "1px solid rgba(56, 139, 253, 0.3)",
              borderRadius: 6,
              color: "var(--accent)",
              fontSize: 12,
              fontWeight: 600,
              cursor: trainingHistory.length === 0 ? "not-allowed" : "pointer",
              opacity: trainingHistory.length === 0 ? 0.5 : 1,
            }}
          >
            JSON
          </button>
          <button
            onClick={() => exportMetrics("tensorboard")}
            disabled={exporting || trainingHistory.length === 0}
            style={{
              padding: "8px 16px",
              background: "transparent",
              border: "1px solid var(--border)",
              borderRadius: 6,
              color: "var(--text-secondary)",
              fontSize: 12,
              fontWeight: 600,
              cursor: trainingHistory.length === 0 ? "not-allowed" : "pointer",
              opacity: trainingHistory.length === 0 ? 0.5 : 1,
            }}
          >
            TensorBoard
          </button>
        </div>
      </div>
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

function MetricBar({
  label,
  value,
  color,
}: {
  label: string;
  value: number;
  color: string;
}) {
  return (
    <div>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          fontSize: 12,
          marginBottom: 4,
        }}
      >
        <span>{label}</span>
        <span style={{ fontFamily: "var(--font-mono)", fontWeight: 600 }}>
          {(value * 100).toFixed(1)}%
        </span>
      </div>
      <div
        style={{
          height: 6,
          background: "var(--bg-secondary)",
          borderRadius: 3,
          overflow: "hidden",
        }}
      >
        <div
          style={{
            width: `${value * 100}%`,
            height: "100%",
            background: color,
            borderRadius: 3,
            transition: "width 0.5s",
          }}
        />
      </div>
    </div>
  );
}

export default MetricsTab;
