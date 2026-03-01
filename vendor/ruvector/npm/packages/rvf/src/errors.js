"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.RvfError = exports.RvfErrorCode = void 0;
/**
 * Error codes mirroring the Rust `ErrorCode` enum (rvf-types).
 *
 * The high byte is the category, the low byte is the specific error.
 */
var RvfErrorCode;
(function (RvfErrorCode) {
    // Category 0x00: Success
    RvfErrorCode[RvfErrorCode["Ok"] = 0] = "Ok";
    RvfErrorCode[RvfErrorCode["OkPartial"] = 1] = "OkPartial";
    // Category 0x01: Format Errors
    RvfErrorCode[RvfErrorCode["InvalidMagic"] = 256] = "InvalidMagic";
    RvfErrorCode[RvfErrorCode["InvalidVersion"] = 257] = "InvalidVersion";
    RvfErrorCode[RvfErrorCode["InvalidChecksum"] = 258] = "InvalidChecksum";
    RvfErrorCode[RvfErrorCode["InvalidSignature"] = 259] = "InvalidSignature";
    RvfErrorCode[RvfErrorCode["TruncatedSegment"] = 260] = "TruncatedSegment";
    RvfErrorCode[RvfErrorCode["InvalidManifest"] = 261] = "InvalidManifest";
    RvfErrorCode[RvfErrorCode["ManifestNotFound"] = 262] = "ManifestNotFound";
    RvfErrorCode[RvfErrorCode["UnknownSegmentType"] = 263] = "UnknownSegmentType";
    RvfErrorCode[RvfErrorCode["AlignmentError"] = 264] = "AlignmentError";
    // Category 0x02: Query Errors
    RvfErrorCode[RvfErrorCode["DimensionMismatch"] = 512] = "DimensionMismatch";
    RvfErrorCode[RvfErrorCode["EmptyIndex"] = 513] = "EmptyIndex";
    RvfErrorCode[RvfErrorCode["MetricUnsupported"] = 514] = "MetricUnsupported";
    RvfErrorCode[RvfErrorCode["FilterParseError"] = 515] = "FilterParseError";
    RvfErrorCode[RvfErrorCode["KTooLarge"] = 516] = "KTooLarge";
    RvfErrorCode[RvfErrorCode["Timeout"] = 517] = "Timeout";
    // Category 0x03: Write Errors
    RvfErrorCode[RvfErrorCode["LockHeld"] = 768] = "LockHeld";
    RvfErrorCode[RvfErrorCode["LockStale"] = 769] = "LockStale";
    RvfErrorCode[RvfErrorCode["DiskFull"] = 770] = "DiskFull";
    RvfErrorCode[RvfErrorCode["FsyncFailed"] = 771] = "FsyncFailed";
    RvfErrorCode[RvfErrorCode["SegmentTooLarge"] = 772] = "SegmentTooLarge";
    RvfErrorCode[RvfErrorCode["ReadOnly"] = 773] = "ReadOnly";
    // Category 0x04: Tile Errors (WASM Microkernel)
    RvfErrorCode[RvfErrorCode["TileTrap"] = 1024] = "TileTrap";
    RvfErrorCode[RvfErrorCode["TileOom"] = 1025] = "TileOom";
    RvfErrorCode[RvfErrorCode["TileTimeout"] = 1026] = "TileTimeout";
    RvfErrorCode[RvfErrorCode["TileInvalidMsg"] = 1027] = "TileInvalidMsg";
    RvfErrorCode[RvfErrorCode["TileUnsupportedOp"] = 1028] = "TileUnsupportedOp";
    // Category 0x05: Crypto Errors
    RvfErrorCode[RvfErrorCode["KeyNotFound"] = 1280] = "KeyNotFound";
    RvfErrorCode[RvfErrorCode["KeyExpired"] = 1281] = "KeyExpired";
    RvfErrorCode[RvfErrorCode["DecryptFailed"] = 1282] = "DecryptFailed";
    RvfErrorCode[RvfErrorCode["AlgoUnsupported"] = 1283] = "AlgoUnsupported";
    // SDK-level errors (0xFF__)
    RvfErrorCode[RvfErrorCode["BackendNotFound"] = 65280] = "BackendNotFound";
    RvfErrorCode[RvfErrorCode["BackendInitFailed"] = 65281] = "BackendInitFailed";
    RvfErrorCode[RvfErrorCode["StoreClosed"] = 65282] = "StoreClosed";
})(RvfErrorCode || (exports.RvfErrorCode = RvfErrorCode = {}));
/** Human-readable labels for each error code. */
const ERROR_MESSAGES = {
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
class RvfError extends Error {
    /** Error category (high byte of the code). */
    get category() {
        return (this.code >> 8) & 0xff;
    }
    /** True when the category indicates a format-level (fatal) error. */
    get isFormatError() {
        return this.category === 0x01;
    }
    constructor(code, detail) {
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
    static fromNative(err) {
        if (err instanceof RvfError)
            return err;
        if (err instanceof Error) {
            const codeMatch = err.message.match(/0x([0-9a-fA-F]{4})/);
            if (codeMatch) {
                const code = parseInt(codeMatch[1], 16);
                if (code in RvfErrorCode) {
                    return new RvfError(code, err.message);
                }
            }
            return new RvfError(RvfErrorCode.BackendInitFailed, err.message);
        }
        return new RvfError(RvfErrorCode.BackendInitFailed, String(err));
    }
}
exports.RvfError = RvfError;
//# sourceMappingURL=errors.js.map