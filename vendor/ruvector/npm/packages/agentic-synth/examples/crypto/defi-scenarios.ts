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

import { createSynth } from '../../src/index.js';

/**
 * Example 1: Generate yield farming scenarios
 * Simulates various DeFi protocols and farming strategies
 */
async function generateYieldFarmingData() {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  const protocols = ['aave', 'compound', 'curve', 'convex', 'yearn'];
  const results = [];

  for (const protocol of protocols) {
    const result = await synth.generateTimeSeries({
      count: 720, // 30 days, hourly data
      interval: '1h',
      startDate: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000),
      metrics: ['apy', 'tvl', 'dailyRewards', 'userCount'],
      trend: protocol === 'aave' ? 'up' : 'stable',
      seasonality: true,
      noise: 0.1,
      schema: {
        timestamp: { type: 'datetime', format: 'iso8601' },
        protocol: { type: 'string', default: protocol },
        pool: { type: 'string', enum: ['USDC-USDT', 'ETH-USDC', 'WBTC-ETH', 'DAI-USDC'] },
        apy: { type: 'number', min: 0.1, max: 500 }, // 0.1% to 500%
        baseAPY: { type: 'number', min: 0.1, max: 50 },
        rewardAPY: { type: 'number', min: 0, max: 450 },
        tvl: { type: 'number', min: 1000000 },
        dailyRewards: { type: 'number', min: 0 },
        userCount: { type: 'integer', min: 100 },
        depositFee: { type: 'number', min: 0, max: 1 },
        withdrawalFee: { type: 'number', min: 0, max: 1 },
        performanceFee: { type: 'number', min: 0, max: 20 }
      },
      constraints: {
        custom: [
          'apy = baseAPY + rewardAPY',
          'dailyRewards proportional to tvl',
          'APY inversely related to TVL (dilution)',
          'Weekend APY typically lower due to reduced activity'
        ]
      }
    });

    results.push({ protocol, data: result.data });
    console.log(`Generated ${protocol} yield farming data: ${result.data.length} records`);
  }

  return results;
}

/**
 * Example 2: Simulate liquidity provision scenarios
 * Includes LP token calculations and position management
 */
async function generateLiquidityProvisionScenarios() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateStructured({
    count: 1000, // 1000 LP positions
    schema: {
      positionId: { type: 'string', format: 'uuid' },
      provider: { type: 'string', required: true },
      walletAddress: { type: 'string', format: 'ethereum_address' },
      dex: { type: 'string', enum: ['uniswap_v2', 'uniswap_v3', 'sushiswap', 'curve', 'balancer'] },
      pool: { type: 'string', enum: ['ETH-USDC', 'WBTC-ETH', 'DAI-USDC', 'stETH-ETH'] },
      entryDate: { type: 'datetime', required: true },
      exitDate: { type: 'datetime' },
      position: {
        type: 'object',
        required: true,
        properties: {
          tokenA: { type: 'string' },
          tokenB: { type: 'string' },
          amountA: { type: 'number', min: 0 },
          amountB: { type: 'number', min: 0 },
          shareOfPool: { type: 'number', min: 0, max: 100 },
          lpTokens: { type: 'number', min: 0 }
        }
      },
      entryPrice: {
        type: 'object',
        properties: {
          tokenA_USD: { type: 'number' },
          tokenB_USD: { type: 'number' },
          ratio: { type: 'number' }
        }
      },
      currentPrice: {
        type: 'object',
        properties: {
          tokenA_USD: { type: 'number' },
          tokenB_USD: { type: 'number' },
          ratio: { type: 'number' }
        }
      },
      returns: {
        type: 'object',
        properties: {
          feesEarned: { type: 'number', min: 0 },
          impermanentLoss: { type: 'number', min: -100, max: 0 },
          totalReturn: { type: 'number' },
          returnPercent: { type: 'number' },
          annualizedReturn: { type: 'number' }
        }
      },
      riskMetrics: {
        type: 'object',
        properties: {
          volatility: { type: 'number', min: 0, max: 200 },
          maxDrawdown: { type: 'number', min: 0, max: 100 },
          sharpeRatio: { type: 'number' }
        }
      }
    },
    constraints: {
      custom: [
        'exitDate >= entryDate or null',
        'impermanentLoss calculated based on price divergence',
        'totalReturn = feesEarned + impermanentLoss',
        'feesEarned based on volume and pool share',
        'Uniswap V3 positions can have concentrated liquidity ranges'
      ]
    }
  });

  console.log('Generated LP scenarios:', result.data.length);

  // Analyze returns
  const avgIL = result.data.reduce((sum: number, pos: any) =>
    sum + pos.returns.impermanentLoss, 0) / result.data.length;
  const avgFees = result.data.reduce((sum: number, pos: any) =>
    sum + pos.returns.feesEarned, 0) / result.data.length;

  console.log(`Average Impermanent Loss: ${avgIL.toFixed(2)}%`);
  console.log(`Average Fees Earned: $${avgFees.toFixed(2)}`);

  return result;
}

