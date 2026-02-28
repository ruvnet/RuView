/**
 * Blockchain and On-Chain Data Generation
 *
 * Examples for generating realistic blockchain data including:
 * - Transaction patterns and behaviors
 * - Wallet activity simulation
 * - Token transfer events
 * - NFT trading activity
 * - MEV (Maximal Extractable Value) scenarios
 */

import { createSynth } from '../../src/index.js';

/**
 * Example 1: Generate realistic transaction patterns
 * Simulates various transaction types across different networks
 */
async function generateTransactionPatterns() {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  const networks = ['ethereum', 'polygon', 'arbitrum', 'optimism', 'base'];
  const results = [];

  for (const network of networks) {
    const result = await synth.generateEvents({
      count: 10000,
      eventTypes: [
        'transfer',
        'contract_call',
        'contract_creation',
        'erc20_transfer',
        'erc721_transfer',
        'erc1155_transfer'
      ],
      distribution: 'poisson',
      timeRange: {
        start: new Date(Date.now() - 24 * 60 * 60 * 1000),
        end: new Date()
      },
      userCount: 5000,
      schema: {
        txHash: { type: 'string', format: 'transaction_hash' },
        blockNumber: { type: 'integer', min: 1000000 },
        blockTimestamp: { type: 'datetime', format: 'iso8601' },
        network: { type: 'string', default: network },
        from: { type: 'string', format: 'ethereum_address' },
        to: { type: 'string', format: 'ethereum_address' },
        value: { type: 'number', min: 0 },
        gasLimit: { type: 'integer', min: 21000, max: 30000000 },
        gasUsed: { type: 'integer', min: 21000 },
        gasPrice: { type: 'number', min: 1 },
        maxFeePerGas: { type: 'number' },
        maxPriorityFeePerGas: { type: 'number' },
        nonce: { type: 'integer', min: 0 },
        transactionIndex: { type: 'integer', min: 0, max: 300 },
        status: { type: 'string', enum: ['success', 'failed'] },
        type: { type: 'integer', enum: [0, 1, 2] }, // Legacy, EIP-2930, EIP-1559
        input: { type: 'string' },
        methodId: { type: 'string' },
        internalTransactions: { type: 'integer', min: 0, max: 100 }
      }
    });

    results.push({ network, data: result.data });
    console.log(`Generated ${network} transactions: ${result.data.length}`);
  }

  return results;
}

/**
 * Example 2: Simulate wallet behavior patterns
 * Includes HODLers, traders, bots, and contract wallets
 */
async function generateWalletBehavior() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const walletTypes = ['hodler', 'trader', 'bot', 'whale', 'retail', 'contract'];

  const result = await synth.generateStructured({
    count: 5000,
    schema: {
      walletAddress: { type: 'string', format: 'ethereum_address', required: true },
      walletType: { type: 'string', enum: walletTypes },
      createdAt: { type: 'datetime', required: true },
      lastActive: { type: 'datetime' },
      balance: {
        type: 'object',
        properties: {
          ETH: { type: 'number', min: 0 },
          USDC: { type: 'number', min: 0 },
          USDT: { type: 'number', min: 0 },
          totalValueUSD: { type: 'number', min: 0 }
        }
      },
      activity: {
        type: 'object',
        properties: {
          totalTxs: { type: 'integer', min: 0 },
          txsLast24h: { type: 'integer', min: 0 },
          txsLast7d: { type: 'integer', min: 0 },
          txsLast30d: { type: 'integer', min: 0 },
          avgTxValue: { type: 'number', min: 0 },
          avgTxFrequency: { type: 'number', min: 0 } // per day
        }
      },
      holdings: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            token: { type: 'string' },
            symbol: { type: 'string' },
            balance: { type: 'number' },
            valueUSD: { type: 'number' },
            allocation: { type: 'number', min: 0, max: 100 }
          }
        }
      },
      defi: {
        type: 'object',
        properties: {
          totalValueLocked: { type: 'number', min: 0 },
          activePools: { type: 'integer', min: 0 },
          lendingPositions: { type: 'integer', min: 0 },
          borrowingPositions: { type: 'integer', min: 0 },
          nftCount: { type: 'integer', min: 0 }
        }
      },
      behaviorMetrics: {
        type: 'object',
        properties: {
          riskProfile: { type: 'string', enum: ['conservative', 'moderate', 'aggressive', 'degen'] },
          avgHoldingPeriod: { type: 'number', min: 0 }, // days
          tradingFrequency: { type: 'string', enum: ['daily', 'weekly', 'monthly', 'rarely'] },
          gasTotalSpent: { type: 'number', min: 0 },
          profitLoss: { type: 'number' },
          winRate: { type: 'number', min: 0, max: 100 }
        }
      },
      labels: {
        type: 'array',
        items: {
          type: 'string',
          enum: ['whale', 'early_adopter', 'nft_collector', 'yield_farmer', 'mev_bot', 'exchange', 'bridge']
        }
      }
    }
  });

  console.log('Generated wallet behavior data:', result.data.length);

  // Analyze by wallet type
  const typeDistribution: any = {};
  result.data.forEach((wallet: any) => {
    typeDistribution[wallet.walletType] = (typeDistribution[wallet.walletType] || 0) + 1;
  });
  console.log('Wallet type distribution:', typeDistribution);

  return result;
}

