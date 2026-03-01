// =============================================================================
// HybridRecommendationService.swift
// Hybrid recommendation service combining WASM engine with remote fallback
// =============================================================================

import Foundation

// MARK: - Hybrid Recommendation Service

/// Actor-based service that combines local WASM recommendations with remote API fallback
public actor HybridRecommendationService {

    private let wasmEngine: WasmRecommendationEngine
    private let stateManager: WasmStateManager
    private let healthKitIntegration: HealthKitVibeProvider?
    private let remoteClient: RemoteRecommendationClient?

    // Configuration
    private let minLocalRecommendations: Int
    private let enableRemoteFallback: Bool

    // Statistics
    private var localHits: Int = 0
    private var remoteHits: Int = 0

    /// Initialize the hybrid recommendation service
    public init(
        wasmPath: URL,
        embeddingDim: Int = 64,
        numActions: Int = 100,
        enableHealthKit: Bool = false,
        enableRemote: Bool = false,
        remoteBaseURL: URL? = nil,
        minLocalRecommendations: Int = 10
    ) async throws {
        // Initialize WASM engine
        self.wasmEngine = try WasmRecommendationEngine(
            wasmPath: wasmPath,
            embeddingDim: embeddingDim,
            numActions: numActions
        )

        // Initialize state manager
        self.stateManager = WasmStateManager()

        // Load persisted state if available
        if let savedState = try? await stateManager.loadState() {
            try? wasmEngine.loadState(savedState)
        }

        // Optional HealthKit integration for vibe detection
        self.healthKitIntegration = enableHealthKit ? HealthKitVibeProvider() : nil

        // Optional remote client
        self.remoteClient = enableRemote && remoteBaseURL != nil
            ? RemoteRecommendationClient(baseURL: remoteBaseURL!)
            : nil

        self.minLocalRecommendations = minLocalRecommendations
        self.enableRemoteFallback = enableRemote
    }

    // MARK: - Recommendations

    /// Get personalized recommendations
    public func getRecommendations(
        candidates: [UInt64],
        topK: Int = 10
    ) async throws -> [ContentRecommendation] {
        // Get current vibe from HealthKit or use default
        let vibe = await getCurrentVibe()
        wasmEngine.setVibe(vibe)

        // Get local recommendations
        let localRecs = try await wasmEngine.recommend(candidates: candidates, topK: topK)
        localHits += 1

        // If we have enough local recommendations, return them
        if localRecs.count >= minLocalRecommendations || !enableRemoteFallback {
            return localRecs.map { rec in
                ContentRecommendation(
                    contentId: rec.contentId,
                    score: rec.score,
                    source: .local
                )
            }
        }

        // Fallback to remote for additional recommendations
        var results = localRecs.map { rec in
            ContentRecommendation(
                contentId: rec.contentId,
                score: rec.score,
                source: .local
            )
        }

        if let remote = remoteClient {
            let remainingCount = topK - localRecs.count
            if let remoteRecs = try? await remote.getRecommendations(
                vibe: vibe,
                count: remainingCount,
                exclude: Set(localRecs.map { $0.contentId })
            ) {
                results.append(contentsOf: remoteRecs.map { rec in
                    ContentRecommendation(
                        contentId: rec.contentId,
                        score: rec.score,
                        source: .remote
                    )
                })
                remoteHits += 1
            }
        }

        return results
    }

    /// Get similar content
    public func getSimilar(to contentId: UInt64, topK: Int = 5) async throws -> [ContentRecommendation] {
        // This would typically use the embedding similarity
        // For now, use the recommendation system with the content as "context"
        let candidates = try await generateCandidates(excluding: contentId)
        return try await getRecommendations(candidates: candidates, topK: topK)
    }

    // MARK: - Learning

    /// Record a user interaction
    public func recordInteraction(_ interaction: UserInteraction) async {
        do {
            try await wasmEngine.learn(interaction: interaction)

            // Periodically save state
            if wasmEngine.updateCount % 50 == 0 {
                await saveState()
            }
        } catch {
            print("Failed to record interaction: \(error)")
        }
    }

    /// Record multiple interactions in batch
    public func recordInteractions(_ interactions: [UserInteraction]) async {
        for interaction in interactions {
            await recordInteraction(interaction)
        }
    }

    // MARK: - State Management

    /// Save current engine state
    public func saveState() async {
        do {
            let state = try wasmEngine.saveState()
            try await stateManager.saveState(state)
        } catch {
            print("Failed to save state: \(error)")
        }
    }

    /// Clear all learned data
    public func clearLearning() async {
        do {
            try await stateManager.clearState()
            // Reinitialize engine would be needed here
        } catch {
            print("Failed to clear learning: \(error)")
        }
    }

    // MARK: - Statistics

    /// Get service statistics
    public func getStatistics() -> ServiceStatistics {
        ServiceStatistics(
            localHits: localHits,
            remoteHits: remoteHits,
            explorationRate: wasmEngine.explorationRate,
            totalUpdates: wasmEngine.updateCount
        )
    }

    // MARK: - Private Helpers

    private func getCurrentVibe() async -> VibeState {
        if let healthKit = healthKitIntegration {
            return await healthKit.getCurrentVibe()
        }
        return VibeState() // Default vibe
    }

    private func generateCandidates(excluding: UInt64) async throws -> [UInt64] {
        // In real implementation, this would query a content catalog
        // For now, return a sample set
        return (1...100).map { UInt64($0) }.filter { $0 != excluding }
    }
}

