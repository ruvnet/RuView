/**
 * Error codes mirroring the Rust `ErrorCode` enum (rvf-types).
 *
 * The high byte is the category, the low byte is the specific error.
 */
export enum RvfErrorCode {
  // Category 0x00: Success
  Ok = 0x0000,
  OkPartial = 0x0001,

  // Category 0x01: Format Errors
  InvalidMagic = 0x0100,
  InvalidVersion = 0x0101,
  InvalidChecksum = 0x0102,
  InvalidSignature = 0x0103,
  TruncatedSegment = 0x0104,
  InvalidManifest = 0x0105,
  ManifestNotFound = 0x0106,
  UnknownSegmentType = 0x0107,
  AlignmentError = 0x0108,

  // Category 0x02: Query Errors
  DimensionMismatch = 0x0200,
  EmptyIndex = 0x0201,
  MetricUnsupported = 0x0202,
  FilterParseError = 0x0203,
  KTooLarge = 0x0204,
  Timeout = 0x0205,

  // Category 0x03: Write Errors
  LockHeld = 0x0300,
  LockStale = 0x0301,
  DiskFull = 0x0302,
  FsyncFailed = 0x0303,
  SegmentTooLarge = 0x0304,
  ReadOnly = 0x0305,

  // Category 0x04: Tile Errors (WASM Microkernel)
  TileTrap = 0x0400,
  TileOom = 0x0401,
  TileTimeout = 0x0402,
  TileInvalidMsg = 0x0403,
  TileUnsupportedOp = 0x0404,

  // Category 0x05: Crypto Errors
  KeyNotFound = 0x0500,
  KeyExpired = 0x0501,
  DecryptFailed = 0x0502,
  AlgoUnsupported = 0x0503,

  // SDK-level errors (0xFF__)
  BackendNotFound = 0xff00,
  BackendInitFailed = 0xff01,
  StoreClosed = 0xff02,
}

/** Human-readable labels for each error code. */
const ERROR_MESSAGES: Record<number, string> = {
  [RvfErrorCode.Ok]: 'Operation succeeded',
  [RvfErrorCode.OkPartial]: 'Partial success (some items failed)',
  [RvfErrorCode.InvalidMagic]: 'Segment magic mismatch',
  [RvfErrorCode.InvalidVersion]: 'Unsupported segment version',
  [RvfErrorCode.InvalidChecksum]: 'Segment hash verification failed',
  [RvfErrorCode.InvalidSignature]: 'Cryptographic signature invalid',
  [RvfErrorCode.TruncatedSegment]: 'Segment payload shorter than declared',
  [RvfErrorCode.InvalidManifest]: 'Root manifest validation failed',
  [RvfErrorCode.ManifestNotFound]: 'No valid manifest in file',
  [RvfErrorCode.UnknownSegmentType]: 'Unrecognized segment type',
  [RvfErrorCode.AlignmentError]: 'Data not at expected 64-byte boundary',
  [RvfErrorCode.DimensionMismatch]: 'Query vector dimension != index dimension',
  [RvfErrorCode.EmptyIndex]: 'No index segments available',
  [RvfErrorCode.MetricUnsupported]: 'Requested distance metric not available',
  [RvfErrorCode.FilterParseError]: 'Invalid filter expression',
  [RvfErrorCode.KTooLarge]: 'Requested K exceeds available vectors',
  [RvfErrorCode.Timeout]: 'Query exceeded time budget',
  [RvfErrorCode.LockHeld]: 'Another writer holds the lock',
  [RvfErrorCode.LockStale]: 'Lock file exists but owner is dead',
  [RvfErrorCode.DiskFull]: 'Insufficient space for write',
  [RvfErrorCode.FsyncFailed]: 'Durable write (fsync) failed',
  [RvfErrorCode.SegmentTooLarge]: 'Segment exceeds 4 GB limit',
  [RvfErrorCode.ReadOnly]: 'Store opened in read-only mode',
  [RvfErrorCode.TileTrap]: 'WASM trap (OOB, unreachable, stack overflow)',
  [RvfErrorCode.TileOom]: 'Tile exceeded scratch memory',
  [RvfErrorCode.TileTimeout]: 'Tile computation exceeded time budget',
  [RvfErrorCode.TileInvalidMsg]: 'Malformed hub-tile message',
  [RvfErrorCode.TileUnsupportedOp]: 'Operation not available on this profile',
  [RvfErrorCode.KeyNotFound]: 'Referenced key_id not found',
  [RvfErrorCode.KeyExpired]: 'Key past valid_until timestamp',
  [RvfErrorCode.DecryptFailed]: 'Decryption or auth tag verification failed',
  [RvfErrorCode.AlgoUnsupported]: 'Cryptographic algorithm not implemented',
  [RvfErrorCode.BackendNotFound]: 'No suitable backend found (install @ruvector/rvf-node or @ruvector/rvf-wasm)',
  [RvfErrorCode.BackendInitFailed]: 'Backend initialization failed',
  [RvfErrorCode.StoreClosed]: 'Store has been closed',
};

/**
 * Custom error class for all RVF operations.
 *
 * Carries a typed `code` field for programmatic matching and a
 * human-readable `message`.
 */
export class RvfError extends Error {
  /** The RVF error code. */
  readonly code: RvfErrorCode;

  /** Error category (high byte of the code). */
  get category(): number {
    return (this.code >> 8) & 0xff;
  }

  /** True when the category indicates a format-level (fatal) error. */
  get isFormatError(): boolean {
    return this.category === 0x01;
  }

  constructor(code: RvfErrorCode, detail?: string) {
    const base = ERROR_MESSAGES[code] ?? `RVF error 0x${code.toString(16).padStart(4, '0')}`;
    const message = detail ? `${base}: ${detail}` : base;
    super(message);
    this.name = 'RvfError';
    this.code = code;
  }

  /**
   * Create an RvfError from a native binding error.
   * Attempts to extract an error code from the message or object.
   */
  static fromNative(err: unknown): RvfError {
    if (err instanceof RvfError) return err;
    if (err instanceof Error) {
      const codeMatch = err.message.match(/0x([0-9a-fA-F]{4})/);
      if (codeMatch) {
        const code = parseInt(codeMatch[1], 16);
        if (code in RvfErrorCode) {
          return new RvfError(code as RvfErrorCode, err.message);
        }
      }
      return new RvfError(RvfErrorCode.BackendInitFailed, err.message);
    }
    return new RvfError(RvfErrorCode.BackendInitFailed, String(err));
  }
}
