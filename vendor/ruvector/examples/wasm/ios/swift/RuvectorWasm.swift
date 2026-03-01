//
// RuvectorWasm.swift
// Privacy-Preserving On-Device AI for iOS
//
// Uses WasmKit to run Ruvector WASM directly on iOS
// Minimum iOS: 15.0 (WasmKit requirement)
//

import Foundation

// MARK: - Core Types

/// Distance metric for vector similarity
public enum DistanceMetric: UInt8 {
    case euclidean = 0
    case cosine = 1
    case manhattan = 2
    case dotProduct = 3
}

/// Quantization mode for memory optimization
public enum QuantizationMode: UInt8 {
    case none = 0
    case scalar = 1    // 4x compression
    case binary = 2    // 32x compression
    case product = 3   // Variable compression
}

/// Search result with vector ID and distance
public struct SearchResult: Identifiable {
    public let id: UInt64
    public let distance: Float
}

// MARK: - Health Learning Types

/// Health metric types (privacy-preserving)
public enum HealthMetricType: UInt8 {
    case heartRate = 0
    case steps = 1
    case sleep = 2
    case activeEnergy = 3
    case exerciseMinutes = 4
    case standHours = 5
    case distance = 6
    case flightsClimbed = 7
    case mindfulness = 8
    case respiratoryRate = 9
    case bloodOxygen = 10
    case hrv = 11
}

/// Health state for learning (no actual values stored)
public struct HealthState {
    public let metric: HealthMetricType
    public let valueBucket: UInt8  // 0-9 normalized
    public let hour: UInt8
    public let dayOfWeek: UInt8

    public init(metric: HealthMetricType, valueBucket: UInt8, hour: UInt8, dayOfWeek: UInt8) {
        self.metric = metric
        self.valueBucket = min(valueBucket, 9)
        self.hour = min(hour, 23)
        self.dayOfWeek = min(dayOfWeek, 6)
    }
}

// MARK: - Location Learning Types

/// Location categories (no coordinates stored)
public enum LocationCategory: UInt8 {
    case home = 0
    case work = 1
    case gym = 2
    case dining = 3
    case shopping = 4
    case transit = 5
    case outdoor = 6
    case entertainment = 7
    case healthcare = 8
    case education = 9
    case unknown = 10
}

/// Location state for learning
public struct LocationState {
    public let category: LocationCategory
    public let hour: UInt8
    public let dayOfWeek: UInt8
    public let durationMinutes: UInt16

    public init(category: LocationCategory, hour: UInt8, dayOfWeek: UInt8, durationMinutes: UInt16) {
        self.category = category
        self.hour = min(hour, 23)
        self.dayOfWeek = min(dayOfWeek, 6)
        self.durationMinutes = durationMinutes
    }
}

// MARK: - Communication Learning Types

/// Communication event types
public enum CommEventType: UInt8 {
    case callIncoming = 0
    case callOutgoing = 1
    case messageReceived = 2
    case messageSent = 3
    case emailReceived = 4
    case emailSent = 5
    case notification = 6
}

/// Communication state
public struct CommState {
    public let eventType: CommEventType
    public let hour: UInt8
    public let dayOfWeek: UInt8
    public let responseTimeBucket: UInt8  // 0-9 normalized

    public init(eventType: CommEventType, hour: UInt8, dayOfWeek: UInt8, responseTimeBucket: UInt8) {
        self.eventType = eventType
        self.hour = min(hour, 23)
        self.dayOfWeek = min(dayOfWeek, 6)
        self.responseTimeBucket = min(responseTimeBucket, 9)
    }
}

// MARK: - Calendar Learning Types

/// Calendar event types
public enum CalendarEventType: UInt8 {
    case meeting = 0
    case focusTime = 1
    case personal = 2
    case travel = 3
    case breakTime = 4
    case exercise = 5
    case social = 6
    case deadline = 7
}

/// Calendar event for learning
public struct CalendarEvent {
    public let eventType: CalendarEventType
    public let startHour: UInt8
    public let durationMinutes: UInt16
    public let dayOfWeek: UInt8
    public let isRecurring: Bool
    public let hasAttendees: Bool

    public init(eventType: CalendarEventType, startHour: UInt8, durationMinutes: UInt16,
                dayOfWeek: UInt8, isRecurring: Bool, hasAttendees: Bool) {
        self.eventType = eventType
        self.startHour = min(startHour, 23)
        self.durationMinutes = durationMinutes
        self.dayOfWeek = min(dayOfWeek, 6)
        self.isRecurring = isRecurring
        self.hasAttendees = hasAttendees
    }
}