/**
 * Example 3: Generate impermanent loss scenarios
 * Detailed analysis of IL under different market conditions
 */
async function generateImpermanentLossScenarios() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateTimeSeries({
    count: 1000,
    interval: '1h',
    startDate: new Date(Date.now() - 42 * 24 * 60 * 60 * 1000), // 42 days
    metrics: ['priceRatio', 'impermanentLoss', 'hodlValue', 'lpValue', 'feesEarned'],
    trend: 'volatile',
    seasonality: false,
    noise: 0.2,
    schema: {
      timestamp: { type: 'datetime', format: 'iso8601' },
      scenario: { type: 'string', enum: ['bull_market', 'bear_market', 'sideways', 'high_volatility'] },
      initialRatio: { type: 'number', default: 1 },
      priceRatio: { type: 'number', min: 0.1, max: 10 },
      priceChange: { type: 'number' },
      impermanentLoss: { type: 'number', min: -100, max: 0 },
      impermanentLossPercent: { type: 'number' },
      hodlValue: { type: 'number', min: 0 },
      lpValue: { type: 'number', min: 0 },
      feesEarned: { type: 'number', min: 0 },
      netProfit: { type: 'number' },
      breakEvenFeeRate: { type: 'number' },
      recommendation: {
        type: 'string',
        enum: ['stay', 'exit', 'rebalance', 'wait']
      }
    },
    constraints: {
      custom: [
        'IL formula: 2 * sqrt(priceRatio) / (1 + priceRatio) - 1',
        'lpValue = hodlValue + impermanentLoss + feesEarned',
        'netProfit = lpValue - hodlValue',
        'Higher price divergence = higher IL',
        'Fees can compensate for IL in high-volume pools'
      ]
    }
  });

  console.log('Generated impermanent loss scenarios:', result.data.length);

  // Find worst IL scenario
  const worstIL = result.data.reduce((worst: any, current: any) =>
    current.impermanentLoss < worst.impermanentLoss ? current : worst
  );
  console.log('Worst IL scenario:', {
    timestamp: worstIL.timestamp,
    priceRatio: worstIL.priceRatio,
    IL: `${worstIL.impermanentLossPercent}%`
  });

  return result;
}

/**
 * Example 4: Generate gas price data and optimization scenarios
 * Critical for DeFi transaction cost analysis
 */
