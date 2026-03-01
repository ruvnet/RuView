/**
 * RuVector WASM Unified - Economy Engine
 *
 * Provides compute credit economy including:
 * - Credit balance management
 * - Contribution multipliers
 * - Staking mechanisms
 * - Transaction history
 * - Reward distribution
 */
import type { CreditAccount, Transaction, StakingPosition, EconomyMetrics, EconomyConfig } from './types';
/**
 * Core economy engine for compute credit management
 */
export interface EconomyEngine {
    /**
     * Get current credit balance
     * @returns Current balance in credits
     */
    creditBalance(): number;
    /**
     * Get contribution multiplier
     * Based on staking, history, and activity
     * @returns Multiplier value (1.0 = base rate)
     */
    contributionMultiplier(): number;
    /**
     * Get full account state
     * @returns Complete credit account information
     */
    getAccount(): CreditAccount;
    /**
     * Check if account can afford operation
     * @param cost Operation cost
     * @returns Whether balance is sufficient
     */
    canAfford(cost: number): boolean;
    /**
     * Deposit credits into staking
     * @param amount Amount to stake
     * @param lockDuration Optional lock duration in seconds
     * @returns Staking position
     */
    stakeDeposit(amount: number, lockDuration?: number): StakingPosition;
    /**
     * Withdraw from staking
     * @param amount Amount to withdraw
     * @returns Withdrawn amount (may include penalties)
     */
    stakeWithdraw(amount: number): number;
    /**
     * Get current staking positions
     * @returns Array of staking positions
     */
    getStakingPositions(): StakingPosition[];
    /**
     * Get total staked amount
     * @returns Total credits staked
     */
    getTotalStaked(): number;
    /**
     * Estimate staking rewards
     * @param amount Amount to stake
     * @param duration Duration in seconds
     * @returns Estimated reward
     */
    estimateStakingReward(amount: number, duration: number): number;
    /**
     * Transfer credits to another account
     * @param targetId Target account ID
     * @param amount Amount to transfer
     * @returns Transaction record
     */
    transfer(targetId: string, amount: number): Transaction;
    /**
     * Deposit credits from external source
     * @param amount Amount to deposit
     * @param source Source identifier
     * @returns Transaction record
     */
    deposit(amount: number, source?: string): Transaction;
    /**
     * Withdraw credits to external destination
     * @param amount Amount to withdraw
     * @param destination Destination identifier
     * @returns Transaction record
     */
    withdraw(amount: number, destination?: string): Transaction;
    /**
     * Get transaction history
     * @param options Filter options
     * @returns Array of transactions
     */
    getTransactionHistory(options?: TransactionFilter): Transaction[];
    /**
     * Get transaction by ID
     * @param transactionId Transaction ID
     * @returns Transaction or undefined
     */
    getTransaction(transactionId: string): Transaction | undefined;
    /**
     * Claim pending rewards
     * @returns Amount claimed
     */
    claimRewards(): number;
    /**
     * Get pending rewards
     * @returns Amount of unclaimed rewards
     */
    getPendingRewards(): number;
    /**
     * Record contribution for rewards
     * @param contributionType Type of contribution
     * @param value Contribution value
     */
    recordContribution(contributionType: ContributionType, value: number): void;
    /**
     * Get contribution history
     * @param startTime Start of period
     * @param endTime End of period
     * @returns Contribution records
     */
    getContributions(startTime?: number, endTime?: number): ContributionRecord[];
    /**
     * Get cost for operation type
     * @param operation Operation identifier
     * @param params Operation parameters
     * @returns Cost in credits
     */
    getCost(operation: OperationType, params?: Record<string, unknown>): number;
    /**
     * Spend credits for operation
     * @param operation Operation type
     * @param params Operation parameters
     * @returns Transaction record
     */
    spend(operation: OperationType, params?: Record<string, unknown>): Transaction;
    /**
     * Get pricing table
     * @returns Map of operations to base costs
     */
    getPricingTable(): Map<OperationType, number>;
    /**
     * Get economy-wide metrics
     * @returns Global economy metrics
     */
    getMetrics(): EconomyMetrics;
    /**
     * Get account analytics
     * @param period Time period
     * @returns Account analytics
     */
    getAnalytics(period?: 'day' | 'week' | 'month'): AccountAnalytics;
    /**
     * Get leaderboard
     * @param metric Ranking metric
     * @param limit Number of entries
     * @returns Leaderboard entries
     */
    getLeaderboard(metric: LeaderboardMetric, limit?: number): LeaderboardEntry[];
}
/** Transaction filter options */
export interface TransactionFilter {
    type?: Transaction['type'];
    startTime?: number;
    endTime?: number;
    minAmount?: number;
    maxAmount?: number;
    limit?: number;
    offset?: number;
}
/** Contribution type */
export type ContributionType = 'compute' | 'storage' | 'bandwidth' | 'validation' | 'training' | 'inference';
/** Contribution record */
export interface ContributionRecord {
    type: ContributionType;
    value: number;
    timestamp: number;
    rewardEarned: number;
}
/** Operation type for pricing */
export type OperationType = 'attention_scaled_dot' | 'attention_multi_head' | 'attention_flash' | 'attention_moe' | 'learning_lora' | 'learning_btsp' | 'nervous_step' | 'nervous_propagate' | 'exotic_quantum' | 'exotic_hyperbolic' | 'storage_read' | 'storage_write';
/** Account analytics */
export interface AccountAnalytics {
    period: string;
    totalSpent: number;
    totalEarned: number;
    netFlow: number;
    topOperations: {
        operation: OperationType;
        count: number;
        cost: number;
    }[];
    stakingYield: number;
    multiplierHistory: {
        time: number;
        value: number;
    }[];
}
/** Leaderboard metric */
export type LeaderboardMetric = 'total_staked' | 'contributions' | 'compute_usage' | 'rewards_earned';
/** Leaderboard entry */
export interface LeaderboardEntry {
    rank: number;
    accountId: string;
    value: number;
    change: number;
}
/**
 * Create an economy engine instance
 * @param config Optional configuration
 * @returns Initialized economy engine
 */
export declare function createEconomyEngine(config?: EconomyConfig): EconomyEngine;
/**
 * Calculate APY for staking
 * @param baseRate Base reward rate
 * @param compoundingFrequency Annual compounding frequency
 */
export declare function calculateStakingApy(baseRate: number, compoundingFrequency?: number): number;
/**
 * Format credit amount for display
 * @param amount Amount in credits
 * @param decimals Decimal places
 */
export declare function formatCredits(amount: number, decimals?: number): string;
//# sourceMappingURL=economy.d.ts.map