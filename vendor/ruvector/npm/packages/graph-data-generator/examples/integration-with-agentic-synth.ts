/**
 * Integration example with @ruvector/agentic-synth
 *
 * This example shows how to use both agentic-synth and graph-data-generator
 * together to create comprehensive synthetic datasets.
 */

import { createSynth } from '@ruvector/agentic-synth';
import { createGraphDataGenerator } from '../src/index.js';
import fs from 'fs';

async function main() {
  // Initialize both generators
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY,
    model: 'gemini-2.0-flash-exp'
  });

  const graphGen = createGraphDataGenerator({
    apiKey: process.env.OPENROUTER_API_KEY,
    model: 'moonshot/kimi-k2-instruct'
  });

  console.log('=== Hybrid Synthetic Data Generation ===\n');

  // Step 1: Generate structured user data with agentic-synth
  console.log('1. Generating user profiles with agentic-synth...');
  const userProfiles = await synth.generateStructured({
    count: 50,
    schema: {
      user_id: { type: 'string' },
      name: { type: 'string' },
      email: { type: 'string' },
      role: { type: 'string', enum: ['developer', 'designer', 'manager', 'analyst'] },
      skills: { type: 'array', items: { type: 'string' } },
      experience_years: { type: 'number', minimum: 0, maximum: 30 }
    }
  });

  console.log(`✓ Generated ${userProfiles.data.length} user profiles`);

  // Step 2: Generate project data with agentic-synth
  console.log('\n2. Generating project data with agentic-synth...');
  const projects = await synth.generateStructured({
    count: 20,
    schema: {
      project_id: { type: 'string' },
      name: { type: 'string' },
      description: { type: 'string' },
      status: { type: 'string', enum: ['active', 'completed', 'on-hold'] },
      start_date: { type: 'string' },
      tech_stack: { type: 'array', items: { type: 'string' } }
    }
  });

  console.log(`✓ Generated ${projects.data.length} projects`);

  // Step 3: Generate knowledge graph relationships with graph-data-generator
  console.log('\n3. Generating knowledge graph with relationships...');
  const knowledgeGraph = await graphGen.generateKnowledgeGraph({
    domain: 'software development teams',
    entities: 100,
    relationships: 300,
    entityTypes: ['Person', 'Project', 'Skill', 'Technology', 'Team'],
    relationshipTypes: [
      'WORKS_ON',
      'HAS_SKILL',
      'USES_TECHNOLOGY',
      'MEMBER_OF',
      'DEPENDS_ON',
      'MENTORS'
    ],
    includeEmbeddings: true
  });

  console.log(`✓ Generated ${knowledgeGraph.data.nodes.length} nodes`);
  console.log(`✓ Generated ${knowledgeGraph.data.edges.length} edges`);

  // Step 4: Generate temporal event data
  console.log('\n4. Generating temporal events for user activities...');
  const temporalEvents = await graphGen.generateTemporalEvents({
    startDate: '2024-01-01',
    endDate: '2024-12-31',
    eventTypes: [
      'code_commit',
      'pull_request',
      'code_review',
      'deployment',
      'meeting',
      'task_completed'
    ],
    eventsPerDay: 50,
    entities: 50,
    includeEmbeddings: false
  });

  console.log(`✓ Generated ${temporalEvents.data.nodes.length} temporal nodes`);
  console.log(`✓ Generated ${temporalEvents.data.edges.length} temporal edges`);

  // Step 5: Generate time-series metrics with agentic-synth
  console.log('\n5. Generating time-series metrics with agentic-synth...');
  const metrics = await synth.generateTimeSeries({
    startDate: '2024-01-01',
    endDate: '2024-12-31',
    interval: '1d',
    metrics: ['code_quality', 'test_coverage', 'deployment_frequency'],
    trend: 'up',
    seasonality: true,
    noise: 0.1
  });

  console.log(`✓ Generated ${metrics.data.length} time-series data points`);

  // Step 6: Combine and export data
  console.log('\n6. Combining and exporting datasets...');

  // Save structured data as JSON
  fs.writeFileSync('users.json', JSON.stringify(userProfiles.data, null, 2));
  fs.writeFileSync('projects.json', JSON.stringify(projects.data, null, 2));
  fs.writeFileSync('metrics.json', JSON.stringify(metrics.data, null, 2));

  // Save graph data as Cypher
  const knowledgeCypher = graphGen.generateCypher(knowledgeGraph.data, {
    useConstraints: true,
    useIndexes: true,
    useMerge: true
  });
  fs.writeFileSync('knowledge-graph.cypher', knowledgeCypher);

  const temporalCypher = graphGen.generateCypher(temporalEvents.data, {
    useConstraints: true,
    useIndexes: true
  });
  fs.writeFileSync('temporal-events.cypher', temporalCypher);

  // Create a combined dataset summary
  const summary = {
    generation_timestamp: new Date().toISOString(),
    datasets: {
      user_profiles: {
        count: userProfiles.data.length,
        provider: 'gemini',
        file: 'users.json'
      },
      projects: {
        count: projects.data.length,
        provider: 'gemini',
        file: 'projects.json'
      },
      knowledge_graph: {
        nodes: knowledgeGraph.data.nodes.length,
        edges: knowledgeGraph.data.edges.length,
        provider: 'openrouter/kimi-k2',
        file: 'knowledge-graph.cypher',
        has_embeddings: true
      },
      temporal_events: {
        nodes: temporalEvents.data.nodes.length,
        edges: temporalEvents.data.edges.length,
        provider: 'openrouter/kimi-k2',
        file: 'temporal-events.cypher'
      },
      time_series_metrics: {
        count: metrics.data.length,
        provider: 'gemini',
        file: 'metrics.json'
      }
    },
    total_generation_time: {
      knowledge_graph: knowledgeGraph.metadata.duration,
      temporal_events: temporalEvents.metadata.duration
    }
  };

  fs.writeFileSync('dataset-summary.json', JSON.stringify(summary, null, 2));

  console.log('\n✓ All datasets generated and saved!');
  console.log('\nGenerated files:');
  console.log('- users.json (structured user profiles)');
  console.log('- projects.json (structured project data)');
  console.log('- metrics.json (time-series metrics)');
  console.log('- knowledge-graph.cypher (Neo4j graph with embeddings)');
  console.log('- temporal-events.cypher (Neo4j temporal events)');
  console.log('- dataset-summary.json (metadata and summary)');

  console.log('\n=== Integration Complete ===');
  console.log(`Total nodes in graphs: ${knowledgeGraph.data.nodes.length + temporalEvents.data.nodes.length}`);
  console.log(`Total edges in graphs: ${knowledgeGraph.data.edges.length + temporalEvents.data.edges.length}`);
  console.log(`Total structured records: ${userProfiles.data.length + projects.data.length}`);
  console.log(`Total time-series points: ${metrics.data.length}`);
}

main().catch(console.error);