async function generateGasPriceData() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateTimeSeries({
    count: 10080, // 1 week, minute-by-minute
    interval: '1m',
    startDate: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000),
    metrics: ['baseFee', 'priorityFee', 'maxFee', 'gasUsed'],
    trend: 'stable',
    seasonality: true, // Network congestion patterns
    noise: 0.25, // High volatility in gas prices
    schema: {
      timestamp: { type: 'datetime', format: 'iso8601' },
      network: { type: 'string', enum: ['ethereum', 'polygon', 'arbitrum', 'optimism', 'base'] },
      block: { type: 'integer', min: 18000000 },
      baseFee: { type: 'number', min: 1, max: 500 }, // Gwei
      priorityFee: { type: 'number', min: 0.1, max: 100 },
      maxFee: { type: 'number', min: 1, max: 600 },
      gasUsed: { type: 'integer', min: 21000, max: 30000000 },
      blockUtilization: { type: 'number', min: 0, max: 100 },
      pendingTxCount: { type: 'integer', min: 0 },
      congestionLevel: {
        type: 'string',
        enum: ['low', 'medium', 'high', 'extreme']
      },
      estimatedCost: {
        type: 'object',
        properties: {
          transfer: { type: 'number' }, // Simple ETH transfer
          swap: { type: 'number' },     // DEX swap
          addLiquidity: { type: 'number' },
          removeLiquidity: { type: 'number' },
          complexDeFi: { type: 'number' } // Multi-protocol interaction
        }
      }
    },
    constraints: {
      custom: [
        'maxFee >= baseFee + priorityFee',
        'Higher baseFee during peak hours (EST afternoon)',
        'Weekend gas typically 20-30% lower',
        'NFT mint events cause temporary spikes',
        'L2 gas prices 10-100x cheaper than mainnet'
      ]
    }
  });

  console.log('Generated gas price data:', result.data.length);

  // Analyze gas patterns
  const avgGas = result.data.reduce((sum: number, d: any) =>
    sum + d.baseFee, 0) / result.data.length;
  const maxGas = Math.max(...result.data.map((d: any) => d.baseFee));
  const minGas = Math.min(...result.data.map((d: any) => d.baseFee));

  console.log('Gas statistics (Gwei):');
  console.log(`  Average: ${avgGas.toFixed(2)}`);
  console.log(`  Max: ${maxGas.toFixed(2)}`);
  console.log(`  Min: ${minGas.toFixed(2)}`);

  return result;
}

/**
 * Example 5: Generate smart contract interaction patterns
 * Simulates complex DeFi transaction sequences
 */
async function generateSmartContractInteractions() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateEvents({
    count: 5000,
    eventTypes: [
      'approve',
      'swap',
      'addLiquidity',
      'removeLiquidity',
      'stake',
      'unstake',
      'claim',
      'compound',
      'flashloan',
      'arbitrage'
    ],
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 24 * 60 * 60 * 1000),
      end: new Date()
    },
    userCount: 2000,
    schema: {
      txHash: { type: 'string', format: 'transaction_hash' },
      timestamp: { type: 'datetime', format: 'iso8601' },
      walletAddress: { type: 'string', format: 'ethereum_address' },
      contractAddress: { type: 'string', format: 'ethereum_address' },
      protocol: {
        type: 'string',
        enum: ['uniswap', 'aave', 'compound', 'curve', 'yearn', 'maker', 'lido']
      },
      function: { type: 'string', required: true },
      gasUsed: { type: 'integer', min: 21000 },
      gasPrice: { type: 'number', min: 1 },
      txCost: { type: 'number', min: 0 },
      value: { type: 'number', min: 0 },
      status: { type: 'string', enum: ['success', 'failed', 'pending'] },
      failureReason: { type: 'string' },
      interactionSequence: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            step: { type: 'integer' },
            action: { type: 'string' },
            contract: { type: 'string' }
          }
        }
      },
      internalTxs: { type: 'integer', min: 0, max: 50 },
      tokensTransferred: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            token: { type: 'string' },
            amount: { type: 'number' },
            from: { type: 'string' },
            to: { type: 'string' }
          }
        }
      }
    }
  });

  console.log('Generated smart contract interactions:', result.data.length);

  // Analyze by protocol
  const protocols: any = {};
  result.data.forEach((tx: any) => {
    protocols[tx.protocol] = (protocols[tx.protocol] || 0) + 1;
  });
  console.log('Interactions by protocol:', protocols);

  // Failure rate
  const failures = result.data.filter((tx: any) => tx.status === 'failed').length;
  console.log(`Failure rate: ${(failures / result.data.length * 100).toFixed(2)}%`);

  return result;
}

