import { validateServerUrl } from '@/utils/urlValidator';

describe('validateServerUrl', () => {
  it('accepts valid http URL', () => {
    const result = validateServerUrl('http://localhost:3000');
    expect(result.valid).toBe(true);
    expect(result.error).toBeUndefined();
  });

  it('accepts valid https URL', () => {
    const result = validateServerUrl('https://example.com');
    expect(result.valid).toBe(true);
  });

  it('accepts valid ws URL', () => {
    const result = validateServerUrl('ws://192.168.1.1:8080');
    expect(result.valid).toBe(true);
  });

  it('accepts valid wss URL', () => {
    const result = validateServerUrl('wss://example.com/ws');
    expect(result.valid).toBe(true);
  });

  it('rejects empty string', () => {
    const result = validateServerUrl('');
    expect(result.valid).toBe(false);
    expect(result.error).toBe('URL must be a non-empty string.');
  });

  it('rejects whitespace-only string', () => {
    const result = validateServerUrl('   ');
    expect(result.valid).toBe(false);
    expect(result.error).toBe('URL must be a non-empty string.');
  });

  it('rejects null input', () => {
    const result = validateServerUrl(null as unknown as string);
    expect(result.valid).toBe(false);
    expect(result.error).toBe('URL must be a non-empty string.');
  });

  it('rejects undefined input', () => {
    const result = validateServerUrl(undefined as unknown as string);
    expect(result.valid).toBe(false);
    expect(result.error).toBe('URL must be a non-empty string.');
  });

  it('rejects numeric input', () => {
    const result = validateServerUrl(123 as unknown as string);
    expect(result.valid).toBe(false);
    expect(result.error).toBe('URL must be a non-empty string.');
  });

  it('rejects ftp protocol', () => {
    const result = validateServerUrl('ftp://files.example.com');
    expect(result.valid).toBe(false);
    expect(result.error).toBe('URL must use http, https, ws, or wss.');
  });

  it('rejects file protocol', () => {
    const result = validateServerUrl('file:///etc/passwd');
    expect(result.valid).toBe(false);
  });

  it('rejects malformed URL', () => {
    const result = validateServerUrl('not-a-url');
    expect(result.valid).toBe(false);
    expect(result.error).toBe('Invalid URL format.');
  });

  it('rejects URL with no host', () => {
    const result = validateServerUrl('http://');
    expect(result.valid).toBe(false);
  });
});
