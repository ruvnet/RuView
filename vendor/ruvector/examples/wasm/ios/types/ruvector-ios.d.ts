/**
 * Ruvector iOS WASM - TypeScript Definitions
 *
 * Privacy-Preserving On-Device AI for iOS/Safari/Chrome
 *
 * @packageDocumentation
 */

// ============================================
// DISTANCE METRICS
// ============================================

/** Distance metric for vector similarity */
export type DistanceMetric = 'euclidean' | 'cosine' | 'manhattan' | 'dot_product';

/** Quantization mode for memory optimization */
export type QuantizationMode = 'none' | 'scalar' | 'binary' | 'product';

// ============================================
// CORE VECTOR DATABASE
// ============================================

/** Search result with vector ID and distance/score */
export interface SearchResult {
  id: number;
  distance: number;
}

/** Vector database with HNSW indexing */
export class VectorDatabaseJS {
  constructor(dimensions: number, metric?: DistanceMetric, quantization?: QuantizationMode);

  /** Insert a vector with ID */
  insert(id: number, vector: Float32Array): void;

  /** Search for k nearest neighbors */
  search(query: Float32Array, k: number): SearchResult[];

  /** Get vector by ID */
  get(id: number): Float32Array | undefined;

  /** Delete vector by ID */
  delete(id: number): boolean;

  /** Number of vectors stored */
  len(): number;

  /** Memory usage in bytes */
  memory_usage(): number;

  /** Serialize to bytes */
  serialize(): Uint8Array;

  /** Deserialize from bytes */
  static deserialize(data: Uint8Array): VectorDatabaseJS;
}

// ============================================
// HNSW INDEX
// ============================================

/** HNSW index configuration */
export interface HnswConfig {
  m?: number;          // Connections per node (default: 16)
  ef_construction?: number;  // Build quality (default: 200)
  ef_search?: number;  // Search quality (default: 50)
}

/** High-performance HNSW vector index */
export class HnswIndexJS {
  constructor(dimensions: number, metric?: DistanceMetric, config?: HnswConfig);

  /** Insert vector with ID */
  insert(id: number, vector: Float32Array): void;

  /** Search for k nearest neighbors */
  search(query: Float32Array, k: number): SearchResult[];

  /** Number of vectors */
  len(): number;

  /** Maximum layer depth */
  max_layer(): number;

  /** Serialize to bytes */
  serialize(): Uint8Array;

  /** Deserialize from bytes */
  static deserialize(data: Uint8Array): HnswIndexJS;
}

// ============================================
// RECOMMENDATION ENGINE
// ============================================

/** Recommendation with confidence score */
export interface Recommendation {
  item_id: number;
  score: number;
  embedding: Float32Array;
}

/** Recommendation engine with Q-learning */
export class RecommendationEngineJS {
  constructor(embedding_dim: number, vocab_size?: number);

  /** Record user interaction (click, purchase, etc.) */
  record_interaction(user_id: number, item_id: number, reward: number): void;

  /** Get recommendations for user */
  recommend(user_id: number, k: number): Recommendation[];

  /** Add item to catalog */
  add_item(item_id: number, features: Float32Array): void;

  /** Get similar items */
  similar_items(item_id: number, k: number): Recommendation[];

  /** Serialize state */
  serialize(): Uint8Array;

  /** Deserialize state */
  static deserialize(data: Uint8Array): RecommendationEngineJS;
}

// ============================================
// SIMD OPERATIONS
// ============================================

/** Compute dot product of two vectors */
export function dot_product(a: Float32Array, b: Float32Array): number;

/** Compute L2 (Euclidean) distance */
export function l2_distance(a: Float32Array, b: Float32Array): number;

/** Compute cosine similarity */
export function cosine_similarity(a: Float32Array, b: Float32Array): number;

/** Normalize vector to unit length */
export function normalize(v: Float32Array): Float32Array;

