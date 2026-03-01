/**
 * Stock Market Simulator
 * Generate realistic OHLCV financial data
 */
import type { StockDataPoint } from '../types/index.js';
export interface StockSimulatorConfig {
    symbols: string[];
    startDate: string | Date;
    endDate: string | Date;
    volatility: 'low' | 'medium' | 'high';
    includeWeekends?: boolean;
}
export interface GenerateOptions {
    includeNews?: boolean;
    includeSentiment?: boolean;
    marketConditions?: 'bearish' | 'neutral' | 'bullish';
}
export declare class StockMarketSimulator {
    private config;
    private volatilityMultiplier;
    constructor(config: StockSimulatorConfig);
    /**
     * Generate stock market data
     */
    generate(options?: GenerateOptions): Promise<StockDataPoint[]>;
    /**
     * Generate data for a single symbol
     */
    private generateSymbol;
    /**
     * Generate a single data point (day)
     */
    private generateDataPoint;
    /**
     * Get initial price for symbol
     */
    private getInitialPrice;
    /**
     * Get base trading volume for symbol
     */
    private getBaseVolume;
    /**
     * Get volatility multiplier
     */
    private getVolatilityMultiplier;
    /**
     * Get trend multiplier based on market conditions
     */
    private getTrendMultiplier;
    /**
     * Check if date is weekend
     */
    private isWeekend;
    /**
     * Generate sentiment score based on price movement
     */
    private generateSentiment;
    /**
     * Generate realistic news headlines
     */
    private generateNews;
    /**
     * Get market statistics
     */
    getStatistics(data: StockDataPoint[]): Record<string, any>;
    /**
     * Calculate price volatility (standard deviation)
     */
    private calculateVolatility;
}
//# sourceMappingURL=stock-market.d.ts.map