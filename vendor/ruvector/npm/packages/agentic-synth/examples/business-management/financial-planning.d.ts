/**
 * Financial Planning and Analysis Data Generation
 * Simulates enterprise financial systems, budgeting, forecasting, and reporting
 */
/**
 * Generate Budget Planning Data
 */
export declare function generateBudgetPlans(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Revenue Forecasts
 */
export declare function generateRevenueForecasts(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Expense Tracking Data (time-series)
 */
export declare function generateExpenseTracking(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Cash Flow Projections
 */
export declare function generateCashFlowProjections(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate P&L Statements
 */
export declare function generateProfitLossStatements(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Balance Sheets
 */
export declare function generateBalanceSheets(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate KPI Dashboard Data (time-series)
 */
export declare function generateKPIDashboards(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate complete financial dataset in parallel
 */
export declare function generateCompleteFinancialDataset(): Promise<{
    budgets: unknown[];
    revenueForecasts: unknown[];
    expenses: unknown[];
    cashFlowProjections: unknown[];
    profitLossStatements: unknown[];
    balanceSheets: unknown[];
    kpiDashboards: unknown[];
    metadata: {
        totalRecords: number;
        generatedAt: string;
    };
}>;
declare const _default: {
    generateBudgetPlans: typeof generateBudgetPlans;
    generateRevenueForecasts: typeof generateRevenueForecasts;
    generateExpenseTracking: typeof generateExpenseTracking;
    generateCashFlowProjections: typeof generateCashFlowProjections;
    generateProfitLossStatements: typeof generateProfitLossStatements;
    generateBalanceSheets: typeof generateBalanceSheets;
    generateKPIDashboards: typeof generateKPIDashboards;
    generateCompleteFinancialDataset: typeof generateCompleteFinancialDataset;
};
export default _default;
//# sourceMappingURL=financial-planning.d.ts.map