/** Compute L2 norm (length) of vector */
export function l2_norm(v: Float32Array): number;

// ============================================
// QUANTIZATION
// ============================================

/** Scalar quantized vector (8-bit) */
export class ScalarQuantizedJS {
  /** Quantize float vector to 8-bit */
  static quantize(vector: Float32Array): ScalarQuantizedJS;

  /** Dequantize back to float32 */
  dequantize(): Float32Array;

  /** Get quantized bytes */
  data(): Uint8Array;

  /** Memory size in bytes */
  memory_size(): number;

  /** Compute approximate distance to another quantized vector */
  distance_to(other: ScalarQuantizedJS): number;
}

/** Binary quantized vector (1-bit) */
export class BinaryQuantizedJS {
  /** Quantize float vector to binary */
  static quantize(vector: Float32Array): BinaryQuantizedJS;

  /** Get binary data */
  data(): Uint8Array;

  /** Memory size in bytes */
  memory_size(): number;

  /** Hamming distance to another binary vector */
  hamming_distance(other: BinaryQuantizedJS): number;
}

/** Product quantized vector (sub-vector clustering) */
export class ProductQuantizedJS {
  constructor(num_subvectors: number, bits_per_subvector: number);

  /** Train codebook on vectors */
  train(vectors: Float32Array[], iterations?: number): void;

  /** Encode vector */
  encode(vector: Float32Array): Uint8Array;

  /** Decode to approximate float vector */
  decode(codes: Uint8Array): Float32Array;

  /** Compute approximate distance */
  distance(codes_a: Uint8Array, codes_b: Uint8Array): number;
}

// ============================================
// IOS LEARNING MODULES
// ============================================

// --- Health Learning ---

/** Health metric types (privacy-preserving, no actual values stored) */
export const HealthMetrics: {
  readonly HEART_RATE: number;
  readonly STEPS: number;
  readonly SLEEP: number;
  readonly ACTIVE_ENERGY: number;
  readonly EXERCISE_MINUTES: number;
  readonly STAND_HOURS: number;
  readonly DISTANCE: number;
  readonly FLIGHTS_CLIMBED: number;
  readonly MINDFULNESS: number;
  readonly RESPIRATORY_RATE: number;
  readonly BLOOD_OXYGEN: number;
  readonly HRV: number;
};

/** Health state for learning */
export interface HealthState {
  metric: number;
  value_bucket: number;  // 0-9 normalized bucket, not actual value
  hour: number;
  day_of_week: number;
}

/** Privacy-preserving health pattern learner */
export class HealthLearnerJS {
  constructor();

  /** Learn from health event (stores only patterns, not values) */
  learn_event(state: HealthState): void;

  /** Predict typical value bucket for time */
  predict(metric: number, hour: number, day_of_week: number): number;

  /** Get activity score (0-1) */
  activity_score(): number;

  /** Get learned patterns */
  patterns(): object;

  /** Serialize for persistence */
  serialize(): Uint8Array;

  /** Deserialize */
  static deserialize(data: Uint8Array): HealthLearnerJS;
}

// --- Location Learning ---

/** Location categories (no coordinates stored) */
export const LocationCategories: {
  readonly HOME: number;
  readonly WORK: number;
  readonly GYM: number;
  readonly DINING: number;
  readonly SHOPPING: number;
  readonly TRANSIT: number;
  readonly OUTDOOR: number;
  readonly ENTERTAINMENT: number;
  readonly HEALTHCARE: number;
  readonly EDUCATION: number;
  readonly UNKNOWN: number;
};

/** Location state for learning */
export interface LocationState {
  category: number;
  hour: number;
  day_of_week: number;
  duration_minutes: number;
}

/** Privacy-preserving location pattern learner */
export class LocationLearnerJS {
  constructor();

  /** Learn from location visit */
  learn_visit(state: LocationState): void;

  /** Predict likely location for time */
  predict(hour: number, day_of_week: number): number;

