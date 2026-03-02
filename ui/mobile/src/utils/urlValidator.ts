const ALLOWED_PROTOCOLS = new Set(['http:', 'https:', 'ws:', 'wss:']);

export interface UrlValidationResult {
  valid: boolean;
  error?: string;
}

export function validateServerUrl(url: string): UrlValidationResult {
  if (typeof url !== 'string' || !url.trim()) {
    return { valid: false, error: 'URL must be a non-empty string.' };
  }

  try {
    const parsed = new URL(url);
    if (!ALLOWED_PROTOCOLS.has(parsed.protocol)) {
      return { valid: false, error: 'URL must use http, https, ws, or wss.' };
    }
    if (!parsed.host) {
      return { valid: false, error: 'URL must include a host.' };
    }
    return { valid: true };
  } catch {
    return { valid: false, error: 'Invalid URL format.' };
  }
}
