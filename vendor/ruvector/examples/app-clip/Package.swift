// swift-tools-version: 5.9
// Package.swift — SPM manifest for the RVF App Clip skeleton.
//
// This package links the pre-built RVF static library (librvf_runtime.a)
// produced by:
//   cargo build --release --target aarch64-apple-ios --lib
//
// Place the compiled .a file under lib/ before building with Xcode.

import PackageDescription

let package = Package(
    name: "RVFAppClip",
    platforms: [
        .iOS(.v16),
    ],
    products: [
        .library(
            name: "AppClip",
            targets: ["AppClip"]
        ),
    ],
    targets: [
        // C bridge module that exposes the RVF FFI header to Swift.
        .target(
            name: "RVFBridge",
            path: "Sources/RVFBridge",
            publicHeadersPath: ".",
            linkerSettings: [
                // Link the pre-built Rust static library.
                .unsafeFlags(["-L../../target/aarch64-apple-ios/release"]),
                .linkedLibrary("rvf_runtime"),
            ]
        ),
        // Swift App Clip target that consumes the C bridge.
        .target(
            name: "AppClip",
            dependencies: ["RVFBridge"],
            path: "Sources/AppClip"
        ),
    ]
)