  /** Get time spent at category today */
  time_at_category(category: number): number;

  /** Get mobility score (0-1) */
  mobility_score(): number;

  /** Serialize */
  serialize(): Uint8Array;

  /** Deserialize */
  static deserialize(data: Uint8Array): LocationLearnerJS;
}

// --- Communication Learning ---

/** Communication event types */
export const CommEventTypes: {
  readonly CALL_INCOMING: number;
  readonly CALL_OUTGOING: number;
  readonly MESSAGE_RECEIVED: number;
  readonly MESSAGE_SENT: number;
  readonly EMAIL_RECEIVED: number;
  readonly EMAIL_SENT: number;
  readonly NOTIFICATION: number;
};

/** Communication state */
export interface CommState {
  event_type: number;
  hour: number;
  day_of_week: number;
  response_time_bucket: number;  // 0-9 normalized
}

/** Privacy-preserving communication pattern learner */
export class CommLearnerJS {
  constructor();

  /** Learn from communication event */
  learn_event(state: CommState): void;

  /** Predict communication frequency for time */
  predict_frequency(hour: number, day_of_week: number): number;

  /** Is this a quiet period? */
  is_quiet_period(hour: number, day_of_week: number): boolean;

  /** Communication score (0-1) */
  communication_score(): number;

  /** Serialize */
  serialize(): Uint8Array;

  /** Deserialize */
  static deserialize(data: Uint8Array): CommLearnerJS;
}

// --- Calendar Learning ---

/** Calendar event types */
export const CalendarEventTypes: {
  readonly MEETING: number;
  readonly FOCUS_TIME: number;
  readonly PERSONAL: number;
  readonly TRAVEL: number;
  readonly BREAK: number;
  readonly EXERCISE: number;
  readonly SOCIAL: number;
  readonly DEADLINE: number;
};

/** Calendar event for learning */
export interface CalendarEvent {
  event_type: number;
  start_hour: number;
  duration_minutes: number;
  day_of_week: number;
  is_recurring: boolean;
  has_attendees: boolean;
}

/** Time slot pattern learned from calendar */
export interface TimeSlotPattern {
  busy_probability: number;
  avg_meeting_duration: number;
  focus_score: number;
  event_count: number;
}

/** Privacy-preserving calendar pattern learner */
export class CalendarLearnerJS {
  constructor();

  /** Learn from calendar event (no titles/details stored) */
  learn_event(event: CalendarEvent): void;

  /** Get busy probability for time slot */
  busy_probability(hour: number, day_of_week: number): number;

  /** Suggest best focus time blocks */
  suggest_focus_times(duration_hours: number): Array<{ day: number; start_hour: number; score: number }>;

  /** Suggest best meeting times */
  suggest_meeting_times(duration_minutes: number): Array<{ day: number; start_hour: number; score: number }>;

  /** Get pattern for time slot */
  pattern_at(hour: number, day_of_week: number): TimeSlotPattern;

  /** Serialize */
  serialize(): Uint8Array;

  /** Deserialize */
  static deserialize(data: Uint8Array): CalendarLearnerJS;
}

// --- App Usage Learning ---

/** App categories */
export const AppCategories: {
  readonly SOCIAL: number;
  readonly PRODUCTIVITY: number;
  readonly ENTERTAINMENT: number;
  readonly NEWS: number;
  readonly COMMUNICATION: number;
  readonly HEALTH: number;
  readonly NAVIGATION: number;
  readonly SHOPPING: number;
  readonly GAMING: number;
  readonly EDUCATION: number;
  readonly FINANCE: number;
  readonly UTILITIES: number;
};

/** App usage session */
export interface AppUsageSession {
  category: number;
  duration_seconds: number;
  hour: number;
  day_of_week: number;
  is_active_use: boolean;
}

/** App usage pattern */
export interface AppUsagePattern {
  total_duration: number;
  session_count: number;
  avg_session_length: number;
  peak_hour: number;
}

