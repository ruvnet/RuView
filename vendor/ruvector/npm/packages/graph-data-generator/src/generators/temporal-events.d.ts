/**
 * Temporal events generator for time-series graph data
 */
import { OpenRouterClient } from '../openrouter-client.js';
import { TemporalEventOptions, TemporalEvent, GraphData, GraphGenerationResult } from '../types.js';
export declare class TemporalEventsGenerator {
    private client;
    constructor(client: OpenRouterClient);
    /**
     * Generate temporal event graph data
     */
    generate(options: TemporalEventOptions): Promise<GraphGenerationResult<GraphData>>;
    /**
     * Generate temporal events
     */
    private generateEvents;
    /**
     * Generate entities from events
     */
    private generateEntities;
    /**
     * Analyze temporal patterns
     */
    analyzeTemporalPatterns(events: TemporalEvent[]): Promise<{
        eventsPerHour: Record<string, number>;
        eventTypeDistribution: Record<string, number>;
        avgTimeBetweenEvents: number;
    }>;
}
/**
 * Create a temporal events generator
 */
export declare function createTemporalEventsGenerator(client: OpenRouterClient): TemporalEventsGenerator;
//# sourceMappingURL=temporal-events.d.ts.map