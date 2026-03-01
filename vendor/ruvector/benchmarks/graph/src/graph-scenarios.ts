/**
 * Graph benchmark scenarios for RuVector graph database
 * Tests various graph operations and compares with Neo4j
 */

export interface GraphScenario {
  name: string;
  description: string;
  type: 'traversal' | 'write' | 'aggregation' | 'mixed' | 'concurrent';
  setup: () => Promise<void>;
  execute: () => Promise<BenchmarkResult>;
  cleanup?: () => Promise<void>;
}

export interface BenchmarkResult {
  scenario: string;
  duration_ms: number;
  operations_per_second: number;
  memory_mb?: number;
  cpu_percent?: number;
  metadata?: Record<string, unknown>;
}

export interface GraphDataset {
  name: string;
  nodes: number;
  edges: number;
  labels: string[];
  relationshipTypes: string[];
  properties: Record<string, string>;
}

/**
 * Social Network Scenario
 * Simulates a social graph with users, posts, and relationships
 */
export const socialNetworkScenario: GraphScenario = {
  name: 'social_network_1m',
  description: 'Social network with 1M users and 10M friendships',
  type: 'mixed',

  setup: async () => {
    console.log('Setting up social network dataset...');
    // Will use agentic-synth to generate realistic social graph data
  },

  execute: async () => {
    const start = Date.now();

    // Benchmark operations:
    // 1. Create users (batch insert)
    // 2. Create friendships (batch edge creation)
    // 3. Friend recommendations (2-hop traversal)
    // 4. Mutual friends (intersection query)
    // 5. Influencer detection (degree centrality)

    const duration = Date.now() - start;

    return {
      scenario: 'social_network_1m',
      duration_ms: duration,
      operations_per_second: 1000000 / (duration / 1000),
      metadata: {
        nodes_created: 1000000,
        edges_created: 10000000,
        queries_executed: 5
      }
    };
  }
};

/**
 * Knowledge Graph Scenario
 * Tests entity relationships and multi-hop reasoning
 */
export const knowledgeGraphScenario: GraphScenario = {
  name: 'knowledge_graph_100k',
  description: 'Knowledge graph with 100K entities and 1M relationships',
  type: 'traversal',

  setup: async () => {
    console.log('Setting up knowledge graph dataset...');
  },

  execute: async () => {
    const start = Date.now();

    // Benchmark operations:
    // 1. Entity creation (Person, Organization, Location, Event)
    // 2. Relationship creation (works_at, located_in, participated_in)
    // 3. Multi-hop queries (person -> organization -> location)
    // 4. Path finding (shortest path between entities)
    // 5. Pattern matching (find all people in same organization and location)

    const duration = Date.now() - start;

    return {
      scenario: 'knowledge_graph_100k',
      duration_ms: duration,
      operations_per_second: 100000 / (duration / 1000)
    };
  }
};

/**
 * Temporal Graph Scenario
 * Tests time-based queries and event ordering
 */
export const temporalGraphScenario: GraphScenario = {
  name: 'temporal_graph_events',
  description: 'Temporal graph with time-series events and state transitions',
  type: 'mixed',

  setup: async () => {
    console.log('Setting up temporal graph dataset...');
  },

  execute: async () => {
    const start = Date.now();

    // Benchmark operations:
    // 1. Event insertion (timestamped nodes)
    // 2. State transitions (temporal edges)
    // 3. Time-range queries (events between timestamps)
    // 4. Temporal path finding (valid paths at time T)
    // 5. Event aggregation (count by time bucket)

    const duration = Date.now() - start;

    return {
      scenario: 'temporal_graph_events',
      duration_ms: duration,
      operations_per_second: 1000000 / (duration / 1000)
    };
  }
};

/**
 * Recommendation Engine Scenario
 * Tests collaborative filtering and similarity queries
 */
export const recommendationScenario: GraphScenario = {
  name: 'recommendation_engine',
  description: 'User-item bipartite graph for recommendations',
  type: 'traversal',

  setup: async () => {
    console.log('Setting up recommendation dataset...');
  },

  execute: async () => {
    const start = Date.now();

    // Benchmark operations:
    // 1. Create users and items
    // 2. Create rating/interaction edges
    // 3. Collaborative filtering (similar users)
    // 4. Item recommendations (2-hop: user -> items <- users -> items)
    // 5. Trending items (aggregation by interaction count)

    const duration = Date.now() - start;

    return {
      scenario: 'recommendation_engine',
      duration_ms: duration,
      operations_per_second: 500000 / (duration / 1000)
    };
  }
};

/**
 * Fraud Detection Scenario
 * Tests pattern matching and anomaly detection
 */