/**
 * Example 3: Generate token transfer events
 * Simulates ERC-20, ERC-721, and ERC-1155 transfers
 */
async function generateTokenTransfers() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateEvents({
    count: 50000,
    eventTypes: ['erc20', 'erc721', 'erc1155'],
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 24 * 60 * 60 * 1000),
      end: new Date()
    },
    userCount: 10000,
    schema: {
      txHash: { type: 'string', format: 'transaction_hash' },
      blockNumber: { type: 'integer', min: 18000000 },
      timestamp: { type: 'datetime', format: 'iso8601' },
      tokenStandard: { type: 'string', enum: ['erc20', 'erc721', 'erc1155'] },
      contractAddress: { type: 'string', format: 'ethereum_address' },
      tokenName: { type: 'string', required: true },
      tokenSymbol: { type: 'string', required: true },
      from: { type: 'string', format: 'ethereum_address' },
      to: { type: 'string', format: 'ethereum_address' },
      value: { type: 'string' }, // Large numbers as string
      valueDecimal: { type: 'number' },
      valueUSD: { type: 'number', min: 0 },
      tokenId: { type: 'string' }, // For NFTs
      amount: { type: 'integer' }, // For ERC-1155
      logIndex: { type: 'integer', min: 0 },
      metadata: {
        type: 'object',
        properties: {
          decimals: { type: 'integer', default: 18 },
          totalSupply: { type: 'string' },
          holderCount: { type: 'integer' },
          marketCap: { type: 'number' }
        }
      }
    }
  });

  console.log('Generated token transfers:', result.data.length);

  // Analyze by token standard
  const standards: any = {};
  result.data.forEach((transfer: any) => {
    standards[transfer.tokenStandard] = (standards[transfer.tokenStandard] || 0) + 1;
  });
  console.log('Transfers by standard:', standards);

  return result;
}

/**
 * Example 4: Generate NFT trading activity
 * Includes mints, sales, and marketplace activity
 */
