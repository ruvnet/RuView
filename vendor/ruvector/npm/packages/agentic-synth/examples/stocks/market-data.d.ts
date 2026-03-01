/**
 * Stock Market Data Generation Examples
 *
 * Demonstrates realistic OHLCV data generation, technical indicators,
 * multi-timeframe data, market depth, and tick-by-tick simulation.
 */
interface OHLCVBar {
    timestamp: Date;
    open: number;
    high: number;
    low: number;
    close: number;
    volume: number;
    symbol: string;
}
/**
 * Generate realistic OHLCV (candlestick) data with proper market microstructure
 */
declare function generateOHLCVData(): Promise<any>;
interface TechnicalIndicators {
    timestamp: Date;
    price: number;
    sma_20: number;
    sma_50: number;
    rsi_14: number;
    macd: number;
    macd_signal: number;
    bb_upper: number;
    bb_middle: number;
    bb_lower: number;
    volume: number;
    symbol: string;
}
/**
 * Generate price data with technical indicators pre-calculated
 */
declare function generateTechnicalIndicators(): Promise<TechnicalIndicators[]>;
interface MultiTimeframeData {
    '1m': OHLCVBar[];
    '5m': OHLCVBar[];
    '1h': OHLCVBar[];
    '1d': OHLCVBar[];
}
/**
 * Generate data across multiple timeframes with proper aggregation
 */
declare function generateMultiTimeframeData(): Promise<MultiTimeframeData>;
/**
 * Generate realistic Level 2 market depth data (order book)
 */
declare function generateMarketDepth(): Promise<any>;
/**
 * Generate high-frequency tick-by-tick trade data
 */
declare function generateTickData(): Promise<any>;
/**
 * Generate market microstructure metrics for analysis
 */
declare function generateMicrostructureMetrics(): Promise<any>;
export { generateOHLCVData, generateTechnicalIndicators, generateMultiTimeframeData, generateMarketDepth, generateTickData, generateMicrostructureMetrics, };
//# sourceMappingURL=market-data.d.ts.map