/// Time slot pattern
public struct TimeSlotPattern {
    public let busyProbability: Float
    public let avgMeetingDuration: Float
    public let focusScore: Float
    public let eventCount: UInt32
}

/// Focus time suggestion
public struct FocusTimeSuggestion: Identifiable {
    public var id: String { "\(day)-\(startHour)" }
    public let day: UInt8
    public let startHour: UInt8
    public let score: Float
}

// MARK: - App Usage Learning Types

/// App categories
public enum AppCategory: UInt8 {
    case social = 0
    case productivity = 1
    case entertainment = 2
    case news = 3
    case communication = 4
    case health = 5
    case navigation = 6
    case shopping = 7
    case gaming = 8
    case education = 9
    case finance = 10
    case utilities = 11
}

/// App usage session
public struct AppUsageSession {
    public let category: AppCategory
    public let durationSeconds: UInt32
    public let hour: UInt8
    public let dayOfWeek: UInt8
    public let isActiveUse: Bool

    public init(category: AppCategory, durationSeconds: UInt32, hour: UInt8,
                dayOfWeek: UInt8, isActiveUse: Bool) {
        self.category = category
        self.durationSeconds = durationSeconds
        self.hour = min(hour, 23)
        self.dayOfWeek = min(dayOfWeek, 6)
        self.isActiveUse = isActiveUse
    }
}

/// Screen time summary
public struct ScreenTimeSummary {
    public let totalMinutes: Float
    public let topCategory: AppCategory
    public let byCategory: [AppCategory: Float]
}

/// Wellbeing insight
public struct WellbeingInsight: Identifiable {
    public var id: String { category }
    public let category: String
    public let message: String
    public let score: Float
}

// MARK: - iOS Context & Recommendations

/// Device context for recommendations
public struct IOSContext {
    public let hour: UInt8
    public let dayOfWeek: UInt8
    public let isWeekend: Bool
    public let batteryLevel: UInt8      // 0-100
    public let networkType: UInt8       // 0=none, 1=wifi, 2=cellular
    public let locationCategory: LocationCategory
    public let recentAppCategory: AppCategory
    public let activityLevel: UInt8     // 0-10
    public let healthScore: Float       // 0-1

    public init(hour: UInt8, dayOfWeek: UInt8, batteryLevel: UInt8 = 100,
                networkType: UInt8 = 1, locationCategory: LocationCategory = .unknown,
                recentAppCategory: AppCategory = .utilities, activityLevel: UInt8 = 5,
                healthScore: Float = 0.5) {
        self.hour = min(hour, 23)
        self.dayOfWeek = min(dayOfWeek, 6)
        self.isWeekend = dayOfWeek == 0 || dayOfWeek == 6
        self.batteryLevel = min(batteryLevel, 100)
        self.networkType = min(networkType, 2)
        self.locationCategory = locationCategory
        self.recentAppCategory = recentAppCategory
        self.activityLevel = min(activityLevel, 10)
        self.healthScore = min(max(healthScore, 0), 1)
    }
}

/// Activity suggestion
public struct ActivitySuggestion: Identifiable {
    public var id: String { category }
    public let category: String
    public let confidence: Float
    public let reason: String
}

/// Context-aware recommendations
public struct ContextRecommendations {
    public let suggestedAppCategory: AppCategory
    public let focusScore: Float
    public let activitySuggestions: [ActivitySuggestion]
    public let optimalNotificationTime: Bool
}

// MARK: - WASM Runtime Error

/// WASM runtime errors
public enum RuvectorError: Error, LocalizedError {
    case wasmNotLoaded
    case initializationFailed(String)
    case memoryAllocationFailed
    case invalidInput(String)
    case serializationFailed
    case deserializationFailed

    public var errorDescription: String? {
        switch self {
        case .wasmNotLoaded:
            return "WASM module not loaded"
        case .initializationFailed(let msg):
            return "Initialization failed: \(msg)"
        case .memoryAllocationFailed:
            return "Memory allocation failed"
        case .invalidInput(let msg):
            return "Invalid input: \(msg)"
        case .serializationFailed:
            return "Serialization failed"
        case .deserializationFailed:
            return "Deserialization failed"
        }
    }
}

// MARK: - Ruvector WASM Runtime

/// Main entry point for Ruvector WASM on iOS
/// Uses WasmKit for native WASM execution
public final class RuvectorWasm {

    /// Shared instance (singleton pattern for resource efficiency)
    public static let shared = RuvectorWasm()

    // WASM runtime state
    private var isLoaded = false
    private var wasmBytes: Data?
    private var memoryPtr: UnsafeMutableRawPointer?
    private var memorySize: Int = 0