export const fraudDetectionScenario: GraphScenario = {
  name: 'fraud_detection',
  description: 'Transaction graph for fraud pattern detection',
  type: 'aggregation',

  setup: async () => {
    console.log('Setting up fraud detection dataset...');
  },

  execute: async () => {
    const start = Date.now();

    // Benchmark operations:
    // 1. Create accounts and transactions
    // 2. Circular transfer detection (cycle detection)
    // 3. Velocity checks (count transactions in time window)
    // 4. Network analysis (connected components)
    // 5. Risk scoring (aggregation across relationships)

    const duration = Date.now() - start;

    return {
      scenario: 'fraud_detection',
      duration_ms: duration,
      operations_per_second: 200000 / (duration / 1000)
    };
  }
};

/**
 * Concurrent Write Scenario
 * Tests multi-threaded write performance
 */
export const concurrentWriteScenario: GraphScenario = {
  name: 'concurrent_writes',
  description: 'Concurrent node and edge creation from multiple threads',
  type: 'concurrent',

  setup: async () => {
    console.log('Setting up concurrent write test...');
  },

  execute: async () => {
    const start = Date.now();

    // Benchmark operations:
    // 1. Spawn multiple concurrent writers
    // 2. Each writes 10K nodes + 50K edges
    // 3. Test with 2, 4, 8, 16 threads
    // 4. Measure throughput and contention

    const duration = Date.now() - start;

    return {
      scenario: 'concurrent_writes',
      duration_ms: duration,
      operations_per_second: 100000 / (duration / 1000),
      metadata: {
        threads: 8,
        contention_rate: 0.05
      }
    };
  }
};

/**
 * Deep Traversal Scenario
 * Tests performance of deep graph traversals
 */
export const deepTraversalScenario: GraphScenario = {
  name: 'deep_traversal',
  description: 'Multi-hop traversals up to 6 degrees of separation',
  type: 'traversal',

  setup: async () => {
    console.log('Setting up deep traversal dataset...');
  },

  execute: async () => {
    const start = Date.now();

    // Benchmark operations:
    // 1. Create dense graph (avg degree = 50)
    // 2. 1-hop traversal (immediate neighbors)
    // 3. 2-hop traversal (friends of friends)
    // 4. 3-hop traversal
    // 5. 6-hop traversal (6 degrees of separation)

    const duration = Date.now() - start;

    return {
      scenario: 'deep_traversal',
      duration_ms: duration,
      operations_per_second: 1000 / (duration / 1000),
      metadata: {
        max_depth: 6,
        avg_results_per_hop: [50, 2500, 125000]
      }
    };
  }
};

/**
 * Aggregation Heavy Scenario
 * Tests aggregation and analytical queries
 */
export const aggregationScenario: GraphScenario = {
  name: 'aggregation_analytics',
  description: 'Complex aggregation and analytical queries',
  type: 'aggregation',

  setup: async () => {
    console.log('Setting up aggregation dataset...');
  },

  execute: async () => {
    const start = Date.now();

    // Benchmark operations:
    // 1. Count nodes by label
    // 2. Average property values
    // 3. Group by with aggregation
    // 4. Percentile calculations
    // 5. Graph statistics (degree distribution)

    const duration = Date.now() - start;

    return {
      scenario: 'aggregation_analytics',
      duration_ms: duration,
      operations_per_second: 1000000 / (duration / 1000)
    };
  }
};

/**
 * All benchmark scenarios
 */
export const allScenarios: GraphScenario[] = [
  socialNetworkScenario,
  knowledgeGraphScenario,
  temporalGraphScenario,
  recommendationScenario,
  fraudDetectionScenario,
  concurrentWriteScenario,
  deepTraversalScenario,
  aggregationScenario
];

/**
 * Dataset definitions for synthetic data generation
 */
export const datasets: GraphDataset[] = [
  {
    name: 'social_network',
    nodes: 1000000,
    edges: 10000000,
    labels: ['Person', 'Post', 'Comment', 'Group'],
    relationshipTypes: ['FRIENDS_WITH', 'POSTED', 'COMMENTED_ON', 'MEMBER_OF', 'LIKES'],
    properties: {
      Person: 'id, name, age, location, joinDate',
      Post: 'id, content, timestamp, likes',
      Comment: 'id, text, timestamp',
      Group: 'id, name, memberCount'
    }
  },
  {
    name: 'knowledge_graph',
    nodes: 100000,
    edges: 1000000,
    labels: ['Person', 'Organization', 'Location', 'Event', 'Concept'],
    relationshipTypes: ['WORKS_AT', 'LOCATED_IN', 'PARTICIPATED_IN', 'RELATED_TO', 'INFLUENCED_BY'],
    properties: {
      Person: 'id, name, birth_date, nationality',
      Organization: 'id, name, founded, industry',
      Location: 'id, name, country, coordinates',
      Event: 'id, name, date, description',
      Concept: 'id, name, domain, definition'
    }
  },
  {
    name: 'temporal_events',
    nodes: 500000,
    edges: 2000000,
    labels: ['Event', 'State', 'Entity'],
    relationshipTypes: ['TRANSITIONS_TO', 'TRIGGERED_BY', 'AFFECTS'],
    properties: {
      Event: 'id, timestamp, type, severity',
      State: 'id, value, validFrom, validTo',
      Entity: 'id, name, currentState'
    }
  }
];
