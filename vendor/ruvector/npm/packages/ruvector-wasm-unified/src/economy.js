"use strict";
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
Object.defineProperty(exports, "__esModule", { value: true });
exports.createEconomyEngine = createEconomyEngine;
exports.calculateStakingApy = calculateStakingApy;
exports.formatCredits = formatCredits;
// ============================================================================
// Factory and Utilities
// ============================================================================
/**
 * Create an economy engine instance
 * @param config Optional configuration
 * @returns Initialized economy engine
 */
function createEconomyEngine(config) {
    const defaultConfig = {
        initialBalance: 1000,
        stakingEnabled: true,
        rewardRate: 0.05,
        ...config,
    };
    // Internal state
    let balance = defaultConfig.initialBalance;
    let stakedAmount = 0;
    let contributionMultiplier = 1.0;
    const transactions = [];
    const stakingPositions = [];
    const contributions = [];
    let pendingRewards = 0;
    let transactionIdCounter = 0;
    // Pricing table
    const pricingTable = new Map([
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
    function createTransaction(type, amount, metadata) {
        const tx = {
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
            const position = {
                amount,
                lockDuration,
                startTime: Date.now(),
                expectedReward: amount * defaultConfig.rewardRate * (lockDuration / (86400 * 365)),
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
            return amount * defaultConfig.rewardRate * (duration / (86400 * 365));
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
                result = result.filter(t => t.timestamp >= options.startTime);
            }
            if (options?.endTime) {
                result = result.filter(t => t.timestamp <= options.endTime);
            }
            if (options?.minAmount) {
                result = result.filter(t => t.amount >= options.minAmount);
            }
            if (options?.maxAmount) {
                result = result.filter(t => t.amount <= options.maxAmount);
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
                stakingYield: defaultConfig.rewardRate * 100,
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
function calculateStakingApy(baseRate, compoundingFrequency = 365) {
    return Math.pow(1 + baseRate / compoundingFrequency, compoundingFrequency) - 1;
}
/**
 * Format credit amount for display
 * @param amount Amount in credits
 * @param decimals Decimal places
 */
function formatCredits(amount, decimals = 4) {
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
//# sourceMappingURL=economy.js.map