/**
 * Example 6: Generate lending/borrowing scenarios
 * Simulates DeFi lending protocols like Aave and Compound
 */
async function generateLendingScenarios() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateStructured({
    count: 500,
    schema: {
      positionId: { type: 'string', format: 'uuid' },
      walletAddress: { type: 'string', format: 'ethereum_address' },
      protocol: { type: 'string', enum: ['aave_v3', 'compound', 'maker', 'euler'] },
      positionType: { type: 'string', enum: ['lender', 'borrower', 'both'] },
      openedAt: { type: 'datetime', required: true },
      collateral: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            asset: { type: 'string', enum: ['ETH', 'WBTC', 'USDC', 'DAI', 'stETH'] },
            amount: { type: 'number', min: 0 },
            valueUSD: { type: 'number', min: 0 },
            collateralFactor: { type: 'number', min: 0, max: 0.95 }
          }
        }
      },
      borrowed: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            asset: { type: 'string' },
            amount: { type: 'number', min: 0 },
            valueUSD: { type: 'number', min: 0 },
            interestRate: { type: 'number', min: 0, max: 50 },
            rateMode: { type: 'string', enum: ['stable', 'variable'] }
          }
        }
      },
      healthFactor: { type: 'number', min: 0, max: 5 },
      ltv: { type: 'number', min: 0, max: 100 }, // Loan-to-Value
      liquidationThreshold: { type: 'number', min: 0, max: 100 },
      liquidationPrice: { type: 'number' },
      netAPY: { type: 'number' },
      totalInterestPaid: { type: 'number', min: 0 },
      totalInterestEarned: { type: 'number', min: 0 },
      riskLevel: { type: 'string', enum: ['safe', 'moderate', 'risky', 'critical'] },
      autoLiquidate: { type: 'boolean' }
    },
    constraints: {
      custom: [
        'healthFactor = totalCollateral / totalBorrowed',
        'healthFactor < 1.0 = liquidation risk',
        'LTV = totalBorrowed / totalCollateral * 100',
        'liquidationPrice based on collateral type',
        'Higher LTV = higher risk = higher potential liquidation'
      ]
    }
  });

  console.log('Generated lending scenarios:', result.data.length);

  // Risk analysis
  const riskLevels: any = {};
  result.data.forEach((pos: any) => {
    riskLevels[pos.riskLevel] = (riskLevels[pos.riskLevel] || 0) + 1;
  });
  console.log('Positions by risk level:', riskLevels);

  const atRisk = result.data.filter((pos: any) => pos.healthFactor < 1.2).length;
  console.log(`Positions at liquidation risk (HF < 1.2): ${atRisk}`);

  return result;
}

/**
 * Example 7: Generate staking rewards data
 * Covers liquid staking and validator rewards
 */
async function generateStakingRewards() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateTimeSeries({
    count: 365, // 1 year, daily
    interval: '1d',
    startDate: new Date(Date.now() - 365 * 24 * 60 * 60 * 1000),
    metrics: ['stakingAPR', 'totalStaked', 'dailyRewards', 'validatorCount'],
    trend: 'stable',
    seasonality: false,
    noise: 0.05,
    schema: {
      timestamp: { type: 'datetime', format: 'iso8601' },
      protocol: {
        type: 'string',
        enum: ['lido', 'rocket_pool', 'frax', 'stakewise', 'native_eth']
      },
      stakingAPR: { type: 'number', min: 2, max: 20 },
      baseAPR: { type: 'number', min: 2, max: 8 },
      bonusAPR: { type: 'number', min: 0, max: 12 },
      totalStaked: { type: 'number', min: 1000000 },
      totalStakedETH: { type: 'number', min: 10000 },
      dailyRewards: { type: 'number', min: 0 },
      validatorCount: { type: 'integer', min: 100 },
      averageValidatorBalance: { type: 'number', default: 32 },
      networkParticipation: { type: 'number', min: 0, max: 100 },
      slashingEvents: { type: 'integer', min: 0, max: 5 },
      fees: {
        type: 'object',
        properties: {
          protocolFee: { type: 'number', min: 0, max: 10 },
          nodeOperatorFee: { type: 'number', min: 0, max: 15 }
        }
      }
    }
  });

  console.log('Generated staking rewards data:', result.data.length);
  console.log('Average APR:',
    (result.data.reduce((sum: number, d: any) => sum + d.stakingAPR, 0) / result.data.length).toFixed(2) + '%'
  );

  return result;
}

