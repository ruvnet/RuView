import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import DatasetsTab from "./DatasetsTab";
import ModelsTab from "./ModelsTab";
import TrainingTab from "./TrainingTab";
import RuVectorTab from "./RuVectorTab";
import MetricsTab from "./MetricsTab";

type TrainingTabType = "datasets" | "models" | "training" | "ruvector" | "metrics";

interface GpuInfo {
  available: boolean;
  name: string | null;
  memory_mb: number | null;
  cuda_version: string | null;
  metal_supported: boolean;
}

const Training: React.FC = () => {
  const [activeTab, setActiveTab] = useState<TrainingTabType>("datasets");
  const [gpuInfo, setGpuInfo] = useState<GpuInfo | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    detectGpu();
  }, []);

  const detectGpu = async () => {
    try {
      const info = await invoke<GpuInfo>("detect_gpu");
      setGpuInfo(info);
    } catch (err) {
      console.error("GPU detection failed:", err);
      setGpuInfo({
        available: false,
        name: null,
        memory_mb: null,
        cuda_version: null,
        metal_supported: false,
      });
    } finally {
      setLoading(false);
    }
  };

  const tabs: { id: TrainingTabType; label: string; icon: string }[] = [
    { id: "datasets", label: "Datasets", icon: "📊" },
    { id: "models", label: "Models", icon: "🧠" },
    { id: "training", label: "Training", icon: "⚡" },
    { id: "ruvector", label: "RuVector", icon: "📡" },
    { id: "metrics", label: "Metrics", icon: "📈" },
  ];

  return (
    <div style={{ padding: "var(--space-5)", maxWidth: 1400 }}>
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
            Training & Models
          </h1>
          <p
            style={{
              fontSize: 13,
              color: "var(--text-secondary)",
              marginTop: 4,
            }}
          >
            Train pose estimation models and configure RuVector signal processing
          </p>
        </div>

        {/* GPU Status */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "var(--space-3)",
            padding: "var(--space-3) var(--space-4)",
            background: gpuInfo?.available
              ? "rgba(63, 185, 80, 0.1)"
              : "rgba(139, 148, 158, 0.1)",
            border: `1px solid ${gpuInfo?.available ? "rgba(63, 185, 80, 0.3)" : "rgba(139, 148, 158, 0.3)"}`,
            borderRadius: 8,
          }}
        >
          <span style={{ fontSize: 18 }}>{gpuInfo?.available ? "🎮" : "💻"}</span>
          <div>
            <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)" }}>
              {loading
                ? "Detecting GPU..."
                : gpuInfo?.available
                  ? gpuInfo.name || "GPU Available"
                  : "CPU Mode"}
            </div>
            <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
              {gpuInfo?.cuda_version
                ? `CUDA ${gpuInfo.cuda_version}`
                : gpuInfo?.metal_supported
                  ? "Metal Supported"
                  : "No GPU acceleration"}
              {gpuInfo?.memory_mb && ` • ${Math.round(gpuInfo.memory_mb / 1024)}GB`}
            </div>
          </div>
        </div>
      </div>

      {/* Tabs */}
      <div
        style={{
          display: "flex",
          gap: "var(--space-1)",
          borderBottom: "1px solid var(--border)",
          marginBottom: "var(--space-5)",
        }}
      >
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              padding: "12px 20px",
              border: "none",
              background: "transparent",
              color: activeTab === tab.id ? "var(--accent)" : "var(--text-secondary)",
              fontSize: 13,
              fontWeight: 600,
              cursor: "pointer",
              borderBottom:
                activeTab === tab.id
                  ? "2px solid var(--accent)"
                  : "2px solid transparent",
              marginBottom: -1,
              transition: "color 0.15s, border-color 0.15s",
            }}
          >
            <span>{tab.icon}</span>
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab Content */}
      <div>
        {activeTab === "datasets" && <DatasetsTab />}
        {activeTab === "models" && <ModelsTab />}
        {activeTab === "training" && <TrainingTab gpuAvailable={gpuInfo?.available ?? false} />}
        {activeTab === "ruvector" && <RuVectorTab />}
        {activeTab === "metrics" && <MetricsTab />}
      </div>
    </div>
  );
};

export default Training;