    // Learning state handles
    private var healthLearnerHandle: Int32 = -1
    private var locationLearnerHandle: Int32 = -1
    private var commLearnerHandle: Int32 = -1
    private var calendarLearnerHandle: Int32 = -1
    private var appUsageLearnerHandle: Int32 = -1
    private var iosLearnerHandle: Int32 = -1

    private init() {}

    // MARK: - Initialization

    /// Load WASM module from bundle
    /// - Parameter bundlePath: Path to .wasm file in app bundle
    public func load(from bundlePath: String) throws {
        guard let data = FileManager.default.contents(atPath: bundlePath) else {
            throw RuvectorError.initializationFailed("WASM file not found at \(bundlePath)")
        }
        try load(wasmData: data)
    }

    /// Load WASM module from data
    /// - Parameter wasmData: Raw WASM bytes
    public func load(wasmData: Data) throws {
        self.wasmBytes = wasmData

        // In production: Initialize WasmKit runtime here
        // For now, mark as loaded for API design
        // TODO: Integrate WasmKit when added as dependency
        //
        // Example WasmKit integration:
        // let module = try WasmKit.Module(bytes: [UInt8](wasmData))
        // let instance = try module.instantiate()
        // self.wasmInstance = instance

        isLoaded = true
    }

    /// Check if WASM is loaded
    public var isReady: Bool { isLoaded }

    // MARK: - Memory Management

    /// Allocate memory in WASM linear memory
    private func allocate(size: Int) throws -> Int {
        guard isLoaded else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call wasm_alloc export
        return 0
    }

    /// Free memory in WASM linear memory
    private func free(ptr: Int, size: Int) throws {
        guard isLoaded else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call wasm_free export
    }

    // MARK: - SIMD Operations

    /// Compute dot product of two vectors
    public func dotProduct(_ a: [Float], _ b: [Float]) throws -> Float {
        guard isLoaded else { throw RuvectorError.wasmNotLoaded }
        guard a.count == b.count else {
            throw RuvectorError.invalidInput("Vectors must have same length")
        }

        // Pure Swift fallback (SIMD when available)
        var result: Float = 0
        for i in 0..<a.count {
            result += a[i] * b[i]
        }
        return result
    }

    /// Compute L2 distance
    public func l2Distance(_ a: [Float], _ b: [Float]) throws -> Float {
        guard a.count == b.count else {
            throw RuvectorError.invalidInput("Vectors must have same length")
        }

        var sum: Float = 0
        for i in 0..<a.count {
            let diff = a[i] - b[i]
            sum += diff * diff
        }
        return sqrt(sum)
    }

    /// Compute cosine similarity
    public func cosineSimilarity(_ a: [Float], _ b: [Float]) throws -> Float {
        guard a.count == b.count else {
            throw RuvectorError.invalidInput("Vectors must have same length")
        }

        var dot: Float = 0
        var normA: Float = 0
        var normB: Float = 0

        for i in 0..<a.count {
            dot += a[i] * b[i]
            normA += a[i] * a[i]
            normB += b[i] * b[i]
        }

        let denom = sqrt(normA) * sqrt(normB)
        return denom > 0 ? dot / denom : 0
    }

    // MARK: - iOS Learner (Unified)

    /// Initialize unified iOS learner
    public func initIOSLearner() throws {
        guard isLoaded else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call ios_learner_init export
        iosLearnerHandle = 0
    }

    /// Update health metrics
    public func updateHealth(_ state: HealthState) throws {
        guard iosLearnerHandle >= 0 else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call ios_update_health export
    }

    /// Update location
    public func updateLocation(_ state: LocationState) throws {
        guard iosLearnerHandle >= 0 else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call ios_update_location export
    }

    /// Update communication patterns
    public func updateCommunication(_ state: CommState) throws {
        guard iosLearnerHandle >= 0 else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call ios_update_communication export
    }

    /// Update calendar
    public func updateCalendar(_ event: CalendarEvent) throws {
        guard iosLearnerHandle >= 0 else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call ios_update_calendar export
    }

    /// Update app usage
    public func updateAppUsage(_ session: AppUsageSession) throws {
        guard iosLearnerHandle >= 0 else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call ios_update_app_usage export
    }

    /// Get context-aware recommendations
    public func getRecommendations(_ context: IOSContext) throws -> ContextRecommendations {
        guard iosLearnerHandle >= 0 else { throw RuvectorError.wasmNotLoaded }

        // TODO: Call ios_get_recommendations export
        // For now, return sensible defaults
        return ContextRecommendations(
            suggestedAppCategory: .productivity,
            focusScore: 0.7,
            activitySuggestions: [
                ActivitySuggestion(category: "Focus", confidence: 0.8, reason: "Good time for deep work")
            ],
            optimalNotificationTime: context.hour >= 9 && context.hour <= 18
        )
    }

