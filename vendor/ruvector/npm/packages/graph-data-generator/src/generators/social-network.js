"use strict";
/**
 * Social network generator using OpenRouter/Kimi K2
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.SocialNetworkGenerator = void 0;
exports.createSocialNetworkGenerator = createSocialNetworkGenerator;
class SocialNetworkGenerator {
    constructor(client) {
        this.client = client;
    }
    /**
     * Generate a social network graph
     */
    async generate(options) {
        const startTime = Date.now();
        // Generate users
        const users = await this.generateUsers(options);
        // Generate connections based on network type
        const connections = await this.generateConnections(users, options);
        // Convert to graph structure
        const nodes = users.map(user => ({
            id: user.id,
            labels: ['User'],
            properties: {
                username: user.username,
                ...user.profile,
                ...(user.metadata || {})
            }
        }));
        const edges = connections.map((conn, idx) => ({
            id: `connection_${idx}`,
            type: conn.type || 'FOLLOWS',
            source: conn.source,
            target: conn.target,
            properties: conn.properties || {}
        }));
        const data = {
            nodes,
            edges,
            metadata: {
                domain: 'social-network',
                generated_at: new Date(),
                total_nodes: nodes.length,
                total_edges: edges.length
            }
        };
        return {
            data,
            metadata: {
                generated_at: new Date(),
                model: this.client.getConfig().model || 'moonshot/kimi-k2-instruct',
                duration: Date.now() - startTime
            }
        };
    }
    /**
     * Generate realistic social network users
     */
    async generateUsers(options) {
        const systemPrompt = 'You are an expert at creating realistic social network user profiles. Generate diverse, believable users.';
        const userPrompt = `Generate ${options.users} realistic social network user profiles.

Each user should have:
- id: unique user ID (format: user_XXXXX)
- username: unique username
- profile: object with name, bio, joined (ISO date), followers (number), following (number)
${options.includeMetadata ? '- metadata: additional information like interests, location, verified status' : ''}

Make the profiles diverse and realistic. Return a JSON array.

Example format:
\`\`\`json
[
  {
    "id": "user_12345",
    "username": "tech_enthusiast_42",
    "profile": {
      "name": "Alex Johnson",
      "bio": "Software developer passionate about AI and open source",
      "joined": "2020-03-15T00:00:00Z",
      "followers": 1250,
      "following": 430
    }${options.includeMetadata ? `,
    "metadata": {
      "interests": ["technology", "AI", "coding"],
      "location": "San Francisco, CA",
      "verified": false
    }` : ''}
  }
]
\`\`\``;
        return this.client.generateStructured(systemPrompt, userPrompt, {
            temperature: 0.9,
            maxTokens: Math.min(8000, options.users * 150)
        });
    }
    /**
     * Generate connections between users
     */
    async generateConnections(users, options) {
        const totalConnections = Math.floor(options.users * options.avgConnections / 2);
        const systemPrompt = `You are an expert at modeling social network connections. Create realistic ${options.networkType || 'random'} network patterns.`;
        const userList = users.slice(0, 100).map(u => `- ${u.id}: @${u.username}`).join('\n');
        const userPrompt = `Given these social network users:

${userList}

Generate ${totalConnections} connections creating a ${options.networkType || 'random'} network structure.

${this.getNetworkTypeGuidance(options.networkType)}

Each connection should have:
- source: user id who initiates the connection
- target: user id being connected to
- type: connection type (FOLLOWS, FRIEND, BLOCKS, MUTES)
- properties: optional properties like since (ISO date), strength (0-1)

Return a JSON array of connections.

Example format:
\`\`\`json
[
  {
    "source": "user_12345",
    "target": "user_67890",
    "type": "FOLLOWS",
    "properties": {
      "since": "2021-06-15T00:00:00Z",
      "strength": 0.8
    }
  }
]
\`\`\``;
        return this.client.generateStructured(systemPrompt, userPrompt, {
            temperature: 0.7,
            maxTokens: Math.min(8000, totalConnections * 80)
        });
    }
    /**
     * Get guidance for network type
     */
    getNetworkTypeGuidance(networkType) {
        switch (networkType) {
            case 'small-world':
                return 'Create clusters of highly connected users with occasional bridges between clusters (small-world property).';
            case 'scale-free':
                return 'Create a power-law distribution where a few users have many connections (influencers) and most have few connections.';
            case 'clustered':
                return 'Create distinct communities/clusters with high internal connectivity and sparse connections between clusters.';
            default:
                return 'Create random connections with varying connection strengths.';
        }
    }
    /**
     * Analyze network properties
     */
    async analyzeNetwork(data) {
        const degrees = new Map();
        for (const edge of data.edges) {
            degrees.set(edge.source, (degrees.get(edge.source) || 0) + 1);
            degrees.set(edge.target, (degrees.get(edge.target) || 0) + 1);
        }
        const degreeValues = Array.from(degrees.values());
        const avgDegree = degreeValues.reduce((a, b) => a + b, 0) / degreeValues.length;
        const maxDegree = Math.max(...degreeValues);
        return {
            avgDegree,
            maxDegree
        };
    }
}
exports.SocialNetworkGenerator = SocialNetworkGenerator;
/**
 * Create a social network generator
 */
function createSocialNetworkGenerator(client) {
    return new SocialNetworkGenerator(client);
}
//# sourceMappingURL=social-network.js.map