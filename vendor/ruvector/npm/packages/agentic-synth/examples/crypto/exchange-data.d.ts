/**
 * Cryptocurrency Exchange Data Generation
 *
 * Examples for generating realistic crypto exchange data including:
 * - OHLCV (Open, High, Low, Close, Volume) data
 * - Order book snapshots and updates
 * - Trade execution data
 * - Liquidity pool metrics
 * - CEX (Centralized Exchange) and DEX (Decentralized Exchange) patterns
 */
/**
 * Example 1: Generate OHLCV data for multiple cryptocurrencies
 * Simulates 24/7 crypto market with realistic price movements
 */
declare function generateOHLCV(): Promise<{
    symbol: string;
    data: unknown[];
}[]>;
/**
 * Example 2: Generate realistic order book data
 * Includes bid/ask spreads, market depth, and price levels
 */
declare function generateOrderBook(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 3: Generate trade execution data
 * Simulates actual trades with realistic patterns
 */
declare function generateTrades(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 4: Generate liquidity pool data (DEX)
 * Simulates AMM (Automated Market Maker) pools
 */
declare function generateLiquidityPools(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 5: Generate cross-exchange arbitrage opportunities
 * Simulates price differences across exchanges
 */
declare function generateArbitrageOpportunities(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 6: Generate 24/7 market data with realistic patterns
 * Includes timezone effects and global trading sessions
 */
declare function generate24x7MarketData(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 7: Generate funding rate data (perpetual futures)
 * Important for derivatives trading
 */
declare function generateFundingRates(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 8: Generate streaming real-time market data
 * Simulates WebSocket-like continuous data feed
 */
declare function streamMarketData(): Promise<void>;
/**
 * Run all examples
 */
export declare function runExchangeDataExamples(): Promise<void>;
export { generateOHLCV, generateOrderBook, generateTrades, generateLiquidityPools, generateArbitrageOpportunities, generate24x7MarketData, generateFundingRates, streamMarketData };
//# sourceMappingURL=exchange-data.d.ts.map