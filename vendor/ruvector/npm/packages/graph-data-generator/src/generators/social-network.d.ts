/**
 * Social network generator using OpenRouter/Kimi K2
 */
import { OpenRouterClient } from '../openrouter-client.js';
import { SocialNetworkOptions, GraphData, GraphGenerationResult } from '../types.js';
export declare class SocialNetworkGenerator {
    private client;
    constructor(client: OpenRouterClient);
    /**
     * Generate a social network graph
     */
    generate(options: SocialNetworkOptions): Promise<GraphGenerationResult<GraphData>>;
    /**
     * Generate realistic social network users
     */
    private generateUsers;
    /**
     * Generate connections between users
     */
    private generateConnections;
    /**
     * Get guidance for network type
     */
    private getNetworkTypeGuidance;
    /**
     * Analyze network properties
     */
    analyzeNetwork(data: GraphData): Promise<{
        avgDegree: number;
        maxDegree: number;
        communities?: number;
        clustering?: number;
    }>;
}
/**
 * Create a social network generator
 */
export declare function createSocialNetworkGenerator(client: OpenRouterClient): SocialNetworkGenerator;
//# sourceMappingURL=social-network.d.ts.map