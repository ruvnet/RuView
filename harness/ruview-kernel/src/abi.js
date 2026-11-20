// SPDX-License-Identifier: MIT
// Shared JSON-ABI envelope handling for every ruview-kernel transport (ADR-368).

export const ABI_VERSION = 1;
export const MAX_INPUT_BYTES = 16 * 1024 * 1024;
export const OPERATIONS = Object.freeze([
  'info', 'validate_config', 'analyze', 'synthesize',
  'session_open', 'session_push', 'session_summary', 'session_close',
  'analyze_flat', 'session_push_flat',
]);
const DEFAULT_SUBCARRIERS = 56;
const FRAME_KEYS = new Set(['amplitudes', 'phases']);

/** Error raised for a `{ok:false}` kernel envelope or a transport failure. */
export class KernelError extends Error {
  constructor(code, message, details = {}) {
    super(message);
    this.name = 'KernelError';
    this.code = code;
    Object.assign(this, details);
  }
}

/** Serialize a request, enforcing the ABI's operation allow-list and size bound. */
export function encodeRequest(op, request) {
  if (!OPERATIONS.includes(op)) throw new KernelError('unknown_operation', `unknown operation: ${String(op).slice(0, 64)}`);
  const json = typeof request === 'string' ? request : JSON.stringify(request ?? {});
  // UTF-8 needs at most 3 bytes per UTF-16 unit; only encode when it could matter.
  const bytes = json.length * 3 <= MAX_INPUT_BYTES ? json.length : new TextEncoder().encode(json).length;
  if (bytes > MAX_INPUT_BYTES) {
    throw new KernelError('limit_exceeded', `request exceeds ${MAX_INPUT_BYTES} bytes`);
  }
  return json;
}

/** Parse a kernel envelope; returns `result` or throws KernelError. */
export function decodeResponse(text) {
  let envelope;
  try {
    envelope = JSON.parse(text);
  } catch {
    throw new KernelError('internal', 'kernel returned malformed JSON');
  }
  if (envelope?.ok === true) return envelope.result;
  const error = envelope?.error || {};
  throw new KernelError(String(error.code || 'internal'), String(error.message || 'kernel error'));
}

/**
 * Pack uniform frames into one Float64Array: all amplitudes, then all phases.
 * Returns null whenever the JSON path must handle the input instead, so that
 * malformed frames still get the kernel's precise per-frame error messages.
 */
export function flattenFrames(frames, n) {
  if (!Array.isArray(frames) || !Number.isInteger(n) || n < 1) return null;
  const count = frames.length;
  const withPhases = count > 0 && Array.isArray(frames[0]?.phases) && frames[0].phases.length > 0;
  const data = new Float64Array(count * n * (withPhases ? 2 : 1));
  const phaseBase = count * n;
  for (let k = 0; k < count; k++) {
    const f = frames[k];
    if (!f || typeof f !== 'object' || !Array.isArray(f.amplitudes) || f.amplitudes.length !== n) return null;
    for (const key of Object.keys(f)) if (!FRAME_KEYS.has(key)) return null;
    const ph = f.phases ?? [];
    if (!Array.isArray(ph) || ph.length !== (withPhases ? n : 0)) return null;
    for (let i = 0; i < n; i++) {
      const a = f.amplitudes[i];
      if (typeof a !== 'number') return null;
      data[k * n + i] = a;
      if (withPhases) {
        const p = ph[i];
        if (typeof p !== 'number') return null;
        data[phaseBase + k * n + i] = p;
      }
    }
  }
  return { data, withPhases };
}

/**
 * Wrap a raw transport `(op, json) => json` into the high-level kernel API.
 * Both backends go through here, so their behaviour differs only in transport.
 * `transportF64(op, json, Float64Array)` is the optional binary fast path.
 */
export function createKernel(transport, meta, transportF64 = null) {
  const call = (op, request) => decodeResponse(transport(op, encodeRequest(op, request)));
  const callF64 = (op, request, data) => {
    if (!(data instanceof Float64Array)) throw new KernelError('invalid_request', 'binary frame data must be a Float64Array');
    if (data.byteLength > MAX_INPUT_BYTES) throw new KernelError('limit_exceeded', `binary data exceeds ${MAX_INPUT_BYTES} bytes`);
    return decodeResponse(transportF64(op, encodeRequest(op, request), data));
  };
  const flat = (frames, config) => (transportF64 ? flattenFrames(frames, config?.n_subcarriers ?? DEFAULT_SUBCARRIERS) : null);
  const kernel = {
    ...meta,
    binary: Boolean(transportF64),
    call,
    info: () => call('info', {}),
    validateConfig: (config = {}) => call('validate_config', config),
    analyze(frames, config = {}, { includeReadings = false } = {}) {
      const packed = flat(frames, config);
      return packed
        ? callF64('analyze_flat', { config, phases: packed.withPhases, include_readings: includeReadings }, packed.data)
        : call('analyze', { config, frames, include_readings: includeReadings });
    },
    /** Zero-copy entry: amplitudes for every frame, then phases if `phases`. */
    analyzeFlat(data, config = {}, { phases = false, includeReadings = false } = {}) {
      if (!transportF64) throw new KernelError('backend_unavailable', 'this kernel build has no binary transport');
      return callF64('analyze_flat', { config, phases, include_readings: includeReadings }, data);
    },
    synthesize: (options = {}) => call('synthesize', options),
    openSession(config = {}) {
      const n = config?.n_subcarriers ?? DEFAULT_SUBCARRIERS;
      const { session } = call('session_open', { config });
      let open = true;
      const ensureOpen = () => { if (!open) throw new KernelError('unknown_session', 'session is closed'); };
      return {
        id: session,
        push(frames) {
          ensureOpen();
          const packed = transportF64 ? flattenFrames(frames, n) : null;
          return packed
            ? callF64('session_push_flat', { session, phases: packed.withPhases }, packed.data).readings
            : call('session_push', { session, frames }).readings;
        },
        summary() { ensureOpen(); return call('session_summary', { session }).summary; },
        close() { if (!open) return false; open = false; return call('session_close', { session }).closed; },
      };
    },
  };
  return Object.freeze(kernel);
}
