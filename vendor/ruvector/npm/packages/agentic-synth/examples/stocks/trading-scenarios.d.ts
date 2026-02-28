/**
 * Trading Scenarios Generation
 *
 * Generate realistic market scenarios for testing trading systems:
 * - Bull/bear markets
 * - Volatility patterns
 * - Flash crashes
 * - Earnings announcements
 * - Market correlations
 */
/**
 * Generate sustained uptrend with occasional pullbacks
 */
declare function generateBullMarket(): Promise<any>;
/**
 * Generate sustained downtrend with sharp selloffs
 */
declare function generateBearMarket(): Promise<any>;
interface VolatilityRegime {
    timestamp: Date;
    price: number;
    realizedVol: number;
    impliedVol: number;
    vix: number;
    regime: 'low' | 'medium' | 'high' | 'extreme';
    symbol: string;
}
/**
 * Generate varying volatility regimes
 */
declare function generateVolatilityPatterns(): Promise<VolatilityRegime[]>;
interface FlashCrashEvent {
    phase: 'normal' | 'crash' | 'recovery';
    timestamp: Date;
    price: number;
    volume: number;
    spread: number;
    liquidityScore: number;
    symbol: string;
}
/**
 * Simulate flash crash with rapid price decline and recovery
 */
declare function generateFlashCrash(): Promise<FlashCrashEvent[]>;
interface EarningsEvent {
    phase: 'pre-announcement' | 'announcement' | 'post-announcement';
    timestamp: Date;
    price: number;
    volume: number;
    impliedVolatility: number;
    optionVolume: number;
    surprise: 'beat' | 'miss' | 'inline';
    symbol: string;
}
/**
 * Simulate earnings announcement with volatility crush
 */
declare function generateEarningsScenario(): Promise<EarningsEvent[]>;
interface CorrelationData {
    timestamp: Date;
    spy: number;
    qqq: number;
    iwm: number;
    vix: number;
    dxy: number;
    correlation_spy_qqq: number;
    correlation_spy_vix: number;
}
/**
 * Generate correlated multi-asset data
 */
declare function generateCorrelatedMarkets(): Promise<CorrelationData[]>;
export { generateBullMarket, generateBearMarket, generateVolatilityPatterns, generateFlashCrash, generateEarningsScenario, generateCorrelatedMarkets, };
//# sourceMappingURL=trading-scenarios.d.ts.map