async function generateNFTActivity() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateEvents({
    count: 5000,
    eventTypes: ['mint', 'sale', 'transfer', 'listing', 'bid', 'offer_accepted'],
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000),
      end: new Date()
    },
    userCount: 3000,
    schema: {
      txHash: { type: 'string', format: 'transaction_hash' },
      timestamp: { type: 'datetime', format: 'iso8601' },
      eventType: {
        type: 'string',
        enum: ['mint', 'sale', 'transfer', 'listing', 'bid', 'offer_accepted']
      },
      marketplace: {
        type: 'string',
        enum: ['opensea', 'blur', 'looksrare', 'x2y2', 'rarible', 'foundation']
      },
      collection: {
        type: 'object',
        properties: {
          address: { type: 'string', format: 'ethereum_address' },
          name: { type: 'string' },
          symbol: { type: 'string' },
          floorPrice: { type: 'number', min: 0 },
          totalVolume: { type: 'number', min: 0 },
          itemCount: { type: 'integer', min: 1 }
        }
      },
      nft: {
        type: 'object',
        properties: {
          tokenId: { type: 'string' },
          name: { type: 'string' },
          rarity: { type: 'string', enum: ['common', 'uncommon', 'rare', 'epic', 'legendary'] },
          rarityRank: { type: 'integer', min: 1 },
          traits: { type: 'array' }
        }
      },
      price: {
        type: 'object',
        properties: {
          amount: { type: 'number', min: 0 },
          currency: { type: 'string', enum: ['ETH', 'WETH', 'USDC', 'APE'] },
          usdValue: { type: 'number', min: 0 }
        }
      },
      from: { type: 'string', format: 'ethereum_address' },
      to: { type: 'string', format: 'ethereum_address' },
      royalty: {
        type: 'object',
        properties: {
          recipient: { type: 'string', format: 'ethereum_address' },
          amount: { type: 'number', min: 0 },
          percentage: { type: 'number', min: 0, max: 10 }
        }
      },
      marketplaceFee: { type: 'number', min: 0 }
    }
  });

  console.log('Generated NFT activity:', result.data.length);

  // Analyze by event type
  const events: any = {};
  result.data.forEach((activity: any) => {
    events[activity.eventType] = (events[activity.eventType] || 0) + 1;
  });
  console.log('Activity by type:', events);

  // Top marketplaces by volume
  const marketplaces: any = {};
  result.data.forEach((activity: any) => {
    if (activity.price) {
      marketplaces[activity.marketplace] =
        (marketplaces[activity.marketplace] || 0) + activity.price.usdValue;
    }
  });
  console.log('Volume by marketplace:', marketplaces);

  return result;
}

/**
 * Example 5: Generate MEV transaction patterns
 * Advanced MEV extraction and sandwich attack simulations
 */
async function generateMEVPatterns() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateStructured({
    count: 1000,
    schema: {
      bundleId: { type: 'string', format: 'uuid' },
      blockNumber: { type: 'integer', min: 18000000 },
      timestamp: { type: 'datetime', required: true },
      mevType: {
        type: 'string',
        enum: ['sandwich', 'arbitrage', 'liquidation', 'jit', 'backrun', 'frontrun']
      },
      bundleTransactions: {
        type: 'array',
        minItems: 1,
        maxItems: 10,
        items: {
          type: 'object',
          properties: {
            txHash: { type: 'string' },
            position: { type: 'integer' },
            type: { type: 'string', enum: ['frontrun', 'victim', 'backrun'] },
            from: { type: 'string' },
            to: { type: 'string' },
            value: { type: 'number' },
            gasPrice: { type: 'number' }
          }
        }
      },
      targetProtocol: {
        type: 'string',
        enum: ['uniswap_v2', 'uniswap_v3', 'sushiswap', 'curve', 'balancer', 'aave', 'compound']
      },
      profit: {
        type: 'object',
        properties: {
          gross: { type: 'number' },
          gasCost: { type: 'number' },
          net: { type: 'number' },
          roi: { type: 'number' }
        }
      },
      victimTx: {
        type: 'object',
        properties: {
          hash: { type: 'string' },
          expectedSlippage: { type: 'number' },
          actualSlippage: { type: 'number' },
          lossUSD: { type: 'number' }
        }
      },
      mevBot: {
        type: 'object',
        properties: {
          address: { type: 'string', format: 'ethereum_address' },
          operator: { type: 'string' },
          flashbotRelay: { type: 'boolean' },
          privateTx: { type: 'boolean' }
        }
      },
      complexity: { type: 'integer', min: 1, max: 10 },
      executionTimeMs: { type: 'number', min: 10, max: 5000 }
    }
  });

  console.log('Generated MEV patterns:', result.data.length);

  // Analyze MEV types
  const types: any = {};
  result.data.forEach((mev: any) => {
    types[mev.mevType] = (types[mev.mevType] || 0) + 1;
  });
  console.log('MEV by type:', types);

  // Total profit extracted
  const totalProfit = result.data.reduce((sum: number, mev: any) =>
    sum + mev.profit.net, 0);
  console.log(`Total MEV extracted: $${totalProfit.toFixed(2)}`);

  return result;
}

/**
 * Example 6: Generate block production data
 * Includes validator performance and block building
 */
