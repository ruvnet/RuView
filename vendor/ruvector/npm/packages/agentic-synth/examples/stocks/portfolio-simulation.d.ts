/**
 * Portfolio Simulation and Management
 *
 * Generate realistic portfolio data for:
 * - Multi-asset portfolios
 * - Rebalancing scenarios
 * - Risk-adjusted returns
 * - Drawdown analysis
 * - Performance attribution
 */
interface Asset {
    symbol: string;
    assetClass: 'equity' | 'fixedIncome' | 'commodity' | 'crypto' | 'alternative';
    weight: number;
    expectedReturn: number;
    volatility: number;
}
interface PortfolioHolding {
    timestamp: Date;
    symbol: string;
    shares: number;
    price: number;
    marketValue: number;
    weight: number;
    dayReturn: number;
    totalReturn: number;
}
interface PortfolioMetrics {
    timestamp: Date;
    totalValue: number;
    cashBalance: number;
    totalReturn: number;
    dailyReturn: number;
    volatility: number;
    sharpeRatio: number;
    maxDrawdown: number;
    beta: number;
    alpha: number;
}
/**
 * Generate a diversified multi-asset portfolio
 */
declare function generateMultiAssetPortfolio(): Promise<{
    portfolioData: Map<string, PortfolioHolding[]>;
    portfolioMetrics: PortfolioMetrics[];
    assets: Asset[];
}>;
interface RebalanceEvent {
    timestamp: Date;
    type: 'calendar' | 'threshold' | 'opportunistic';
    holdings: PortfolioHolding[];
    targetWeights: Record<string, number>;
    actualWeights: Record<string, number>;
    trades: Trade[];
    transactionCosts: number;
}
interface Trade {
    symbol: string;
    action: 'buy' | 'sell';
    shares: number;
    price: number;
    value: number;
    commission: number;
}
/**
 * Generate portfolio rebalancing scenarios
 */
declare function generateRebalancingScenarios(): Promise<RebalanceEvent[]>;
interface RiskMetrics {
    timestamp: Date;
    portfolioReturn: number;
    benchmarkReturn: number;
    excessReturn: number;
    trackingError: number;
    informationRatio: number;
    sharpeRatio: number;
    sortinoRatio: number;
    calmarRatio: number;
    beta: number;
    alpha: number;
    correlation: number;
}
/**
 * Calculate comprehensive risk-adjusted return metrics
 */
declare function generateRiskAdjustedReturns(): Promise<RiskMetrics[]>;
interface DrawdownPeriod {
    startDate: Date;
    troughDate: Date;
    endDate: Date | null;
    peakValue: number;
    troughValue: number;
    recoveryValue: number | null;
    drawdown: number;
    duration: number;
    recoveryDuration: number | null;
    underwater: boolean;
}
/**
 * Analyze portfolio drawdowns
 */
declare function generateDrawdownAnalysis(): Promise<DrawdownPeriod[]>;
export { generateMultiAssetPortfolio, generateRebalancingScenarios, generateRiskAdjustedReturns, generateDrawdownAnalysis, };
//# sourceMappingURL=portfolio-simulation.d.ts.map