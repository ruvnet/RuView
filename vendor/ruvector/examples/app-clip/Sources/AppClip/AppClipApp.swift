// AppClipApp.swift — Entry point for the RVF App Clip.
//
// This is a minimal SwiftUI App Clip that scans QR cognitive seeds
// and decodes them using the RVF C FFI. Designed to stay under the
// 15 MB App Clip size limit per Apple guidelines.
//
// App Clip invocation URL scheme:
//   https://rvf.example.com/seed?id=<file_id>
//
// The App Clip can be invoked by:
//   1. Scanning an RVQS QR code directly (camera flow)
//   2. Tapping an App Clip Code / NFC tag
//   3. Opening a Smart App Banner link

import SwiftUI

@main
struct AppClipApp: App {
    @StateObject private var appState = AppClipState()

    var body: some Scene {
        WindowGroup {
            AppClipView()
                .onContinueUserActivity(
                    NSUserActivityTypeBrowsingWeb,
                    perform: handleUserActivity
                )
                .environmentObject(appState)
        }
    }

    /// Handle App Clip invocation via URL.
    ///
    /// When the App Clip is launched from a Smart App Banner or App Clip Code,
    /// iOS delivers the invocation URL as a user activity. We extract the
    /// seed identifier and trigger a download + decode flow.
    private func handleUserActivity(_ activity: NSUserActivity) {
        guard let url = activity.webpageURL else { return }
        appState.handleInvocationURL(url)
    }
}

// MARK: - AppClipState

/// Shared state for App Clip lifecycle and invocation handling.
@MainActor
final class AppClipState: ObservableObject {

    /// The invocation URL that launched this App Clip (if any).
    @Published var invocationURL: URL?

    /// Handle an App Clip invocation URL.
    ///
    /// Extracts the seed ID from the URL query parameters and could
    /// trigger a network fetch for the seed payload.
    func handleInvocationURL(_ url: URL) {
        invocationURL = url

        // Extract seed ID from query parameters.
        // Example: https://rvf.example.com/seed?id=0102030405060708
        guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
              let seedIDParam = components.queryItems?.first(where: { $0.name == "id" }),
              let _ = seedIDParam.value
        else {
            return
        }

        // In production:
        // 1. Fetch the seed payload from the CDN using the seed ID.
        // 2. Pass the raw bytes to SeedDecoder.decode(data:).
        // 3. Begin progressive download of the full RVF file.
    }
}
