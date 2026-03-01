/**
 * Event data generator
 */
import { BaseGenerator } from './base.js';
import { EventOptions } from '../types.js';
export declare class EventGenerator extends BaseGenerator<EventOptions> {
    protected generatePrompt(options: EventOptions): string;
    protected parseResult(response: string, options: EventOptions): unknown[];
    /**
     * Generate synthetic events with local computation
     */
    generateLocal(options: EventOptions): Promise<Array<Record<string, unknown>>>;
    private generateTimestamps;
    private generateMetadata;
}
//# sourceMappingURL=events.d.ts.map