// MARK: - Supporting Types

/// Recommendation with source information
public struct ContentRecommendation {
    public let contentId: UInt64
    public let score: Float
    public let source: RecommendationSource

    public enum RecommendationSource {
        case local
        case remote
        case hybrid
    }
}

/// Service statistics
public struct ServiceStatistics {
    public let localHits: Int
    public let remoteHits: Int
    public let explorationRate: Float
    public let totalUpdates: UInt64

    public var localHitRate: Float {
        let total = localHits + remoteHits
        return total > 0 ? Float(localHits) / Float(total) : 0
    }
}

// MARK: - HealthKit Integration

/// Provides vibe state from HealthKit data
public actor HealthKitVibeProvider {

    public init() {
        // Request HealthKit permissions in real implementation
    }

    /// Get current vibe from HealthKit data
    public func getCurrentVibe() async -> VibeState {
        // In real implementation:
        // - Query HKHealthStore for heart rate, HRV, activity
        // - Compute energy level from activity data
        // - Estimate mood from HRV patterns
        // - Determine focus from recent activity

        // For now, return a simulated vibe based on time of day
        let hour = Calendar.current.component(.hour, from: Date())

        let energy: Float
        let focus: Float
        let timeContext = Float(hour) / 24.0

        switch hour {
        case 6..<9:   // Morning
            energy = 0.6
            focus = 0.7
        case 9..<12:  // Late morning
            energy = 0.8
            focus = 0.9
        case 12..<14: // Lunch
            energy = 0.5
            focus = 0.4
        case 14..<17: // Afternoon
            energy = 0.7
            focus = 0.8
        case 17..<20: // Evening
            energy = 0.6
            focus = 0.5
        case 20..<23: // Night
            energy = 0.4
            focus = 0.3
        default:      // Late night
            energy = 0.2
            focus = 0.2
        }

        return VibeState(
            energy: energy,
            mood: 0.5, // Neutral
            focus: focus,
            timeContext: timeContext
        )
    }
}

// MARK: - Remote Client

/// Client for remote recommendation API
public actor RemoteRecommendationClient {
    private let baseURL: URL
    private let session: URLSession

    public init(baseURL: URL) {
        self.baseURL = baseURL
        self.session = URLSession(configuration: .default)
    }

    /// Get recommendations from remote API
    public func getRecommendations(
        vibe: VibeState,
        count: Int,
        exclude: Set<UInt64>
    ) async throws -> [Recommendation] {
        // Build request
        var request = URLRequest(url: baseURL.appendingPathComponent("recommendations"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        let body: [String: Any] = [
            "vibe": [
                "energy": vibe.energy,
                "mood": vibe.mood,
                "focus": vibe.focus,
                "time_context": vibe.timeContext
            ],
            "count": count,
            "exclude": Array(exclude)
        ]

        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        // Make request
        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse,
              httpResponse.statusCode == 200 else {
            throw RemoteClientError.requestFailed
        }

        // Parse response
        guard let json = try JSONSerialization.jsonObject(with: data) as? [[String: Any]] else {
            throw RemoteClientError.invalidResponse
        }

        return json.compactMap { item in
            guard let id = item["id"] as? UInt64,
                  let score = item["score"] as? Float else {
                return nil
            }
            return Recommendation(contentId: id, score: score)
        }
    }
}

public enum RemoteClientError: Error {
    case requestFailed
    case invalidResponse
}
