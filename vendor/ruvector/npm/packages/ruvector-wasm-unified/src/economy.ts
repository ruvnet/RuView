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

import type {
  CreditAccount,
  Transaction,
  StakingPosition,
  EconomyMetrics,
  EconomyConfig,
} from './types';

// ============================================================================
// Economy Engine Interface
// ============================================================================

/**
 * Core economy engine for compute credit management
 */
export interface EconomyEngine {
  // -------------------------------------------------------------------------
  // Account Management
  // -------------------------------------------------------------------------

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

  // -------------------------------------------------------------------------
  // Staking Operations
  // -------------------------------------------------------------------------

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

  // -------------------------------------------------------------------------
  // Transactions
  // -------------------------------------------------------------------------

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

  // -------------------------------------------------------------------------
  // Rewards & Penalties
  // -------------------------------------------------------------------------

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

  // -------------------------------------------------------------------------
  // Pricing & Costs
  // -------------------------------------------------------------------------

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

  // -------------------------------------------------------------------------
  // Metrics & Analytics
  // -------------------------------------------------------------------------

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

// ============================================================================
// Supporting Types
// ============================================================================

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
export type ContributionType =
  | 'compute'
  | 'storage'
  | 'bandwidth'
  | 'validation'
  | 'training'
  | 'inference';

/** Contribution record */
export interface ContributionRecord {
  type: ContributionType;
  value: number;
  timestamp: number;
  rewardEarned: number;
}

/** Operation type for pricing */
export type OperationType =
  | 'attention_scaled_dot'
  | 'attention_multi_head'
  | 'attention_flash'
  | 'attention_moe'
  | 'learning_lora'
  | 'learning_btsp'
  | 'nervous_step'
  | 'nervous_propagate'
  | 'exotic_quantum'
  | 'exotic_hyperbolic'
  | 'storage_read'
  | 'storage_write';

/** Account analytics */
export interface AccountAnalytics {
  period: string;
  totalSpent: number;
  totalEarned: number;
  netFlow: number;
  topOperations: { operation: OperationType; count: number; cost: number }[];
  stakingYield: number;
  multiplierHistory: { time: number; value: number }[];
}

/** Leaderboard metric */
export type LeaderboardMetric =
  | 'total_staked'
  | 'contributions'
  | 'compute_usage'
  | 'rewards_earned';

/** Leaderboard entry */
export interface LeaderboardEntry {
  rank: number;
  accountId: string;
  value: number;
  change: number;
}

// ============================================================================
// Factory and Utilities
// ============================================================================

/**
 * Create an economy engine instance
 * @param config Optional configuration
 * @returns Initialized economy engine
 */
export function createEconomyEngine(config?: EconomyConfig): EconomyEngine {
  const defaultConfig: EconomyConfig = {
    initialBalance: 1000,
    stakingEnabled: true,
    rewardRate: 0.05,
    ...config,
  };

  // Internal state
  let balance = defaultConfig.initialBalance!;
  let stakedAmount = 0;
  let contributionMultiplier = 1.0;
  const transactions: Transaction[] = [];
  const stakingPositions: StakingPosition[] = [];
  const contributions: ContributionRecord[] = [];
  let pendingRewards = 0;
  let transactionIdCounter = 0;

  // Pricing table
  const pricingTable = new Map<OperationType, number>([
    ['attention_scaled_dot', 0.001],
    ['attention_multi_head', 0.005],
    ['attention_flash', 0.003],
    ['attention_moe', 0.01],
    ['learning_lora', 0.02],
    ['learning_btsp', 0.005],
    ['nervous_step', 0.0001],
    ['nervous_propagate', 0.001],
    ['exotic_quantum', 0.05],
    ['exotic_hyperbolic', 0.02],
    ['storage_read', 0.0001],
    ['storage_write', 0.0005],
  ]);

  function createTransaction(
    type: Transaction['type'],
    amount: number,
    metadata?: Record<string, unknown>
  ): Transaction {
    const tx: Transaction = {
      id: `tx_${transactionIdCounter++}`,
      type,
      amount,
      timestamp: Date.now(),
      metadata,
    };
    transactions.push(tx);
    return tx;
  }

  return {
    creditBalance: () => balance,
    contributionMultiplier: () => contributionMultiplier,
    getAccount: () => ({
      balance,
      stakedAmount,
      contributionMultiplier,
      lastUpdate: Date.now(),
    }),
    canAfford: (cost) => balance >= cost,
    stakeDeposit: (amount, lockDuration = 86400 * 30) => {
      if (amount > balance) {
        throw new Error('Insufficient balance for staking');
      }
      balance -= amount;
      stakedAmount += amount;
      const position: StakingPosition = {
        amount,
        lockDuration,
        startTime: Date.now(),
        expectedReward: amount * defaultConfig.rewardRate! * (lockDuration / (86400 * 365)),
      };
      stakingPositions.push(position);
      createTransaction('stake', amount);
      // Update multiplier based on staking
      contributionMultiplier = 1.0 + Math.log10(1 + stakedAmount / 1000) * 0.5;
      return position;
    },
    stakeWithdraw: (amount) => {
      if (amount > stakedAmount) {
        throw new Error('Insufficient staked amount');
      }
      stakedAmount -= amount;
      balance += amount;
      createTransaction('unstake', amount);
      contributionMultiplier = 1.0 + Math.log10(1 + stakedAmount / 1000) * 0.5;
      return amount;
    },
    getStakingPositions: () => [...stakingPositions],
    getTotalStaked: () => stakedAmount,
    estimateStakingReward: (amount, duration) => {
      return amount * defaultConfig.rewardRate! * (duration / (86400 * 365));
    },
    transfer: (targetId, amount) => {
      if (amount > balance) {
        throw new Error('Insufficient balance for transfer');
      }
      balance -= amount;
      return createTransaction('withdraw', amount, { targetId });
    },
    deposit: (amount, source) => {
      balance += amount;
      return createTransaction('deposit', amount, { source });
    },
    withdraw: (amount, destination) => {
      if (amount > balance) {
        throw new Error('Insufficient balance for withdrawal');
      }
      balance -= amount;
      return createTransaction('withdraw', amount, { destination });
    },
    getTransactionHistory: (options) => {
      let result = [...transactions];
      if (options?.type) {
        result = result.filter(t => t.type === options.type);
      }
      if (options?.startTime) {
        result = result.filter(t => t.timestamp >= options.startTime!);
      }
      if (options?.endTime) {
        result = result.filter(t => t.timestamp <= options.endTime!);
      }
      if (options?.minAmount) {
        result = result.filter(t => t.amount >= options.minAmount!);
      }
      if (options?.maxAmount) {
        result = result.filter(t => t.amount <= options.maxAmount!);
      }
      if (options?.offset) {
        result = result.slice(options.offset);
      }
      if (options?.limit) {
        result = result.slice(0, options.limit);
      }
      return result;
    },
    getTransaction: (transactionId) => {
      return transactions.find(t => t.id === transactionId);
    },
    claimRewards: () => {
      const claimed = pendingRewards;
      balance += claimed;
      pendingRewards = 0;
      if (claimed > 0) {
        createTransaction('reward', claimed);
      }
      return claimed;
    },
    getPendingRewards: () => pendingRewards,
    recordContribution: (contributionType, value) => {
      const reward = value * 0.1 * contributionMultiplier;
      contributions.push({
        type: contributionType,
        value,
        timestamp: Date.now(),
        rewardEarned: reward,
      });
      pendingRewards += reward;
    },
    getContributions: (startTime, endTime) => {
      let result = [...contributions];
      if (startTime) {
        result = result.filter(c => c.timestamp >= startTime);
      }
      if (endTime) {
        result = result.filter(c => c.timestamp <= endTime);
      }
      return result;
    },
    getCost: (operation, params) => {
      const baseCost = pricingTable.get(operation) ?? 0;
      // Apply multiplier discount
      return baseCost / contributionMultiplier;
    },
    spend: (operation, params) => {
      const cost = pricingTable.get(operation) ?? 0;
      const adjustedCost = cost / contributionMultiplier;
      if (adjustedCost > balance) {
        throw new Error(`Insufficient balance for ${operation}`);
      }
      balance -= adjustedCost;
      return createTransaction('withdraw', adjustedCost, { operation, params });
    },
    getPricingTable: () => new Map(pricingTable),
    getMetrics: () => ({
      totalSupply: 1000000,
      totalStaked: stakedAmount,
      circulatingSupply: 1000000 - stakedAmount,
      averageMultiplier: contributionMultiplier,
    }),
    getAnalytics: (period = 'week') => {
      const periodMs = {
        day: 86400000,
        week: 604800000,
        month: 2592000000,
      }[period];
      const startTime = Date.now() - periodMs;
      const periodTx = transactions.filter(t => t.timestamp >= startTime);
      const spent = periodTx
        .filter(t => t.type === 'withdraw')
        .reduce((sum, t) => sum + t.amount, 0);
      const earned = periodTx
        .filter(t => t.type === 'deposit' || t.type === 'reward')
        .reduce((sum, t) => sum + t.amount, 0);
      return {
        period,
        totalSpent: spent,
        totalEarned: earned,
        netFlow: earned - spent,
        topOperations: [],
        stakingYield: defaultConfig.rewardRate! * 100,
        multiplierHistory: [],
      };
    },
    getLeaderboard: (metric, limit = 10) => {
      // Placeholder - would connect to global state
      return [];
    },
  };
}

/**
 * Calculate APY for staking
 * @param baseRate Base reward rate
 * @param compoundingFrequency Annual compounding frequency
 */
export function calculateStakingApy(
  baseRate: number,
  compoundingFrequency: number = 365
): number {
  return Math.pow(1 + baseRate / compoundingFrequency, compoundingFrequency) - 1;
}

/**
 * Format credit amount for display
 * @param amount Amount in credits
 * @param decimals Decimal places
 */
export function formatCredits(amount: number, decimals: number = 4): string {
  if (amount >= 1e9) {
    return `${(amount / 1e9).toFixed(2)}B`;
  }
  if (amount >= 1e6) {
    return `${(amount / 1e6).toFixed(2)}M`;
  }
  if (amount >= 1e3) {
    return `${(amount / 1e3).toFixed(2)}K`;
  }
  return amount.toFixed(decimals);
}
