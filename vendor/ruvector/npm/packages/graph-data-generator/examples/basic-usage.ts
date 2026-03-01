/**
 * Basic usage examples for @ruvector/graph-data-generator
 */

import { createGraphDataGenerator } from '../src/index.js';
import fs from 'fs';

async function main() {
  // Initialize generator with OpenRouter API key
  const generator = createGraphDataGenerator({
    apiKey: process.env.OPENROUTER_API_KEY,
    model: 'moonshot/kimi-k2-instruct'
  });

  console.log('=== Knowledge Graph Generation ===');
  const knowledgeGraph = await generator.generateKnowledgeGraph({
    domain: 'technology',
    entities: 50,
    relationships: 150,
    includeEmbeddings: true,
    embeddingDimension: 768
  });

  console.log(`Generated ${knowledgeGraph.data.nodes.length} nodes`);
  console.log(`Generated ${knowledgeGraph.data.edges.length} edges`);
  console.log(`Duration: ${knowledgeGraph.metadata.duration}ms`);

  // Generate Cypher statements
  const cypher = generator.generateCypher(knowledgeGraph.data, {
    useConstraints: true,
    useIndexes: true,
    useMerge: false
  });

  // Save to file
  fs.writeFileSync('knowledge-graph.cypher', cypher);
  console.log('Saved Cypher to knowledge-graph.cypher');

  console.log('\n=== Social Network Generation ===');
  const socialNetwork = await generator.generateSocialNetwork({
    users: 100,
    avgConnections: 10,
    networkType: 'small-world',
    includeMetadata: true,
    includeEmbeddings: false
  });

  console.log(`Generated ${socialNetwork.data.nodes.length} users`);
  console.log(`Generated ${socialNetwork.data.edges.length} connections`);

  const socialCypher = generator.generateCypher(socialNetwork.data);
  fs.writeFileSync('social-network.cypher', socialCypher);
  console.log('Saved Cypher to social-network.cypher');

  console.log('\n=== Temporal Events Generation ===');
  const temporalEvents = await generator.generateTemporalEvents({
    startDate: '2024-01-01',
    endDate: '2024-01-31',
    eventTypes: ['login', 'purchase', 'logout', 'error'],
    eventsPerDay: 20,
    entities: 25,
    includeEmbeddings: false
  });

  console.log(`Generated ${temporalEvents.data.nodes.length} nodes`);
  console.log(`Generated ${temporalEvents.data.edges.length} edges`);

  const temporalCypher = generator.generateCypher(temporalEvents.data);
  fs.writeFileSync('temporal-events.cypher', temporalCypher);
  console.log('Saved Cypher to temporal-events.cypher');

  console.log('\n=== Entity Relationships Generation ===');
  const erGraph = await generator.generateEntityRelationships({
    domain: 'e-commerce',
    entityCount: 75,
    relationshipDensity: 0.2,
    includeEmbeddings: false
  });

  console.log(`Generated ${erGraph.data.nodes.length} entities`);
  console.log(`Generated ${erGraph.data.edges.length} relationships`);

  const erCypher = generator.generateCypher(erGraph.data);
  fs.writeFileSync('entity-relationships.cypher', erCypher);
  console.log('Saved Cypher to entity-relationships.cypher');

  console.log('\n✓ All examples completed successfully!');
}

main().catch(console.error);
