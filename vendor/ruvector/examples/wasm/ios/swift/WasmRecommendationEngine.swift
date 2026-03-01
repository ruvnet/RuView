// =============================================================================
// WasmRecommendationEngine.swift
// High-performance WASM-based recommendation engine for iOS
// Compatible with WasmKit runtime
// =============================================================================

import Foundation

// MARK: - Types

/// Content metadata for embedding generation
public struct ContentMetadata {
    public let id: UInt64
    public let contentType: ContentType
    public let durationSecs: UInt32
    public let categoryFlags: UInt32
    public let popularity: Float
    public let recency: Float

    public enum ContentType: UInt8 {
        case video = 0
        case audio = 1
        case image = 2
        case text = 3
    }

    public init(
        id: UInt64,
        contentType: ContentType,
        durationSecs: UInt32 = 0,
        categoryFlags: UInt32 = 0,
        popularity: Float = 0.5,
        recency: Float = 0.5
    ) {
        self.id = id
        self.contentType = contentType
        self.durationSecs = durationSecs
        self.categoryFlags = categoryFlags
        self.popularity = popularity
        self.recency = recency
    }
}

/// User vibe/preference state
public struct VibeState {
    public var energy: Float      // 0.0 = calm, 1.0 = energetic
    public var mood: Float        // -1.0 = negative, 1.0 = positive
    public var focus: Float       // 0.0 = relaxed, 1.0 = focused
    public var timeContext: Float // 0.0 = morning, 1.0 = night
    public var preferences: (Float, Float, Float, Float)

    public init(
        energy: Float = 0.5,
        mood: Float = 0.0,
        focus: Float = 0.5,
        timeContext: Float = 0.5,
        preferences: (Float, Float, Float, Float) = (0, 0, 0, 0)
    ) {
        self.energy = energy
        self.mood = mood
        self.focus = focus
        self.timeContext = timeContext
        self.preferences = preferences
    }
}

/// User interaction types
public enum InteractionType: UInt8 {
    case view = 0
    case like = 1
    case share = 2
    case skip = 3
    case complete = 4
    case dismiss = 5
}

/// User interaction event
public struct UserInteraction {
    public let contentId: UInt64
    public let interaction: InteractionType
    public let timeSpent: Float
    public let position: UInt8

    public init(
        contentId: UInt64,
        interaction: InteractionType,
        timeSpent: Float = 0,
        position: UInt8 = 0
    ) {
        self.contentId = contentId
        self.interaction = interaction
        self.timeSpent = timeSpent
        self.position = position
    }
}

/// Recommendation result
public struct Recommendation {
    public let contentId: UInt64
    public let score: Float
}

// MARK: - WasmRecommendationEngine

/// High-performance recommendation engine powered by WebAssembly
///
/// Usage:
/// ```swift
/// let engine = try WasmRecommendationEngine(wasmPath: Bundle.main.url(forResource: "recommendation", withExtension: "wasm")!)
/// engine.setVibe(VibeState(energy: 0.8, mood: 0.5))
/// let recs = try engine.recommend(candidates: [1, 2, 3, 4, 5], topK: 3)
/// ```
public class WasmRecommendationEngine {

    // MARK: - WASM Function References
    // These would be populated by WasmKit's module instantiation

    private let wasmModule: Any // WasmKit.Module
    private let wasmInstance: Any // WasmKit.Instance

    // Function pointers (simulated for demonstration)
    private var initFunc: ((UInt32, UInt32) -> Int32)?
    private var embedContentFunc: ((UInt64, UInt8, UInt32, UInt32, Float, Float) -> UnsafePointer<Float>?)?
    private var setVibeFunc: ((Float, Float, Float, Float, Float, Float, Float, Float) -> Void)?
    private var getRecommendationsFunc: ((UnsafePointer<UInt64>, UInt32, UInt32, UnsafeMutablePointer<UInt8>) -> UInt32)?
    private var updateLearningFunc: ((UInt64, UInt8, Float, UInt8) -> Void)?
    private var computeSimilarityFunc: ((UInt64, UInt64) -> Float)?
    private var saveStateFunc: (() -> UInt32)?
    private var loadStateFunc: ((UnsafePointer<UInt8>, UInt32) -> Int32)?
    private var getEmbeddingDimFunc: (() -> UInt32)?
    private var getExplorationRateFunc: (() -> Float)?
    private var getUpdateCountFunc: (() -> UInt64)?