async function generateBlockData() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateTimeSeries({
    count: 7200, // ~24 hours of blocks (12s block time)
    interval: '12s',
    startDate: new Date(Date.now() - 24 * 60 * 60 * 1000),
    metrics: ['gasUsed', 'baseFee', 'mevReward', 'transactionCount'],
    trend: 'stable',
    seasonality: true,
    noise: 0.15,
    schema: {
      blockNumber: { type: 'integer', min: 18000000, required: true },
      timestamp: { type: 'datetime', format: 'iso8601' },
      hash: { type: 'string', format: 'block_hash' },
      parentHash: { type: 'string', format: 'block_hash' },
      proposer: { type: 'string', format: 'ethereum_address' },
      validator: {
        type: 'object',
        properties: {
          index: { type: 'integer' },
          balance: { type: 'number' },
          effectiveBalance: { type: 'number', default: 32 },
          slashed: { type: 'boolean', default: false }
        }
      },
      transactions: {
        type: 'object',
        properties: {
          count: { type: 'integer', min: 0, max: 300 },
          totalValue: { type: 'number' },
          totalFees: { type: 'number' },
          internalTxs: { type: 'integer' }
        }
      },
      gas: {
        type: 'object',
        properties: {
          used: { type: 'integer', min: 0, max: 30000000 },
          limit: { type: 'integer', default: 30000000 },
          utilization: { type: 'number', min: 0, max: 100 },
          baseFee: { type: 'number', min: 1 },
          avgPriorityFee: { type: 'number' }
        }
      },
      mev: {
        type: 'object',
        properties: {
          bundles: { type: 'integer', min: 0 },
          reward: { type: 'number', min: 0 },
          flashbotsTx: { type: 'integer', min: 0 }
        }
      },
      size: { type: 'integer', min: 1000 },
      difficulty: { type: 'string' },
      extraData: { type: 'string' },
      blockReward: { type: 'number', min: 0 }
    }
  });

  console.log('Generated block data:', result.data.length);

  // Calculate average block stats
  const avgGasUsed = result.data.reduce((sum: number, block: any) =>
    sum + block.gas.used, 0) / result.data.length;
  const avgTxCount = result.data.reduce((sum: number, block: any) =>
    sum + block.transactions.count, 0) / result.data.length;

  console.log('Average block statistics:');
  console.log(`  Gas used: ${avgGasUsed.toFixed(0)}`);
  console.log(`  Transactions: ${avgTxCount.toFixed(0)}`);

  return result;
}

/**
 * Example 7: Generate smart contract deployment patterns
 * Tracks contract creation and verification
 */
async function generateContractDeployments() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateEvents({
    count: 1000,
    eventTypes: ['erc20', 'erc721', 'erc1155', 'proxy', 'defi', 'custom'],
    distribution: 'uniform',
    timeRange: {
      start: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000),
      end: new Date()
    },
    userCount: 500,
    schema: {
      txHash: { type: 'string', format: 'transaction_hash' },
      blockNumber: { type: 'integer', min: 18000000 },
      timestamp: { type: 'datetime', format: 'iso8601' },
      deployer: { type: 'string', format: 'ethereum_address' },
      contractAddress: { type: 'string', format: 'ethereum_address' },
      contractType: {
        type: 'string',
        enum: ['token', 'nft', 'defi', 'dao', 'bridge', 'oracle', 'gaming', 'other']
      },
      standard: { type: 'string', enum: ['erc20', 'erc721', 'erc1155', 'erc4626', 'custom'] },
      bytecode: { type: 'string' },
      bytecodeSize: { type: 'integer', min: 100, max: 24576 },
      constructorArgs: { type: 'array' },
      verified: { type: 'boolean' },
      verificationDate: { type: 'datetime' },
      compiler: {
        type: 'object',
        properties: {
          version: { type: 'string' },
          optimization: { type: 'boolean' },
          runs: { type: 'integer', default: 200 }
        }
      },
      proxy: {
        type: 'object',
        properties: {
          isProxy: { type: 'boolean' },
          implementation: { type: 'string' },
          proxyType: { type: 'string', enum: ['transparent', 'uups', 'beacon', 'none'] }
        }
      },
      gasUsed: { type: 'integer', min: 100000 },
      deploymentCost: { type: 'number', min: 0 },
      activity: {
        type: 'object',
        properties: {
          uniqueUsers: { type: 'integer', min: 0 },
          totalTxs: { type: 'integer', min: 0 },
          totalVolume: { type: 'number', min: 0 }
        }
      }
    }
  });

  console.log('Generated contract deployments:', result.data.length);

  // Analyze by type
  const types: any = {};
  result.data.forEach((contract: any) => {
    types[contract.contractType] = (types[contract.contractType] || 0) + 1;
  });
  console.log('Deployments by type:', types);

  const verified = result.data.filter((c: any) => c.verified).length;
  console.log(`Verification rate: ${(verified / result.data.length * 100).toFixed(1)}%`);

  return result;
}

