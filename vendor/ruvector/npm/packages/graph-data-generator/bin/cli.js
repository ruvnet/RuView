#!/usr/bin/env node

/**
 * CLI for @ruvector/graph-data-generator
 */

import { Command } from 'commander';
import { createGraphDataGenerator } from '../dist/index.js';
import fs from 'fs';
import path from 'path';

const program = new Command();

program
  .name('graph-synth')
  .description('AI-powered synthetic graph data generator with OpenRouter/Kimi K2')
  .version('0.1.0');

program
  .command('knowledge-graph')
  .description('Generate a knowledge graph')
  .requiredOption('-d, --domain <domain>', 'Domain for the knowledge graph')
  .requiredOption('-e, --entities <number>', 'Number of entities to generate')
  .requiredOption('-r, --relationships <number>', 'Number of relationships to generate')
  .option('--embeddings', 'Include vector embeddings')
  .option('-o, --output <file>', 'Output file (default: stdout)')
  .action(async (options) => {
    try {
      const generator = createGraphDataGenerator();

      console.error('Generating knowledge graph...');
      const result = await generator.generateKnowledgeGraph({
        domain: options.domain,
        entities: parseInt(options.entities),
        relationships: parseInt(options.relationships),
        includeEmbeddings: options.embeddings || false
      });

      const cypher = generator.generateCypher(result.data, {
        useConstraints: true,
        useIndexes: true
      });

      if (options.output) {
        fs.writeFileSync(options.output, cypher);
        console.error(`✓ Written to ${options.output}`);
      } else {
        console.log(cypher);
      }

      console.error(`✓ Generated ${result.data.nodes.length} nodes and ${result.data.edges.length} edges`);
    } catch (error) {
      console.error('Error:', error.message);
      process.exit(1);
    }
  });

program
  .command('social-network')
  .description('Generate a social network')
  .requiredOption('-u, --users <number>', 'Number of users to generate')
  .requiredOption('-c, --connections <number>', 'Average connections per user')
  .option('-t, --type <type>', 'Network type (random|small-world|scale-free|clustered)', 'random')
  .option('--embeddings', 'Include vector embeddings')
  .option('-o, --output <file>', 'Output file (default: stdout)')
  .action(async (options) => {
    try {
      const generator = createGraphDataGenerator();

      console.error('Generating social network...');
      const result = await generator.generateSocialNetwork({
        users: parseInt(options.users),
        avgConnections: parseInt(options.connections),
        networkType: options.type,
        includeEmbeddings: options.embeddings || false
      });

      const cypher = generator.generateCypher(result.data, {
        useConstraints: true,
        useIndexes: true
      });

      if (options.output) {
        fs.writeFileSync(options.output, cypher);
        console.error(`✓ Written to ${options.output}`);
      } else {
        console.log(cypher);
      }

      console.error(`✓ Generated ${result.data.nodes.length} users and ${result.data.edges.length} connections`);
    } catch (error) {
      console.error('Error:', error.message);
      process.exit(1);
    }
  });

program
  .command('temporal-events')
  .description('Generate temporal event graph')
  .requiredOption('-s, --start <date>', 'Start date (ISO format)')
  .requiredOption('-e, --end <date>', 'End date (ISO format)')
  .requiredOption('-t, --types <types>', 'Event types (comma-separated)')
  .option('-d, --events-per-day <number>', 'Events per day', '10')
  .option('--embeddings', 'Include vector embeddings')
  .option('-o, --output <file>', 'Output file (default: stdout)')
  .action(async (options) => {
    try {
      const generator = createGraphDataGenerator();

      console.error('Generating temporal events...');
      const result = await generator.generateTemporalEvents({
        startDate: options.start,
        endDate: options.end,
        eventTypes: options.types.split(',').map(t => t.trim()),
        eventsPerDay: parseInt(options.eventsPerDay),
        includeEmbeddings: options.embeddings || false
      });

      const cypher = generator.generateCypher(result.data, {
        useConstraints: true,
        useIndexes: true
      });

      if (options.output) {
        fs.writeFileSync(options.output, cypher);
        console.error(`✓ Written to ${options.output}`);
      } else {
        console.log(cypher);
      }

      console.error(`✓ Generated ${result.data.nodes.length} nodes and ${result.data.edges.length} edges`);
    } catch (error) {
      console.error('Error:', error.message);
      process.exit(1);
    }
  });

program
  .command('entity-relationships')
  .description('Generate entity-relationship graph')
  .requiredOption('-d, --domain <domain>', 'Domain for the entities')
  .requiredOption('-e, --entities <number>', 'Number of entities to generate')
  .requiredOption('--density <number>', 'Relationship density (0-1)')
  .option('--embeddings', 'Include vector embeddings')
  .option('-o, --output <file>', 'Output file (default: stdout)')
  .action(async (options) => {
    try {
      const generator = createGraphDataGenerator();

      console.error('Generating entity-relationship graph...');
      const result = await generator.generateEntityRelationships({
        domain: options.domain,
        entityCount: parseInt(options.entities),
        relationshipDensity: parseFloat(options.density),
        includeEmbeddings: options.embeddings || false
      });

      const cypher = generator.generateCypher(result.data, {
        useConstraints: true,
        useIndexes: true
      });

      if (options.output) {
        fs.writeFileSync(options.output, cypher);
        console.error(`✓ Written to ${options.output}`);
      } else {
        console.log(cypher);
      }

      console.error(`✓ Generated ${result.data.nodes.length} nodes and ${result.data.edges.length} edges`);
    } catch (error) {
      console.error('Error:', error.message);
      process.exit(1);
    }
  });

program.parse();
