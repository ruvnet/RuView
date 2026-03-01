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
/**
 * Example 1: Generate realistic transaction patterns
 * Simulates various transaction types across different networks
 */
declare function generateTransactionPatterns(): Promise<{
    network: string;
    data: unknown[];
}[]>;
/**
 * Example 2: Simulate wallet behavior patterns
 * Includes HODLers, traders, bots, and contract wallets
 */
declare function generateWalletBehavior(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 3: Generate token transfer events
 * Simulates ERC-20, ERC-721, and ERC-1155 transfers
 */
declare function generateTokenTransfers(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 4: Generate NFT trading activity
 * Includes mints, sales, and marketplace activity
 */
declare function generateNFTActivity(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 5: Generate MEV transaction patterns
 * Advanced MEV extraction and sandwich attack simulations
 */
declare function generateMEVPatterns(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 6: Generate block production data
 * Includes validator performance and block building
 */
declare function generateBlockData(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 7: Generate smart contract deployment patterns
 * Tracks contract creation and verification
 */
declare function generateContractDeployments(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Example 8: Generate cross-chain bridge activity
 * Simulates asset transfers between blockchains
 */
declare function generateBridgeActivity(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Run all blockchain data examples
 */
export declare function runBlockchainDataExamples(): Promise<void>;
export { generateTransactionPatterns, generateWalletBehavior, generateTokenTransfers, generateNFTActivity, generateMEVPatterns, generateBlockData, generateContractDeployments, generateBridgeActivity };
//# sourceMappingURL=blockchain-data.d.ts.map