/**
 * Example 8: Generate cross-chain bridge activity
 * Simulates asset transfers between blockchains
 */
async function generateBridgeActivity() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateEvents({
    count: 2000,
    eventTypes: ['deposit', 'withdraw', 'relay'],
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000),
      end: new Date()
    },
    userCount: 1000,
    schema: {
      bridgeTxId: { type: 'string', format: 'uuid' },
      timestamp: { type: 'datetime', format: 'iso8601' },
      bridge: {
        type: 'string',
        enum: ['stargate', 'hop', 'across', 'synapse', 'multichain', 'wormhole', 'layerzero']
      },
      sourceChain: {
        type: 'string',
        enum: ['ethereum', 'polygon', 'arbitrum', 'optimism', 'avalanche', 'bsc']
      },
      destinationChain: {
        type: 'string',
        enum: ['ethereum', 'polygon', 'arbitrum', 'optimism', 'avalanche', 'bsc']
      },
      user: { type: 'string', format: 'ethereum_address' },
      asset: { type: 'string', enum: ['ETH', 'USDC', 'USDT', 'DAI', 'WBTC'] },
      amount: { type: 'number', min: 1 },
      amountUSD: { type: 'number', min: 1 },
      fees: {
        type: 'object',
        properties: {
          sourceGas: { type: 'number' },
          bridgeFee: { type: 'number' },
          destinationGas: { type: 'number' },
          totalFee: { type: 'number' }
        }
      },
      status: {
        type: 'string',
        enum: ['pending', 'relaying', 'completed', 'failed', 'refunded']
      },
      sourceTxHash: { type: 'string', format: 'transaction_hash' },
      destinationTxHash: { type: 'string', format: 'transaction_hash' },
      completionTime: { type: 'number', min: 60, max: 3600 }, // seconds
      securityDelay: { type: 'number', min: 0, max: 86400 }
    }
  });

  console.log('Generated bridge activity:', result.data.length);

  // Analyze bridge usage
  const bridges: any = {};
  result.data.forEach((tx: any) => {
    bridges[tx.bridge] = (bridges[tx.bridge] || 0) + 1;
  });
  console.log('Usage by bridge:', bridges);

  // Calculate success rate
  const completed = result.data.filter((tx: any) => tx.status === 'completed').length;
  console.log(`Success rate: ${(completed / result.data.length * 100).toFixed(1)}%`);

  return result;
}

/**
 * Run all blockchain data examples
 */
export async function runBlockchainDataExamples() {
  console.log('=== Blockchain and On-Chain Data Generation Examples ===\n');

  console.log('Example 1: Transaction Patterns');
  await generateTransactionPatterns();
  console.log('\n---\n');

  console.log('Example 2: Wallet Behavior');
  await generateWalletBehavior();
  console.log('\n---\n');

  console.log('Example 3: Token Transfers');
  await generateTokenTransfers();
  console.log('\n---\n');

  console.log('Example 4: NFT Activity');
  await generateNFTActivity();
  console.log('\n---\n');

  console.log('Example 5: MEV Patterns');
  await generateMEVPatterns();
  console.log('\n---\n');

  console.log('Example 6: Block Production Data');
  await generateBlockData();
  console.log('\n---\n');

  console.log('Example 7: Contract Deployments');
  await generateContractDeployments();
  console.log('\n---\n');

  console.log('Example 8: Bridge Activity');
  await generateBridgeActivity();
}

// Export individual examples
export {
  generateTransactionPatterns,
  generateWalletBehavior,
  generateTokenTransfers,
  generateNFTActivity,
  generateMEVPatterns,
  generateBlockData,
  generateContractDeployments,
  generateBridgeActivity
};

// Uncomment to run
// runBlockchainDataExamples().catch(console.error);
