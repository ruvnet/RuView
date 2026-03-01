/**
 * Time-series data generator
 */
import { BaseGenerator } from './base.js';
import { TimeSeriesOptions } from '../types.js';
export declare class TimeSeriesGenerator extends BaseGenerator<TimeSeriesOptions> {
    protected generatePrompt(options: TimeSeriesOptions): string;
    protected parseResult(response: string, options: TimeSeriesOptions): unknown[];
    /**
     * Generate synthetic time-series with local computation (faster for simple patterns)
     */
    generateLocal(options: TimeSeriesOptions): Promise<Array<Record<string, unknown>>>;
    private parseInterval;
}
//# sourceMappingURL=timeseries.d.ts.map