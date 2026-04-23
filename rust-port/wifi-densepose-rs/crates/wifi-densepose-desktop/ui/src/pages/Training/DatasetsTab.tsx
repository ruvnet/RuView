import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Dataset {
  id: string;
  name: string;
  description: string;
  size_mb: number;
  samples: number;
  downloaded: boolean;
  path: string | null;
}

const STANDARD_DATASETS: Omit<Dataset, "downloaded" | "path">[] = [
  {
    id: "mmfi",
    name: "MM-Fi Dataset",
    description: "Multi-modal WiFi sensing dataset with 40 subjects, 27 activities",
    size_mb: 2400,
    samples: 320000,
  },
  {
    id: "wipose",
    name: "Wi-Pose Dataset",
    description: "WiFi-based pose estimation with 3D skeleton annotations",
    size_mb: 1800,
    samples: 150000,
  },
  {
    id: "wiar",
    name: "WiAR Dataset",
    description: "WiFi activity recognition with CSI data",
    size_mb: 500,
    samples: 45000,
  },
];

const DatasetsTab: React.FC = () => {
  const [datasets, setDatasets] = useState<Dataset[]>([]);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<number>(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadDatasets();
  }, []);

  const loadDatasets = async () => {
    try {
      const downloaded = await invoke<string[]>("list_datasets");
      const ds = STANDARD_DATASETS.map((d) => ({
        ...d,
        downloaded: downloaded.includes(d.id),
        path: downloaded.includes(d.id) ? `~/.ruview/datasets/${d.id}` : null,
      }));
      setDatasets(ds);
    } catch (err) {
      // If command not implemented yet, show placeholders
      setDatasets(
        STANDARD_DATASETS.map((d) => ({
          ...d,
          downloaded: false,
          path: null,
        }))
      );
    }
  };

  const handleDownload = async (datasetId: string) => {
    setDownloading(datasetId);
    setDownloadProgress(0);
    setError(null);

    try {
      // Simulate download progress for now
      for (let i = 0; i <= 100; i += 10) {
        setDownloadProgress(i);
        await new Promise((r) => setTimeout(r, 500));
      }

      // TODO: Call actual download command
      // await invoke("download_dataset", { datasetId });

      setDatasets((prev) =>
        prev.map((d) =>
          d.id === datasetId
            ? { ...d, downloaded: true, path: `~/.ruview/datasets/${d.id}` }
            : d
        )
      );
    } catch (err) {
      setError(`Download failed: ${err}`);
    } finally {
      setDownloading(null);
    }
  };

  return (
    <div>
      {/* Stats Row */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(3, 1fr)",
          gap: "var(--space-4)",
          marginBottom: "var(--space-5)",
        }}
      >
        <StatCard
          label="Available Datasets"
          value={datasets.length}
        />
        <StatCard
          label="Downloaded"
          value={datasets.filter((d) => d.downloaded).length}
          color="var(--status-online)"
        />
        <StatCard
          label="Total Samples"
          value={`${(datasets.reduce((acc, d) => acc + (d.downloaded ? d.samples : 0), 0) / 1000).toFixed(0)}K`}
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

      {/* Dataset Cards */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(340px, 1fr))",
          gap: "var(--space-4)",
        }}
      >
        {datasets.map((dataset) => (
          <div
            key={dataset.id}
            className="card"
            style={{
              padding: "var(--space-4)",
              opacity: dataset.downloaded ? 1 : 0.85,
            }}
          >
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "start",
                marginBottom: "var(--space-3)",
              }}
            >
              <div>
                <h3 style={{ margin: 0, fontSize: 15, fontWeight: 600 }}>
                  {dataset.name}
                </h3>
                <p
                  style={{
                    fontSize: 12,
                    color: "var(--text-muted)",
                    marginTop: 4,
                    lineHeight: 1.4,
                  }}
                >
                  {dataset.description}
                </p>
              </div>
              {dataset.downloaded && (
                <span
                  style={{
                    background: "rgba(63, 185, 80, 0.15)",
                    color: "var(--status-online)",
                    padding: "2px 8px",
                    borderRadius: 4,
                    fontSize: 10,
                    fontWeight: 600,
                  }}
                >
                  DOWNLOADED
                </span>
              )}
            </div>

            <div
              style={{
                display: "flex",
                gap: "var(--space-4)",
                fontSize: 12,
                color: "var(--text-secondary)",
                marginBottom: "var(--space-3)",
              }}
            >
              <span>📦 {(dataset.size_mb / 1024).toFixed(1)} GB</span>
              <span>📊 {(dataset.samples / 1000).toFixed(0)}K samples</span>
            </div>

            {downloading === dataset.id ? (
              <div>
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
                      width: `${downloadProgress}%`,
                      height: "100%",
                      background: "var(--accent)",
                      transition: "width 0.3s",
                    }}
                  />
                </div>
                <div
                  style={{
                    fontSize: 11,
                    color: "var(--text-muted)",
                    marginTop: 4,
                    textAlign: "center",
                  }}
                >
                  Downloading... {downloadProgress}%
                </div>
              </div>
            ) : (
              <div style={{ display: "flex", gap: "var(--space-2)" }}>
                {dataset.downloaded ? (
                  <>
                    <button
                      style={{
                        flex: 1,
                        padding: "8px 12px",
                        background: "rgba(56, 139, 253, 0.1)",
                        border: "1px solid rgba(56, 139, 253, 0.3)",
                        borderRadius: 6,
                        color: "var(--accent)",
                        fontSize: 12,
                        fontWeight: 600,
                        cursor: "pointer",
                      }}
                    >
                      Preview
                    </button>
                    <button
                      style={{
                        padding: "8px 12px",
                        background: "transparent",
                        border: "1px solid var(--border)",
                        borderRadius: 6,
                        color: "var(--text-secondary)",
                        fontSize: 12,
                        fontWeight: 600,
                        cursor: "pointer",
                      }}
                    >
                      Delete
                    </button>
                  </>
                ) : (
                  <button
                    onClick={() => handleDownload(dataset.id)}
                    className="btn-gradient"
                    style={{ flex: 1, fontSize: 12 }}
                  >
                    Download Dataset
                  </button>
                )}
              </div>
            )}
          </div>
        ))}
      </div>

      {/* Import Custom Dataset */}
      <div
        className="card"
        style={{
          marginTop: "var(--space-5)",
          padding: "var(--space-4)",
          border: "2px dashed var(--border)",
          textAlign: "center",
        }}
      >
        <div style={{ fontSize: 32, marginBottom: "var(--space-2)" }}>📁</div>
        <h4 style={{ margin: 0, fontSize: 14, fontWeight: 600 }}>
          Import Custom Dataset
        </h4>
        <p
          style={{
            fontSize: 12,
            color: "var(--text-muted)",
            marginTop: 4,
            marginBottom: "var(--space-3)",
          }}
        >
          Import CSI recordings in CSV, NPZ, or HDF5 format
        </p>
        <button
          style={{
            padding: "8px 20px",
            background: "transparent",
            border: "1px solid var(--border)",
            borderRadius: 6,
            color: "var(--text-secondary)",
            fontSize: 12,
            fontWeight: 600,
            cursor: "pointer",
          }}
        >
          Browse Files
        </button>
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

export default DatasetsTab;
