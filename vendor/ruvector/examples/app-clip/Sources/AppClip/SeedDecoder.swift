// SeedDecoder.swift — Swift wrapper for decoding RVQS QR Cognitive Seeds.
//
// Calls into the RVF C FFI (librvf_runtime.a) via the RVFBridge module
// to parse raw seed bytes scanned from a QR code.

import Foundation
import RVFBridge

// MARK: - SeedInfo

/// Decoded information from an RVQS QR Cognitive Seed.
struct SeedInfo: Codable, Equatable, Sendable {
    /// Seed format version.
    let version: UInt16
    /// Number of download hosts in the manifest.
    let hosts: UInt32
    /// Number of progressive download layers.
    let layers: UInt32
    /// SHAKE-256-64 content hash as a hex string.
    let contentHash: String
    /// Total vector count the seed references.
    let totalVectorCount: UInt32
    /// Vector dimensionality.
    let dimension: UInt16
    /// Total seed payload size in bytes.
    let totalSeedSize: UInt32
    /// Whether the seed has an embedded WASM microkernel.
    let hasMicrokernel: Bool
    /// Whether the seed is cryptographically signed.
    let isSigned: Bool
    /// Primary download URL (if available).
    let primaryHostURL: String?
}

// MARK: - SeedDecoderError

/// Errors that can occur during seed decoding.
enum SeedDecoderError: LocalizedError {
    case emptyData
    case parseFailed(code: Int32)
    case urlExtractionFailed(code: Int32)

    var errorDescription: String? {
        switch self {
        case .emptyData:
            return "Seed data is empty."
        case .parseFailed(let code):
            return "Seed parse failed with error code \(code)."
        case .urlExtractionFailed(let code):
            return "Host URL extraction failed with error code \(code)."
        }
    }
}

// MARK: - SeedDecoder

/// Decodes RVQS QR Cognitive Seeds by calling the RVF C FFI.
///
/// Usage:
/// ```swift
/// let decoder = SeedDecoder()
/// let info = try decoder.decode(data: qrPayload)
/// print(info.contentHash)
/// ```
final class SeedDecoder: Sendable {

    init() {}

    /// Decode raw QR seed bytes into a `SeedInfo`.
    ///
    /// - Parameter data: The raw RVQS seed payload from a QR code.
    /// - Returns: Parsed seed information.
    /// - Throws: `SeedDecoderError` if parsing fails.
    func decode(data: Data) throws -> SeedInfo {
        guard !data.isEmpty else {
            throw SeedDecoderError.emptyData
        }

        // Parse the 64-byte header via the C FFI.
        var header = RvqsHeaderC()
        let parseResult: Int32 = data.withUnsafeBytes { rawBuffer in
            guard let baseAddress = rawBuffer.baseAddress else {
                return RVQS_ERR_NULL_PTR
            }
            let ptr = baseAddress.assumingMemoryBound(to: UInt8.self)
            return rvqs_parse_header(ptr, rawBuffer.count, &header)
        }

        guard parseResult == RVQS_OK else {
            throw SeedDecoderError.parseFailed(code: parseResult)
        }

        // Extract primary host URL (best-effort; nil if not available).
        let primaryURL = extractPrimaryHostURL(from: data)

        // Derive host_count and layer_count from the seed result.
        // The C FFI provides the header; we infer counts from manifest presence.
        // For a full implementation, rvf_seed_parse would walk the TLV manifest.
        // In this skeleton, we report manifest presence via flags.
        let hasManifest = (header.flags & 0x0002) != 0
        let hostCount: UInt32 = primaryURL != nil ? 1 : 0
        let layerCount: UInt32 = hasManifest ? 1 : 0

        // Build the hex string for content_hash.
        let hashBytes = withUnsafeBytes(of: header.content_hash) { Array($0) }
        let contentHash = hashBytes.map { String(format: "%02x", $0) }.joined()

        // Check flags.
        let hasMicrokernel = (header.flags & 0x0001) != 0
        let isSigned = (header.flags & 0x0004) != 0

        return SeedInfo(
            version: header.seed_version,
            hosts: hostCount,
            layers: layerCount,
            contentHash: contentHash,
            totalVectorCount: header.total_vector_count,
            dimension: header.dimension,
            totalSeedSize: header.total_seed_size,
            hasMicrokernel: hasMicrokernel,
            isSigned: isSigned,
            primaryHostURL: primaryURL
        )
    }

    /// Verify the content hash of a seed payload.
    ///
    /// - Parameter data: The raw RVQS seed payload.
    /// - Returns: `true` if the content hash is valid.
    func verifyContentHash(data: Data) -> Bool {
        let result: Int32 = data.withUnsafeBytes { rawBuffer in
            guard let baseAddress = rawBuffer.baseAddress else {
                return RVQS_ERR_NULL_PTR
            }
            let ptr = baseAddress.assumingMemoryBound(to: UInt8.self)
            return rvqs_verify_content_hash(ptr, rawBuffer.count)
        }
        return result == RVQS_OK
    }

    // MARK: - Private

    /// Extract the primary download host URL from the seed's TLV manifest.
    private func extractPrimaryHostURL(from data: Data) -> String? {
        var urlBuffer = [UInt8](repeating: 0, count: 256)
        var urlLength: Int = 0

        let result: Int32 = data.withUnsafeBytes { rawBuffer in
            guard let baseAddress = rawBuffer.baseAddress else {
                return RVQS_ERR_NULL_PTR
            }
            let ptr = baseAddress.assumingMemoryBound(to: UInt8.self)
            return rvqs_get_primary_host_url(
                ptr, rawBuffer.count,
                &urlBuffer, urlBuffer.count,
                &urlLength
            )
        }

        guard result == RVQS_OK, urlLength > 0 else {
            return nil
        }

        return String(bytes: urlBuffer[..<urlLength], encoding: .utf8)
    }
}
