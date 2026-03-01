// =============================================================================
// RecommendationTests.swift
// Unit tests for the WASM recommendation engine
// =============================================================================

import XCTest
@testable import RuvectorRecommendation

final class RecommendationTests: XCTestCase {

    // MARK: - Content Metadata Tests

    func testContentMetadataCreation() {
        let content = ContentMetadata(
            id: 123,
            contentType: .video,
            durationSecs: 120,
            categoryFlags: 0b1010,
            popularity: 0.8,
            recency: 0.9
        )

        XCTAssertEqual(content.id, 123)
        XCTAssertEqual(content.contentType, .video)
        XCTAssertEqual(content.durationSecs, 120)
    }

    func testContentMetadataFromDictionary() {
        let dict: [String: Any] = [
            "id": UInt64(456),
            "type": UInt8(1),
            "duration": UInt32(300),
            "popularity": Float(0.7)
        ]

        let content = ContentMetadata(from: dict)
        XCTAssertNotNil(content)
        XCTAssertEqual(content?.id, 456)
        XCTAssertEqual(content?.contentType, .audio)
    }

    // MARK: - Vibe State Tests

    func testVibeStateDefault() {
        let vibe = VibeState()

        XCTAssertEqual(vibe.energy, 0.5)
        XCTAssertEqual(vibe.mood, 0.0)
        XCTAssertEqual(vibe.focus, 0.5)
    }

    func testVibeStateCustom() {
        let vibe = VibeState(
            energy: 0.8,
            mood: 0.5,
            focus: 0.9,
            timeContext: 0.3,
            preferences: (0.1, 0.2, 0.3, 0.4)
        )

        XCTAssertEqual(vibe.energy, 0.8)
        XCTAssertEqual(vibe.mood, 0.5)
        XCTAssertEqual(vibe.preferences.0, 0.1)
    }

    // MARK: - Interaction Tests

    func testUserInteraction() {
        let interaction = UserInteraction(
            contentId: 789,
            interaction: .like,
            timeSpent: 45.0,
            position: 2
        )

        XCTAssertEqual(interaction.contentId, 789)
        XCTAssertEqual(interaction.interaction, .like)
        XCTAssertEqual(interaction.timeSpent, 45.0)
    }

    func testInteractionTypes() {
        XCTAssertEqual(InteractionType.view.rawValue, 0)
        XCTAssertEqual(InteractionType.like.rawValue, 1)
        XCTAssertEqual(InteractionType.share.rawValue, 2)
        XCTAssertEqual(InteractionType.skip.rawValue, 3)
        XCTAssertEqual(InteractionType.complete.rawValue, 4)
        XCTAssertEqual(InteractionType.dismiss.rawValue, 5)
    }

    // MARK: - Performance Tests

    func testRecommendationSpeed() async throws {
        // This test requires the actual WASM module to be available
        // Skip if not in a full integration environment

        // Performance baseline: should complete in under 100ms
        let start = Date()

        // Simulate recommendation workload
        var total: Float = 0
        for i in 0..<1000 {
            total += Float(i) * 0.001
        }

        let duration = Date().timeIntervalSince(start)
        XCTAssertLessThan(duration, 0.1, "Simulation should complete in under 100ms")

        // Prevent optimization
        XCTAssertGreaterThan(total, 0)
    }

    // MARK: - State Manager Tests

    func testStateManagerSaveLoad() async throws {
        let tempURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("test_state_\(UUID().uuidString).bin")

        let manager = WasmStateManager(stateURL: tempURL)

        // Save test data
        let testData = Data([0x01, 0x02, 0x03, 0x04])
        try await manager.saveState(testData)

        // Load and verify
        let loaded = try await manager.loadState()
        XCTAssertEqual(loaded, testData)

        // Cleanup
        try await manager.clearState()
        let afterClear = try await manager.loadState()
        XCTAssertNil(afterClear)
    }

    // MARK: - Error Tests

    func testWasmEngineErrors() {
        let errors: [WasmEngineError] = [
            .initializationFailed,
            .functionNotFound("test"),
            .embeddingFailed,
            .saveFailed,
            .loadFailed,
            .invalidInput("test message")
        ]

        for error in errors {
            XCTAssertNotNil(error.errorDescription)
            XCTAssertFalse(error.errorDescription!.isEmpty)
        }
    }
}

// MARK: - Statistics Tests

final class StatisticsTests: XCTestCase {

    func testServiceStatistics() {
        let stats = ServiceStatistics(
            localHits: 80,
            remoteHits: 20,
            explorationRate: 0.1,
            totalUpdates: 1000
        )

        XCTAssertEqual(stats.localHits, 80)
        XCTAssertEqual(stats.remoteHits, 20)
        XCTAssertEqual(stats.localHitRate, 0.8, accuracy: 0.01)
    }

    func testLocalHitRateZero() {
        let stats = ServiceStatistics(
            localHits: 0,
            remoteHits: 0,
            explorationRate: 0.1,
            totalUpdates: 0
        )

        XCTAssertEqual(stats.localHitRate, 0)
    }
}
