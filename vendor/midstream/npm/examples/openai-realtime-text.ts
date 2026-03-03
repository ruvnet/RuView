/**
 * Example: OpenAI Realtime API with Text
 *
 * Demonstrates text-based conversation with OpenAI Realtime API
 * integrated with MidStream's temporal analysis
 */

import { OpenAIRealtimeClient, createDefaultSessionConfig } from '../src/openai-realtime.js';
import * as dotenv from 'dotenv';

dotenv.config();

async function main() {
  // Create client
  const client = new OpenAIRealtimeClient({
    apiKey: process.env.OPENAI_API_KEY!,
    model: process.env.OPENAI_REALTIME_MODEL,
    voice: 'alloy',
    temperature: 0.8,
  });

  // Set up event listeners
  client.on('connected', () => {
    console.log('✓ Connected to OpenAI Realtime API');
  });

  client.on('session.created', (session) => {
    console.log('✓ Session created:', session.id);

    // Update session config
    client.updateSession({
      ...createDefaultSessionConfig(),
      modalities: ['text'], // Text-only for this example
      instructions: 'You are a helpful assistant that analyzes conversations in real-time.',
    });
  });

  client.on('response.text.delta', (delta) => {
    process.stdout.write(delta);
  });

  client.on('response.text.done', (text) => {
    console.log('\n');
  });

  client.on('response.done', (response) => {
    console.log('✓ Response completed');

    // Get MidStream analysis
    const analysis = client.getMidStreamAnalysis();
    console.log('\n📊 MidStream Analysis:');
    console.log(`  - Messages analyzed: ${analysis.messageCount}`);
    console.log(`  - Meta-learning level: ${analysis.metaLearning.currentLevel}`);
  });

  client.on('midstream.analysis', (status) => {
    console.log('\n🧠 Real-time MidStream update:', {
      conversationSize: status.conversationHistorySize,
      averageReward: status.averageReward.toFixed(2),
    });
  });

  client.on('error', (error) => {
    console.error('❌ Error:', error.message);
  });

  client.on('disconnected', () => {
    console.log('✗ Disconnected from OpenAI');
  });

  // Connect
  try {
    await client.connect();

    // Simulate conversation
    console.log('\n💬 Starting conversation...\n');

    // Message 1
    console.log('User: Hello! Can you help me understand patterns in conversations?');
    client.sendText('Hello! Can you help me understand patterns in conversations?');

    // Wait for response
    await new Promise(resolve => {
      client.once('response.done', resolve);
    });

    await new Promise(resolve => setTimeout(resolve, 1000));

    // Message 2
    console.log('\nUser: What are some common conversation patterns?');
    client.sendText('What are some common conversation patterns?');

    await new Promise(resolve => {
      client.once('response.done', resolve);
    });

    await new Promise(resolve => setTimeout(resolve, 1000));

    // Message 3
    console.log('\nUser: Can you give me an example?');
    client.sendText('Can you give me an example?');

    await new Promise(resolve => {
      client.once('response.done', resolve);
    });

    // Final analysis
    await new Promise(resolve => setTimeout(resolve, 1000));

    console.log('\n\n═══════════════════════════════════════');
    console.log('📈 Final MidStream Analysis');
    console.log('═══════════════════════════════════════');

    const finalAnalysis = client.getMidStreamAnalysis();
    console.log(JSON.stringify(finalAnalysis, null, 2));

    const agent = client.getAgent();
    const status = agent.getStatus();

    console.log('\n📊 Agent Status:');
    console.log(`  - Conversation history: ${status.conversationHistorySize} messages`);
    console.log(`  - Average reward: ${status.averageReward.toFixed(2)}`);
    console.log(`  - Meta-learning: ${status.metaLearning.currentLevel}`);

    // Cleanup
    client.disconnect();
    process.exit(0);
  } catch (error) {
    console.error('❌ Fatal error:', error);
    process.exit(1);
  }
}

main();
