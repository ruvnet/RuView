export function formatRssi(v: number | null | undefined): string {
  if (typeof v !== 'number' || !Number.isFinite(v)) {
    return '-- dBm';
  }
  return `${Math.round(v)} dBm`;
}

export function formatBpm(v: number | null | undefined): string {
  if (typeof v !== 'number' || !Number.isFinite(v)) {
    return '--';
  }
  return `${Math.round(v)} BPM`;
}

export function formatConfidence(v: number | null | undefined): string {
  if (typeof v !== 'number' || !Number.isFinite(v)) {
    return '--';
  }
  const normalized = v > 1 ? v / 100 : v;
  return `${Math.round(Math.max(0, Math.min(1, normalized)) * 100)}%`;
}

export function formatUptime(ms: number | null | undefined): string {
  if (typeof ms !== 'number' || !Number.isFinite(ms) || ms < 0) {
    return '--:--:--';
  }

  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
}
