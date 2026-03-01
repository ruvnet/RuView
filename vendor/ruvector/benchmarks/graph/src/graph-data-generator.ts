/**
 * Graph data generator using agentic-synth
 * Generates synthetic graph datasets for benchmarking
 */

import { AgenticSynth, createSynth } from '@ruvector/agentic-synth';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

export interface GraphNode {
  id: string;
  labels: string[];
  properties: Record<string, unknown>;
}

export interface GraphEdge {
  id: string;
  from: string;
  to: string;
  type: string;
  properties: Record<string, unknown>;
}

export interface GraphDataset {
  nodes: GraphNode[];
  edges: GraphEdge[];
  metadata: {
    nodeCount: number;
    edgeCount: number;
    avgDegree: number;
    labels: string[];
    relationshipTypes: string[];
  };
}

/**
 * Generate social network graph data
 */
export async function generateSocialNetwork(
  numUsers: number = 1000000,
  avgFriends: number = 10
): Promise<GraphDataset> {
  console.log(`Generating social network: ${numUsers} users, avg ${avgFriends} friends...`);

  const synth = createSynth({
    provider: 'gemini',
    model: 'gemini-2.0-flash-exp'
  });

  const nodes: GraphNode[] = [];
  const edges: GraphEdge[] = [];

  // Generate users in batches
  const batchSize = 10000;
  const numBatches = Math.ceil(numUsers / batchSize);

  for (let batch = 0; batch < numBatches; batch++) {
    const batchStart = batch * batchSize;
    const batchEnd = Math.min(batchStart + batchSize, numUsers);
    const batchUsers = batchEnd - batchStart;

    console.log(`  Generating users ${batchStart}-${batchEnd}...`);

    // Use agentic-synth to generate realistic user data
    const userResult = await synth.generateStructured({
      type: 'json',
      count: batchUsers,
      schema: {
        id: 'string',
        name: 'string',
        age: 'number',
        location: 'string',
        interests: 'array<string>',
        joinDate: 'timestamp'
      },
      prompt: `Generate realistic social media user profiles with diverse demographics,
               locations (cities worldwide), ages (18-80), and interests (hobbies, activities, topics).
               Make names culturally appropriate for their locations.`
    });

    // Convert to graph nodes
    for (let i = 0; i < batchUsers; i++) {
      const userId = `user_${batchStart + i}`;
      const userData = userResult.data[i] as Record<string, unknown>;

      nodes.push({
        id: userId,
        labels: ['Person', 'User'],
        properties: userData
      });
    }
  }

  console.log(`Generated ${nodes.length} user nodes`);

  // Generate friendships (edges)
  const numEdges = Math.floor(numUsers * avgFriends / 2); // Undirected, so divide by 2
  console.log(`Generating ${numEdges} friendships...`);

  // Use preferential attachment (scale-free network)
  const degrees = new Array(numUsers).fill(0);

  for (let i = 0; i < numEdges; i++) {
    if (i % 100000 === 0) {
      console.log(`  Generated ${i} edges...`);
    }

    // Select nodes with preferential attachment
    let from = Math.floor(Math.random() * numUsers);
    let to = Math.floor(Math.random() * numUsers);

    // Avoid self-loops
    while (to === from) {
      to = Math.floor(Math.random() * numUsers);
    }

    const edgeId = `friendship_${i}`;
    const friendshipDate = new Date(
      Date.now() - Math.random() * 365 * 24 * 60 * 60 * 1000 * 5
    ).toISOString();

    edges.push({
      id: edgeId,
      from: `user_${from}`,
      to: `user_${to}`,
      type: 'FRIENDS_WITH',
      properties: {
        since: friendshipDate,
        strength: Math.random()
      }
    });

    degrees[from]++;
    degrees[to]++;
  }

  const avgDegree = degrees.reduce((a, b) => a + b, 0) / numUsers;
  console.log(`Average degree: ${avgDegree.toFixed(2)}`);

  return {
    nodes,
    edges,
    metadata: {
      nodeCount: nodes.length,
      edgeCount: edges.length,
      avgDegree,
      labels: ['Person', 'User'],
      relationshipTypes: ['FRIENDS_WITH']
    }
  };
}