/** Screen time summary */
export interface ScreenTimeSummary {
  total_minutes: number;
  top_category: number;
  by_category: Map<number, number>;
}

/** Wellbeing insight */
export interface WellbeingInsight {
  category: string;
  message: string;
  score: number;
}

/** Privacy-preserving app usage learner */
export class AppUsageLearnerJS {
  constructor();

  /** Learn from app session (no app names stored) */
  learn_session(session: AppUsageSession): void;

  /** Predict most likely category for time */
  predict_category(hour: number, day_of_week: number): number;

  /** Get screen time summary for today */
  screen_time_summary(): ScreenTimeSummary;

  /** Get usage pattern for category */
  usage_pattern(category: number): AppUsagePattern;

  /** Get digital wellbeing insights */
  wellbeing_insights(): WellbeingInsight[];

  /** Serialize */
  serialize(): Uint8Array;

  /** Deserialize */
  static deserialize(data: Uint8Array): AppUsageLearnerJS;
}

// ============================================
// UNIFIED iOS LEARNER
// ============================================

/** Device context for recommendations */
export interface iOSContext {
  hour: number;
  day_of_week: number;
  is_weekend: boolean;
  battery_level: number;    // 0-100
  network_type: number;     // 0=none, 1=wifi, 2=cellular
  location_category: number;
  recent_app_category: number;
  activity_level: number;   // 0-10
  health_score: number;     // 0-1
}

/** Activity suggestion */
export interface ActivitySuggestion {
  category: string;
  confidence: number;
  reason: string;
}

/** Context-aware recommendations */
export interface ContextRecommendations {
  suggested_app_category: number;
  focus_score: number;
  activity_suggestions: ActivitySuggestion[];
  optimal_notification_time: boolean;
}

/** Unified iOS on-device learner */
export class iOSLearnerJS {
  constructor();

  /** Update health metrics */
  update_health(state: HealthState): void;

  /** Update location */
  update_location(state: LocationState): void;

  /** Update communication patterns */
  update_communication(state: CommState): void;

  /** Update calendar patterns */
  update_calendar(event: CalendarEvent): void;

  /** Update app usage */
  update_app_usage(session: AppUsageSession): void;

  /** Get context-aware recommendations */
  get_recommendations(context: iOSContext): ContextRecommendations;

  /** Train iteration (call periodically for Q-learning) */
  train_iteration(): void;

  /** Get learning iterations count */
  iterations(): number;

  /** Full state serialization */
  serialize(): Uint8Array;

  /** Deserialize full state */
  static deserialize(data: Uint8Array): iOSLearnerJS;
}

// ============================================
// iOS CAPABILITIES
// ============================================

/** Device capability detection */
export interface iOSCapabilities {
  supports_simd: boolean;
  supports_threads: boolean;
  supports_bulk_memory: boolean;
  supports_exception_handling: boolean;
  memory_mb: number;
  is_low_power_mode: boolean;
  thermal_state: 'nominal' | 'fair' | 'serious' | 'critical';
}

/** Detect device capabilities */
export function detect_capabilities(): iOSCapabilities;

/** Get optimal HNSW config for device */
export function optimal_hnsw_config(capabilities: iOSCapabilities): HnswConfig;

/** Get optimal quantization mode for device */
export function optimal_quantization(capabilities: iOSCapabilities, vector_count: number): QuantizationMode;

// ============================================
// WASM MODULE INITIALIZATION
// ============================================

/** Initialize the WASM module */
export default function init(module_or_path?: WebAssembly.Module | string): Promise<void>;

/** Memory stats */
export interface MemoryStats {
  used_bytes: number;
  allocated_bytes: number;
  peak_bytes: number;
}

/** Get current memory usage */
export function memory_stats(): MemoryStats;

/** Version info */
export const VERSION: string;
export const BUILD_DATE: string;
export const FEATURES: string[];