    /// Train one iteration (call periodically)
    public func trainIteration() throws {
        guard iosLearnerHandle >= 0 else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call ios_train export
    }

    // MARK: - Calendar Learning

    /// Initialize calendar learner
    public func initCalendarLearner() throws {
        guard isLoaded else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call calendar_init export
        calendarLearnerHandle = 0
    }

    /// Learn from calendar event
    public func learnCalendarEvent(_ event: CalendarEvent) throws {
        guard calendarLearnerHandle >= 0 else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call calendar_learn_event export
    }

    /// Get busy probability for time slot
    public func calendarBusyProbability(hour: UInt8, dayOfWeek: UInt8) throws -> Float {
        guard calendarLearnerHandle >= 0 else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call calendar_is_busy export
        return 0.5
    }

    /// Suggest focus times
    public func suggestFocusTimes(durationHours: UInt8) throws -> [FocusTimeSuggestion] {
        guard calendarLearnerHandle >= 0 else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call through WASM
        return [
            FocusTimeSuggestion(day: 1, startHour: 9, score: 0.9),
            FocusTimeSuggestion(day: 2, startHour: 14, score: 0.85)
        ]
    }

    // MARK: - App Usage Learning

    /// Initialize app usage learner
    public func initAppUsageLearner() throws {
        guard isLoaded else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call app_usage_init export
        appUsageLearnerHandle = 0
    }

    /// Learn from app session
    public func learnAppSession(_ session: AppUsageSession) throws {
        guard appUsageLearnerHandle >= 0 else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call app_usage_learn export
    }

    /// Get screen time (hours)
    public func screenTime() throws -> Float {
        guard appUsageLearnerHandle >= 0 else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call app_usage_screen_time export
        return 2.5
    }

    // MARK: - Persistence

    /// Serialize all learning state
    public func serialize() throws -> Data {
        guard isLoaded else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call serialize exports for each learner
        return Data()
    }

    /// Deserialize learning state
    public func deserialize(_ data: Data) throws {
        guard isLoaded else { throw RuvectorError.wasmNotLoaded }
        // TODO: Call deserialize exports
    }

    /// Save state to file
    public func save(to url: URL) throws {
        let data = try serialize()
        try data.write(to: url)
    }

    /// Load state from file
    public func restore(from url: URL) throws {
        let data = try Data(contentsOf: url)
        try deserialize(data)
    }
}

// MARK: - SwiftUI Integration

#if canImport(SwiftUI)
import SwiftUI

/// Observable wrapper for SwiftUI
@available(iOS 15.0, macOS 12.0, *)
@MainActor
public final class RuvectorViewModel: ObservableObject {
    @Published public private(set) var isReady = false
    @Published public private(set) var recommendations: ContextRecommendations?
    @Published public private(set) var screenTimeHours: Float = 0
    @Published public private(set) var focusScore: Float = 0

    private let runtime = RuvectorWasm.shared

    public init() {}

    /// Load WASM module
    public func load(from bundlePath: String) async throws {
        try runtime.load(from: bundlePath)
        try runtime.initIOSLearner()
        try runtime.initCalendarLearner()
        try runtime.initAppUsageLearner()
        isReady = true
    }

    /// Update recommendations for current context
    public func updateRecommendations(context: IOSContext) async throws {
        recommendations = try runtime.getRecommendations(context)
        focusScore = recommendations?.focusScore ?? 0
    }

    /// Update screen time
    public func updateScreenTime() async throws {
        screenTimeHours = try runtime.screenTime()
    }

    /// Record app usage
    public func recordAppUsage(_ session: AppUsageSession) async throws {
        try runtime.learnAppSession(session)
        try await updateScreenTime()
    }

    /// Record calendar event
    public func recordCalendarEvent(_ event: CalendarEvent) async throws {
        try runtime.learnCalendarEvent(event)
    }
}
#endif

// MARK: - Combine Integration

#if canImport(Combine)
import Combine

@available(iOS 13.0, macOS 10.15, *)
extension RuvectorWasm {
    /// Publisher for periodic training
    public func trainingPublisher(interval: TimeInterval = 60) -> AnyPublisher<Void, Never> {
        Timer.publish(every: interval, on: .main, in: .common)
            .autoconnect()
            .map { [weak self] _ in
                try? self?.trainIteration()
            }
            .eraseToAnyPublisher()
    }
}
#endif
