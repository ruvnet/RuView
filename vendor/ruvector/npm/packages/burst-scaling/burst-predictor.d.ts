/**
 * Burst Predictor - Predictive Scaling Engine
 *
 * Handles predictive scaling by analyzing:
 * - Event calendars (sports, releases, etc.)
 * - Historical traffic patterns
 * - ML-based load forecasting
 * - Regional load predictions
 */
export interface PredictedBurst {
    eventId: string;
    eventName: string;
    startTime: Date;
    endTime: Date;
    expectedMultiplier: number;
    confidence: number;
    regions: RegionalPrediction[];
    preWarmTime: number;
}
export interface RegionalPrediction {
    region: string;
    expectedLoad: number;
    requiredInstances: number;
    currentInstances: number;
}
export interface HistoricalPattern {
    eventType: string;
    avgMultiplier: number;
    avgDuration: number;
    peakTime: number;
    regionsAffected: string[];
}
export interface EventCalendar {
    events: CalendarEvent[];
}
export interface CalendarEvent {
    id: string;
    name: string;
    type: 'sports' | 'release' | 'promotion' | 'other';
    startTime: Date;
    region: string[];
    expectedViewers?: number;
}
export declare class BurstPredictor {
    private readonly regions;
    private readonly notifyHook;
    private historicalPatterns;
    private upcomingEvents;
    private readonly baseLoad;
    private readonly maxInstancesPerRegion;
    private readonly minInstancesPerRegion;
    constructor(regions?: string[], notifyHook?: (message: string) => Promise<void>);
    /**
     * Load historical patterns from past burst events
     */
    private loadHistoricalPatterns;
    /**
     * Load upcoming events from event calendar
     */
    loadEventCalendar(calendar: EventCalendar): Promise<void>;
    /**
     * Predict upcoming bursts based on event calendar and historical patterns
     */
    predictUpcomingBursts(lookaheadHours?: number): Promise<PredictedBurst[]>;
    /**
     * Predict burst characteristics for a specific event
     */
    private predictBurst;
    /**
     * ML-based multiplier adjustment
     * In production, this would use a trained model
     */
    private mlAdjustMultiplier;
    /**
     * Calculate confidence score for prediction
     */
    private calculateConfidence;
    /**
     * Predict load distribution across regions
     */
    private predictRegionalLoad;
    /**
     * Create conservative prediction when no historical data exists
     */
    private createConservativePrediction;
    /**
     * Analyze historical data to improve predictions
     */
    analyzeHistoricalData(startDate: Date, endDate: Date): Promise<Map<string, HistoricalPattern>>;
    /**
     * Get pre-warming schedule for upcoming events
     */
    getPreWarmingSchedule(): Promise<Array<{
        eventId: string;
        eventName: string;
        preWarmStartTime: Date;
        targetCapacity: number;
    }>>;
    /**
     * Train ML model on past burst events (simplified)
     */
    trainModel(trainingData: Array<{
        eventType: string;
        actualMultiplier: number;
        duration: number;
        features: Record<string, number>;
    }>): Promise<void>;
    /**
     * Get current prediction accuracy metrics
     */
    getPredictionAccuracy(): Promise<{
        accuracy: number;
        mape: number;
        predictions: number;
    }>;
}
//# sourceMappingURL=burst-predictor.d.ts.map