/**
 * Example 8: Generate MEV (Maximal Extractable Value) scenarios
 * Advanced DeFi strategy simulations
 */
async function generateMEVScenarios() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateEvents({
    count: 1000,
    eventTypes: ['sandwich', 'arbitrage', 'liquidation', 'jit_liquidity', 'nft_snipe'],
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000),
      end: new Date()
    },
    userCount: 200, // MEV bots
    schema: {
      txHash: { type: 'string', format: 'transaction_hash' },
      blockNumber: { type: 'integer', min: 18000000 },
      timestamp: { type: 'datetime', format: 'iso8601' },
      mevType: {
        type: 'string',
        enum: ['sandwich', 'arbitrage', 'liquidation', 'jit_liquidity', 'backrun']
      },
      botAddress: { type: 'string', format: 'ethereum_address' },
      targetTx: { type: 'string', format: 'transaction_hash' },
      profit: { type: 'number', min: -1000, max: 100000 },
      profitUSD: { type: 'number' },
      gasCost: { type: 'number', min: 0 },
      netProfit: { type: 'number' },
      roi: { type: 'number' },
      complexity: { type: 'integer', min: 1, max: 10 },
      involvedProtocols: {
        type: 'array',
        items: { type: 'string' }
      },
      flashloanUsed: { type: 'boolean' },
      flashloanAmount: { type: 'number' },
      executionTime: { type: 'number', min: 1, max: 1000 } // ms
    }
  });

  console.log('Generated MEV scenarios:', result.data.length);

  const totalProfit = result.data.reduce((sum: number, mev: any) => sum + mev.netProfit, 0);
  const profitableOps = result.data.filter((mev: any) => mev.netProfit > 0).length;

  console.log(`Total MEV extracted: $${totalProfit.toFixed(2)}`);
  console.log(`Profitable operations: ${profitableOps}/${result.data.length} (${(profitableOps/result.data.length*100).toFixed(1)}%)`);

  return result;
}

/**
 * Run all DeFi scenario examples
 */
export async function runDeFiScenarioExamples() {
  console.log('=== DeFi Protocol Simulation Examples ===\n');

  console.log('Example 1: Yield Farming Data');
  await generateYieldFarmingData();
  console.log('\n---\n');

  console.log('Example 2: Liquidity Provision Scenarios');
  await generateLiquidityProvisionScenarios();
  console.log('\n---\n');

  console.log('Example 3: Impermanent Loss Scenarios');
  await generateImpermanentLossScenarios();
  console.log('\n---\n');

  console.log('Example 4: Gas Price Data');
  await generateGasPriceData();
  console.log('\n---\n');

  console.log('Example 5: Smart Contract Interactions');
  await generateSmartContractInteractions();
  console.log('\n---\n');

  console.log('Example 6: Lending/Borrowing Scenarios');
  await generateLendingScenarios();
  console.log('\n---\n');

  console.log('Example 7: Staking Rewards');
  await generateStakingRewards();
  console.log('\n---\n');

  console.log('Example 8: MEV Scenarios');
  await generateMEVScenarios();
}

// Export individual examples
export {
  generateYieldFarmingData,
  generateLiquidityProvisionScenarios,
  generateImpermanentLossScenarios,
  generateGasPriceData,
  generateSmartContractInteractions,
  generateLendingScenarios,
  generateStakingRewards,
  generateMEVScenarios
};

// Uncomment to run
// runDeFiScenarioExamples().catch(console.error);
