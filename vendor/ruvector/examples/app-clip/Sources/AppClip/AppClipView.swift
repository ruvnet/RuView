// AppClipView.swift — SwiftUI view for the RVF App Clip.
//
// Presents a QR scanner interface and displays decoded seed information.
// Uses AVFoundation for camera access and the SeedDecoder for parsing.

import SwiftUI
import AVFoundation

// MARK: - AppClipView

/// Root view for the App Clip experience.
///
/// Flow: Scan QR -> Decode RVQS seed -> Display cognitive seed info.
struct AppClipView: View {
    @StateObject private var viewModel = AppClipViewModel()

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                switch viewModel.state {
                case .scanning:
                    scannerSection
                case .decoding:
                    decodingSection
                case .decoded(let info):
                    decodedSection(info)
                case .error(let message):
                    errorSection(message)
                }
            }
            .navigationTitle("RVF Seed Scanner")
            .navigationBarTitleDisplayMode(.inline)
        }
    }

    // MARK: - Scanner

    private var scannerSection: some View {
        VStack(spacing: 24) {
            Spacer()

            // Camera preview placeholder.
            // In production, this would be a UIViewRepresentable wrapping
            // an AVCaptureVideoPreviewLayer for real-time QR scanning.
            ZStack {
                RoundedRectangle(cornerRadius: 16)
                    .fill(Color.black.opacity(0.8))
                    .frame(width: 280, height: 280)

                RoundedRectangle(cornerRadius: 12)
                    .strokeBorder(Color.white.opacity(0.6), lineWidth: 2)
                    .frame(width: 240, height: 240)

                VStack(spacing: 12) {
                    Image(systemName: "qrcode.viewfinder")
                        .font(.system(size: 48))
                        .foregroundStyle(.white)
                    Text("Point camera at QR seed")
                        .font(.subheadline)
                        .foregroundStyle(.white.opacity(0.8))
                }
            }

            Text("Scan a cognitive seed QR code to bootstrap intelligence.")
                .font(.footnote)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            // Demo button for testing without a camera.
            Button {
                viewModel.decodeDemoSeed()
            } label: {
                Label("Use Demo Seed", systemImage: "doc.viewfinder")
                    .font(.body.weight(.medium))
            }
            .buttonStyle(.borderedProminent)
            .tint(.blue)

            Spacer()
        }
    }

    // MARK: - Decoding

    private var decodingSection: some View {
        VStack(spacing: 16) {
            Spacer()
            ProgressView()
                .scaleEffect(1.5)
            Text("Decoding seed...")
                .font(.headline)
                .foregroundStyle(.secondary)
            Spacer()
        }
    }

    // MARK: - Decoded Result

    private func decodedSection(_ info: SeedInfo) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                // Header card.
                GroupBox {
                    VStack(alignment: .leading, spacing: 8) {
                        Label("Cognitive Seed", systemImage: "brain")
                            .font(.headline)

                        Divider()

                        infoRow("Version", value: "v\(info.version)")
                        infoRow("Dimension", value: "\(info.dimension)")
                        infoRow("Vectors", value: formatCount(info.totalVectorCount))
                        infoRow("Seed Size", value: formatBytes(info.totalSeedSize))
                    }
                }

                // Content hash.
                GroupBox {
                    VStack(alignment: .leading, spacing: 8) {
                        Label("Content Hash", systemImage: "number")
                            .font(.headline)

                        Divider()

                        Text(info.contentHash)
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(.secondary)
                            .textSelection(.enabled)
                    }
                }

                // Manifest info.
                GroupBox {
                    VStack(alignment: .leading, spacing: 8) {
                        Label("Manifest", systemImage: "arrow.down.circle")
                            .font(.headline)

                        Divider()

                        infoRow("Hosts", value: "\(info.hosts)")
                        infoRow("Layers", value: "\(info.layers)")
                        infoRow("Microkernel", value: info.hasMicrokernel ? "Yes" : "No")
                        infoRow("Signed", value: info.isSigned ? "Yes" : "No")

                        if let url = info.primaryHostURL {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Primary Host")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                Text(url)
                                    .font(.system(.caption2, design: .monospaced))
                                    .foregroundStyle(.blue)
                                    .textSelection(.enabled)
                            }
                        }
                    }
                }

                // Action buttons.
                Button {
                    viewModel.reset()
                } label: {
                    Label("Scan Another", systemImage: "qrcode.viewfinder")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
            }
            .padding()
        }
    }

    // MARK: - Error

    private func errorSection(_ message: String) -> some View {
        VStack(spacing: 24) {
            Spacer()

            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 48))
                .foregroundStyle(.red)

            Text("Decode Failed")
                .font(.title2.weight(.semibold))

            Text(message)
                .font(.body)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            Button {
                viewModel.reset()
            } label: {
                Label("Try Again", systemImage: "arrow.counterclockwise")
                    .font(.body.weight(.medium))
            }
            .buttonStyle(.borderedProminent)

            Spacer()
        }
    }

    // MARK: - Helpers

    private func infoRow(_ label: String, value: String) -> some View {
        HStack {
            Text(label)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .font(.subheadline.weight(.medium))
        }
    }

    private func formatCount(_ count: UInt32) -> String {
        if count >= 1_000_000 {
            return String(format: "%.1fM", Double(count) / 1_000_000.0)
        } else if count >= 1_000 {
            return String(format: "%.1fK", Double(count) / 1_000.0)
        }
        return "\(count)"
    }

    private func formatBytes(_ bytes: UInt32) -> String {
        if bytes >= 1024 {
            return String(format: "%.1f KB", Double(bytes) / 1024.0)
        }
        return "\(bytes) B"
    }
}