/**
 * Generate knowledge graph data
 */
export async function generateKnowledgeGraph(
  numEntities: number = 100000
): Promise<GraphDataset> {
  console.log(`Generating knowledge graph: ${numEntities} entities...`);

  const synth = createSynth({
    provider: 'gemini',
    model: 'gemini-2.0-flash-exp'
  });

  const nodes: GraphNode[] = [];
  const edges: GraphEdge[] = [];

  // Generate different entity types
  const entityTypes = [
    { label: 'Person', count: 0.3, schema: { name: 'string', birthDate: 'date', nationality: 'string' } },
    { label: 'Organization', count: 0.25, schema: { name: 'string', founded: 'number', industry: 'string' } },
    { label: 'Location', count: 0.2, schema: { name: 'string', country: 'string', lat: 'number', lon: 'number' } },
    { label: 'Event', count: 0.15, schema: { name: 'string', date: 'date', type: 'string' } },
    { label: 'Concept', count: 0.1, schema: { name: 'string', domain: 'string', definition: 'string' } }
  ];

  let entityId = 0;

  for (const entityType of entityTypes) {
    const count = Math.floor(numEntities * entityType.count);
    console.log(`  Generating ${count} ${entityType.label} entities...`);

    const result = await synth.generateStructured({
      type: 'json',
      count,
      schema: entityType.schema,
      prompt: `Generate realistic ${entityType.label} entities for a knowledge graph.
               Ensure diversity and real-world accuracy.`
    });

    for (const entity of result.data) {
      nodes.push({
        id: `entity_${entityId++}`,
        labels: [entityType.label, 'Entity'],
        properties: entity as Record<string, unknown>
      });
    }
  }

  console.log(`Generated ${nodes.length} entity nodes`);

  // Generate relationships
  const relationshipTypes = [
    'WORKS_AT',
    'LOCATED_IN',
    'PARTICIPATED_IN',
    'RELATED_TO',
    'INFLUENCED_BY'
  ];

  const numEdges = numEntities * 10; // 10 relationships per entity on average
  console.log(`Generating ${numEdges} relationships...`);

  for (let i = 0; i < numEdges; i++) {
    if (i % 50000 === 0) {
      console.log(`  Generated ${i} relationships...`);
    }

    const from = Math.floor(Math.random() * nodes.length);
    const to = Math.floor(Math.random() * nodes.length);

    if (from === to) continue;

    const relType = relationshipTypes[Math.floor(Math.random() * relationshipTypes.length)];

    edges.push({
      id: `rel_${i}`,
      from: nodes[from].id,
      to: nodes[to].id,
      type: relType,
      properties: {
        confidence: Math.random(),
        source: 'generated'
      }
    });
  }

  return {
    nodes,
    edges,
    metadata: {
      nodeCount: nodes.length,
      edgeCount: edges.length,
      avgDegree: (edges.length * 2) / nodes.length,
      labels: entityTypes.map(t => t.label),
      relationshipTypes
    }
  };
}

/**
 * Generate temporal event graph
 */
