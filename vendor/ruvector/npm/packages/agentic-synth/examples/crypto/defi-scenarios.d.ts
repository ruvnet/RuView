/**
 * DeFi Protocol Simulation and Scenario Generation
 *
 * Examples for generating realistic DeFi data including:
 * - Yield farming returns and APY calculations
 * - Liquidity provision scenarios
 * - Impermanent loss simulations
 * - Gas price variations and optimization
 * - Smart contract interaction patterns
 */
/**
 * Example 1: Generate yield farming scenarios
 * Simulates various DeFi protocols and farming strategies
 */
declare function generateYieldFarmingData(): Promise<{
    protocol: string;
    data: unknown[];
}[]>;
/**
 * Example 2: Simulate liquidity provision scenarios
 * Includes LP token calculations and position management
 */
declare function generateLiquidityProvisionScenarios(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 3: Generate impermanent loss scenarios
 * Detailed analysis of IL under different market conditions
 */
declare function generateImpermanentLossScenarios(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 4: Generate gas price data and optimization scenarios
 * Critical for DeFi transaction cost analysis
 */
declare function generateGasPriceData(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 5: Generate smart contract interaction patterns
 * Simulates complex DeFi transaction sequences
 */
declare function generateSmartContractInteractions(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 6: Generate lending/borrowing scenarios
 * Simulates DeFi lending protocols like Aave and Compound
 */
declare function generateLendingScenarios(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 7: Generate staking rewards data
 * Covers liquid staking and validator rewards
 */
declare function generateStakingRewards(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 8: Generate MEV (Maximal Extractable Value) scenarios
 * Advanced DeFi strategy simulations
 */
declare function generateMEVScenarios(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Run all DeFi scenario examples
 */
export declare function runDeFiScenarioExamples(): Promise<void>;
export { generateYieldFarmingData, generateLiquidityProvisionScenarios, generateImpermanentLossScenarios, generateGasPriceData, generateSmartContractInteractions, generateLendingScenarios, generateStakingRewards, generateMEVScenarios };
//# sourceMappingURL=defi-scenarios.d.ts.map