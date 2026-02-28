/**
 * Error codes mirroring the Rust `ErrorCode` enum (rvf-types).
 *
 * The high byte is the category, the low byte is the specific error.
 */
export declare enum RvfErrorCode {
    Ok = 0,
    OkPartial = 1,
    InvalidMagic = 256,
    InvalidVersion = 257,
    InvalidChecksum = 258,
    InvalidSignature = 259,
    TruncatedSegment = 260,
    InvalidManifest = 261,
    ManifestNotFound = 262,
    UnknownSegmentType = 263,
    AlignmentError = 264,
    DimensionMismatch = 512,
    EmptyIndex = 513,
    MetricUnsupported = 514,
    FilterParseError = 515,
    KTooLarge = 516,
    Timeout = 517,
    LockHeld = 768,
    LockStale = 769,
    DiskFull = 770,
    FsyncFailed = 771,
    SegmentTooLarge = 772,
    ReadOnly = 773,
    TileTrap = 1024,
    TileOom = 1025,
    TileTimeout = 1026,
    TileInvalidMsg = 1027,
    TileUnsupportedOp = 1028,
    KeyNotFound = 1280,
    KeyExpired = 1281,
    DecryptFailed = 1282,
    AlgoUnsupported = 1283,
    BackendNotFound = 65280,
    BackendInitFailed = 65281,
    StoreClosed = 65282
}
/**
 * Custom error class for all RVF operations.
 *
 * Carries a typed `code` field for programmatic matching and a
 * human-readable `message`.
 */
export declare class RvfError extends Error {
    /** The RVF error code. */
    readonly code: RvfErrorCode;
    /** Error category (high byte of the code). */
    get category(): number;
    /** True when the category indicates a format-level (fatal) error. */
    get isFormatError(): boolean;
    constructor(code: RvfErrorCode, detail?: string);
    /**
     * Create an RvfError from a native binding error.
     * Attempts to extract an error code from the message or object.
     */
    static fromNative(err: unknown): RvfError;
}
//# sourceMappingURL=errors.d.ts.map