export async function generateTemporalGraph(
  numEvents: number = 500000,
  timeRangeDays: number = 365
): Promise<GraphDataset> {
  console.log(`Generating temporal graph: ${numEvents} events over ${timeRangeDays} days...`);

  const synth = createSynth({
    provider: 'gemini',
    model: 'gemini-2.0-flash-exp'
  });

  const nodes: GraphNode[] = [];
  const edges: GraphEdge[] = [];

  // Generate time-series events
  console.log('  Generating event data...');

  const eventResult = await synth.generateTimeSeries({
    type: 'timeseries',
    count: numEvents,
    interval: Math.floor((timeRangeDays * 24 * 60 * 60 * 1000) / numEvents),
    schema: {
      eventType: 'string',
      severity: 'number',
      entity: 'string',
      state: 'string'
    },
    prompt: `Generate realistic system events including state changes, user actions,
             system alerts, and business events. Include severity levels 1-5.`
  });

  for (let i = 0; i < numEvents; i++) {
    const eventData = eventResult.data[i] as Record<string, unknown>;

    nodes.push({
      id: `event_${i}`,
      labels: ['Event'],
      properties: {
        ...eventData,
        timestamp: new Date(Date.now() - Math.random() * timeRangeDays * 24 * 60 * 60 * 1000).toISOString()
      }
    });
  }

  console.log(`Generated ${nodes.length} event nodes`);

  // Generate state transitions (temporal edges)
  console.log('  Generating state transitions...');

  for (let i = 0; i < numEvents - 1; i++) {
    if (i % 50000 === 0) {
      console.log(`  Generated ${i} transitions...`);
    }

    // Connect events that are causally related (next event in sequence)
    if (Math.random() < 0.3) {
      edges.push({
        id: `transition_${i}`,
        from: `event_${i}`,
        to: `event_${i + 1}`,
        type: 'TRANSITIONS_TO',
        properties: {
          duration: Math.random() * 1000,
          probability: Math.random()
        }
      });
    }

    // Add some random connections for causality
    if (Math.random() < 0.1 && i > 10) {
      const target = Math.floor(Math.random() * i);
      edges.push({
        id: `caused_by_${i}`,
        from: `event_${i}`,
        to: `event_${target}`,
        type: 'CAUSED_BY',
        properties: {
          correlation: Math.random()
        }
      });
    }
  }

  return {
    nodes,
    edges,
    metadata: {
      nodeCount: nodes.length,
      edgeCount: edges.length,
      avgDegree: (edges.length * 2) / nodes.length,
      labels: ['Event', 'State'],
      relationshipTypes: ['TRANSITIONS_TO', 'CAUSED_BY']
    }
  };
}

/**
 * Save dataset to files
 */
export function saveDataset(dataset: GraphDataset, name: string, outputDir: string = './data') {
  mkdirSync(outputDir, { recursive: true });

  const nodesFile = join(outputDir, `${name}_nodes.json`);
  const edgesFile = join(outputDir, `${name}_edges.json`);
  const metadataFile = join(outputDir, `${name}_metadata.json`);

  console.log(`Saving dataset to ${outputDir}...`);

  writeFileSync(nodesFile, JSON.stringify(dataset.nodes, null, 2));
  writeFileSync(edgesFile, JSON.stringify(dataset.edges, null, 2));
  writeFileSync(metadataFile, JSON.stringify(dataset.metadata, null, 2));

  console.log(`  Nodes: ${nodesFile}`);
  console.log(`  Edges: ${edgesFile}`);
  console.log(`  Metadata: ${metadataFile}`);
}

/**
 * Main function to generate all datasets
 */
export async function generateAllDatasets() {
  console.log('=== RuVector Graph Benchmark Data Generation ===\n');

  // Social Network
  const socialNetwork = await generateSocialNetwork(1000000, 10);
  saveDataset(socialNetwork, 'social_network', './benchmarks/data/graph');

  console.log('');

  // Knowledge Graph
  const knowledgeGraph = await generateKnowledgeGraph(100000);
  saveDataset(knowledgeGraph, 'knowledge_graph', './benchmarks/data/graph');

  console.log('');

  // Temporal Graph
  const temporalGraph = await generateTemporalGraph(500000, 365);
  saveDataset(temporalGraph, 'temporal_events', './benchmarks/data/graph');

  console.log('\n=== Data Generation Complete ===');
}

// Run if called directly
if (require.main === module) {
  generateAllDatasets().catch(console.error);
}