    private let embeddingDim: Int
    private let numActions: Int

    // MARK: - Initialization

    /// Initialize the recommendation engine with a WASM module
    /// - Parameters:
    ///   - wasmPath: URL to the recommendation.wasm file
    ///   - embeddingDim: Embedding dimension (default: 64)
    ///   - numActions: Number of action slots (default: 100)
    public init(
        wasmPath: URL,
        embeddingDim: Int = 64,
        numActions: Int = 100
    ) throws {
        self.embeddingDim = embeddingDim
        self.numActions = numActions

        // Load WASM module
        // In real implementation, use WasmKit:
        // let runtime = Runtime()
        // let wasmData = try Data(contentsOf: wasmPath)
        // module = try Module(bytes: Array(wasmData))
        // instance = try module.instantiate(runtime: runtime)

        self.wasmModule = NSNull() // Placeholder
        self.wasmInstance = NSNull() // Placeholder

        // Bind exported functions
        try bindExports()

        // Initialize engine
        let result = initFunc?(UInt32(embeddingDim), UInt32(numActions)) ?? -1
        guard result == 0 else {
            throw WasmEngineError.initializationFailed
        }
    }

    /// Bind WASM exported functions
    private func bindExports() throws {
        // In real implementation with WasmKit:
        // initFunc = instance.exports["init"] as? Function
        // embedContentFunc = instance.exports["embed_content"] as? Function
        // ... etc

        // For demonstration, these would be populated from the WASM instance
    }

    // MARK: - Content Embedding

    /// Generate embedding for content
    /// - Parameter content: Content metadata
    /// - Returns: Embedding vector as Float array
    public func embed(content: ContentMetadata) async throws -> [Float] {
        guard let embedFunc = embedContentFunc else {
            throw WasmEngineError.functionNotFound("embed_content")
        }

        guard let ptr = embedFunc(
            content.id,
            content.contentType.rawValue,
            content.durationSecs,
            content.categoryFlags,
            content.popularity,
            content.recency
        ) else {
            throw WasmEngineError.embeddingFailed
        }

        // Copy embedding from WASM memory
        let buffer = UnsafeBufferPointer(start: ptr, count: embeddingDim)
        return Array(buffer)
    }

    // MARK: - Vibe State

    /// Set the current user vibe state
    /// - Parameter vibe: User's current vibe/mood state
    public func setVibe(_ vibe: VibeState) {
        setVibeFunc?(
            vibe.energy,
            vibe.mood,
            vibe.focus,
            vibe.timeContext,
            vibe.preferences.0,
            vibe.preferences.1,
            vibe.preferences.2,
            vibe.preferences.3
        )
    }

    // MARK: - Recommendations

    /// Get recommendations based on current vibe and history
    /// - Parameters:
    ///   - candidates: Array of candidate content IDs
    ///   - topK: Number of recommendations to return
    /// - Returns: Array of recommendations sorted by score
    public func recommend(
        candidates: [UInt64],
        topK: Int = 10
    ) async throws -> [Recommendation] {
        guard let getRecsFunc = getRecommendationsFunc else {
            throw WasmEngineError.functionNotFound("get_recommendations")
        }

        // Prepare output buffer (12 bytes per recommendation: 8 for ID + 4 for score)
        let outputSize = topK * 12
        var outputBuffer = [UInt8](repeating: 0, count: outputSize)

        let count = candidates.withUnsafeBufferPointer { candidatesPtr in
            outputBuffer.withUnsafeMutableBufferPointer { outputPtr in
                getRecsFunc(
                    candidatesPtr.baseAddress!,
                    UInt32(candidates.count),
                    UInt32(topK),
                    outputPtr.baseAddress!
                )
            }
        }

        // Parse results
        var recommendations: [Recommendation] = []
        for i in 0..<Int(count) {
            let offset = i * 12

            // Extract ID (8 bytes, little-endian)
            let id = outputBuffer[offset..<offset+8].withUnsafeBytes { ptr in
                ptr.load(as: UInt64.self)
            }

            // Extract score (4 bytes, little-endian)
            let score = outputBuffer[offset+8..<offset+12].withUnsafeBytes { ptr in
                ptr.load(as: Float.self)
            }

            recommendations.append(Recommendation(contentId: id, score: score))
        }

        return recommendations
    }