// MARK: - ViewModel

/// View model driving the App Clip scan-decode flow.
@MainActor
final class AppClipViewModel: ObservableObject {

    enum State: Equatable {
        case scanning
        case decoding
        case decoded(SeedInfo)
        case error(String)
    }

    @Published var state: State = .scanning

    private let decoder = SeedDecoder()

    /// Handle raw QR payload bytes from the camera scanner.
    func handleScannedData(_ data: Data) {
        state = .decoding
        Task {
            do {
                let info = try decoder.decode(data: data)
                state = .decoded(info)
            } catch {
                state = .error(error.localizedDescription)
            }
        }
    }

    /// Decode a built-in demo seed for testing without a camera.
    func decodeDemoSeed() {
        // Construct a minimal valid RVQS header (64 bytes) for demonstration.
        // In production, this would come from a real QR scan.
        var payload = Data(count: 64)
        payload.withUnsafeMutableBytes { buf in
            let p = buf.baseAddress!.assumingMemoryBound(to: UInt8.self)

            // seed_magic = 0x52565153 ("RVQS") in little-endian.
            p[0] = 0x53; p[1] = 0x51; p[2] = 0x56; p[3] = 0x52
            // seed_version = 1.
            p[4] = 0x01; p[5] = 0x00
            // flags = 0 (minimal).
            p[6] = 0x00; p[7] = 0x00
            // file_id = 8 bytes.
            for i in 8..<16 { p[i] = UInt8(i) }
            // total_vector_count = 1000 (little-endian).
            p[0x10] = 0xE8; p[0x11] = 0x03; p[0x12] = 0x00; p[0x13] = 0x00
            // dimension = 128.
            p[0x14] = 0x80; p[0x15] = 0x00
            // base_dtype = 0, profile_id = 0.
            p[0x16] = 0x00; p[0x17] = 0x00
            // created_ns = 0 (8 bytes, already zero).
            // microkernel_offset = 64, microkernel_size = 0.
            p[0x20] = 0x40; p[0x21] = 0x00; p[0x22] = 0x00; p[0x23] = 0x00
            // download_manifest_offset = 64, download_manifest_size = 0.
            p[0x28] = 0x40; p[0x29] = 0x00; p[0x2A] = 0x00; p[0x2B] = 0x00
            // sig_algo = 0, sig_length = 0.
            // total_seed_size = 64.
            p[0x34] = 0x40; p[0x35] = 0x00; p[0x36] = 0x00; p[0x37] = 0x00
            // content_hash = 8 bytes of 0xAB.
            for i in 0x38..<0x40 { p[i] = 0xAB }
        }

        handleScannedData(payload)
    }

    /// Reset to scanning state.
    func reset() {
        state = .scanning
    }
}

// MARK: - QR Scanner Coordinator (AVFoundation placeholder)

/// Coordinator for camera-based QR code scanning.
///
/// In a full implementation, this would wrap AVCaptureSession with a
/// metadata output delegate to detect QR codes in real-time.
/// Kept as a placeholder to show the integration pattern.
#if canImport(AVFoundation)
final class QRScannerCoordinator: NSObject, AVCaptureMetadataOutputObjectsDelegate {

    var onSeedScanned: ((Data) -> Void)?

    func metadataOutput(
        _ output: AVCaptureMetadataOutput,
        didOutput metadataObjects: [AVMetadataObject],
        from connection: AVCaptureConnection
    ) {
        guard let readable = metadataObjects.first as? AVMetadataMachineReadableCodeObject,
              readable.type == .qr,
              let stringValue = readable.stringValue,
              let data = stringValue.data(using: .utf8)
        else {
            return
        }

        // In production, the QR code would contain raw binary data.
        // For App Clips invoked via URL, the seed bytes would be
        // fetched from the URL's associated payload.
        onSeedScanned?(data)
    }
}
#endif
