import type { HealthStatus } from "../types";

interface StatusBadgeProps {
  status: HealthStatus;
  size?: "sm" | "md" | "lg";
}

const STATUS_STYLES: Record<HealthStatus, { color: string; label: string }> = {
  online:   { color: "var(--status-online)",  label: "Online" },
  offline:  { color: "var(--status-error)",   label: "Offline" },
  degraded: { color: "var(--status-warning)", label: "Degraded" },
  unknown:  { color: "var(--text-muted)",     label: "Unknown" },
};

const SIZE_STYLES: Record<string, { fontSize: number; padding: string; dot: number }> = {
  sm: { fontSize: 11, padding: "2px 8px", dot: 6 },
  md: { fontSize: 13, padding: "4px 12px", dot: 8 },
  lg: { fontSize: 15, padding: "6px 16px", dot: 10 },
};

export function StatusBadge({ status, size = "sm" }: StatusBadgeProps) {
  const { color, label } = STATUS_STYLES[status];
  const s = SIZE_STYLES[size];

  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        color,
        fontSize: s.fontSize,
        fontWeight: 600,
        fontFamily: "var(--font-sans)",
        padding: s.padding,
        borderRadius: 9999,
        lineHeight: 1,
        whiteSpace: "nowrap",
        background: "rgba(255, 255, 255, 0.04)",
      }}
    >
      <span
        style={{
          width: s.dot,
          height: s.dot,
          borderRadius: "50%",
          backgroundColor: color,
          flexShrink: 0,
          boxShadow: status === "online"
            ? `0 0 4px ${color}, 0 0 8px ${color}`
            : status === "degraded"
              ? `0 0 4px ${color}`
              : "none",
        }}
      />
      {label}
    </span>
  );
}