    // MARK: - Learning

    /// Record a user interaction for learning
    /// - Parameter interaction: User interaction event
    public func learn(interaction: UserInteraction) async throws {
        updateLearningFunc?(
            interaction.contentId,
            interaction.interaction.rawValue,
            interaction.timeSpent,
            interaction.position
        )
    }

    // MARK: - Similarity

    /// Compute similarity between two content items
    /// - Parameters:
    ///   - idA: First content ID
    ///   - idB: Second content ID
    /// - Returns: Cosine similarity (-1.0 to 1.0)
    public func similarity(between idA: UInt64, and idB: UInt64) -> Float {
        return computeSimilarityFunc?(idA, idB) ?? 0.0
    }

    // MARK: - State Persistence

    /// Save engine state for persistence
    /// - Returns: Serialized state data
    public func saveState() throws -> Data {
        guard let saveFunc = saveStateFunc else {
            throw WasmEngineError.functionNotFound("save_state")
        }

        let size = saveFunc()
        guard size > 0 else {
            throw WasmEngineError.saveFailed
        }

        // Read from WASM memory at the memory pool location
        // In real implementation, get pointer from get_memory_ptr()
        return Data() // Placeholder
    }

    /// Load engine state from persisted data
    /// - Parameter data: Previously saved state data
    public func loadState(_ data: Data) throws {
        guard let loadFunc = loadStateFunc else {
            throw WasmEngineError.functionNotFound("load_state")
        }

        let result = data.withUnsafeBytes { ptr in
            loadFunc(ptr.baseAddress!.assumingMemoryBound(to: UInt8.self), UInt32(data.count))
        }

        guard result == 0 else {
            throw WasmEngineError.loadFailed
        }
    }

    // MARK: - Statistics

    /// Get current exploration rate
    public var explorationRate: Float {
        return getExplorationRateFunc?() ?? 0.0
    }

    /// Get total learning updates
    public var updateCount: UInt64 {
        return getUpdateCountFunc?() ?? 0
    }

    /// Get embedding dimension
    public var dimension: Int {
        return Int(getEmbeddingDimFunc?() ?? 0)
    }
}

// MARK: - State Manager

/// Actor for thread-safe state persistence
public actor WasmStateManager {
    private let stateURL: URL

    public init(stateURL: URL? = nil) {
        self.stateURL = stateURL ?? FileManager.default
            .urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("recommendation_state.bin")
    }

    /// Save state to disk
    public func saveState(_ data: Data) async throws {
        try data.write(to: stateURL, options: .atomic)
    }

    /// Load state from disk
    public func loadState() async throws -> Data? {
        guard FileManager.default.fileExists(atPath: stateURL.path) else {
            return nil
        }
        return try Data(contentsOf: stateURL)
    }

    /// Delete saved state
    public func clearState() async throws {
        if FileManager.default.fileExists(atPath: stateURL.path) {
            try FileManager.default.removeItem(at: stateURL)
        }
    }
}

// MARK: - Errors

public enum WasmEngineError: Error, LocalizedError {
    case initializationFailed
    case functionNotFound(String)
    case embeddingFailed
    case saveFailed
    case loadFailed
    case invalidInput(String)

    public var errorDescription: String? {
        switch self {
        case .initializationFailed:
            return "Failed to initialize WASM recommendation engine"
        case .functionNotFound(let name):
            return "WASM function not found: \(name)"
        case .embeddingFailed:
            return "Failed to generate content embedding"
        case .saveFailed:
            return "Failed to save engine state"
        case .loadFailed:
            return "Failed to load engine state"
        case .invalidInput(let message):
            return "Invalid input: \(message)"
        }
    }
}

// MARK: - Extensions

extension ContentMetadata {
    /// Create from dictionary (useful for decoding from API)
    public init?(from dict: [String: Any]) {
        guard let id = dict["id"] as? UInt64,
              let typeRaw = dict["type"] as? UInt8,
              let type = ContentType(rawValue: typeRaw) else {
            return nil
        }

        self.init(
            id: id,
            contentType: type,
            durationSecs: dict["duration"] as? UInt32 ?? 0,
            categoryFlags: dict["categories"] as? UInt32 ?? 0,
            popularity: dict["popularity"] as? Float ?? 0.5,
            recency: dict["recency"] as? Float ?? 0.5
        )
    }
}
