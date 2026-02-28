// swift-tools-version:5.9
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "RuvectorRecommendation",
    platforms: [
        .iOS(.v16),
        .macOS(.v13)
    ],
    products: [
        .library(
            name: "RuvectorRecommendation",
            targets: ["RuvectorRecommendation"]),
    ],
    dependencies: [
        // WasmKit for WASM runtime
        .package(url: "https://github.com/swiftwasm/WasmKit.git", from: "0.1.0"),
    ],
    targets: [
        .target(
            name: "RuvectorRecommendation",
            dependencies: [
                .product(name: "WasmKit", package: "WasmKit"),
            ],
            path: ".",
            exclude: ["Package.swift", "Resources"],
            sources: ["WasmRecommendationEngine.swift", "HybridRecommendationService.swift"],
            resources: [
                .copy("Resources/recommendation.wasm")
            ]
        ),
        .testTarget(
            name: "RuvectorRecommendationTests",
            dependencies: ["RuvectorRecommendation"],
            path: "Tests"
        ),
    ]
)
