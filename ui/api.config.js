// API Configuration for WiFi-DensePose UI
// Auto-detects backend URL from current page location

const detectedHost = window.location.hostname;
const detectedPort = window.location.port || (window.location.protocol === 'https:' ? '443' : '80');
const baseUrl = `${window.location.protocol}//${detectedHost}:${detectedPort}`;
const wsPort = detectedPort === '3000' ? '3001' : detectedPort;

export const API_CONFIG = {
  BASE_URL: baseUrl,
  WS_URL: `ws://${detectedHost}:${wsPort}`,
  DEFAULT_HEADERS: {
    'Content-Type': 'application/json',
  },
};

export function buildApiUrl(endpoint, params = {}) {
  const url = new URL(endpoint, API_CONFIG.BASE_URL);
  Object.entries(params).forEach(([k, v]) => {
    if (v !== undefined && v !== null) url.searchParams.set(k, v);
  });
  return url.toString();
}
