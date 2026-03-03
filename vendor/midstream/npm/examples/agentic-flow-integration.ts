/**
 * Example: Agentic Flow Proxy Integration
 *
 * Demonstrates using OpenAI Realtime API through agentic-flow proxy
 * with workflow orchestration and MidStream analysis
 */

import { AgenticFlowProxyClient, OpenAIRealtimeClient } from '../src/openai-realtime.js';
import * as dotenv from 'dotenv';

dotenv.config();

async function main() {
  console.log('🔄 Agentic Flow Proxy + OpenAI Realtime + MidStream');
  console.log('═══════════════════════════════════════════════════\n');

  // Create agentic-flow proxy client
  const proxyClient = new AgenticFlowProxyClient({
    baseUrl: process.env.AGENTIC_FLOW_PROXY_URL || 'https://api.agenticflow.com/v1',
    apiKey: process.env.AGENTIC_FLOW_API_KEY!,
    openAiApiKey: process.env.OPENAI_API_KEY!,
  });

  console.log('✓ Agentic Flow Proxy client created');

  // Create realtime session through proxy
  const realtimeClient = await proxyClient.createRealtimeSession({
    apiKey: process.env.OPENAI_API_KEY!,
    model: process.env.OPENAI_REALTIME_MODEL,
    voice: 'nova',
    temperature: 0.7,
  });

  console.log('✓ Realtime session created through proxy\n');

  // Set up event listeners
  realtimeClient.on('session.created', (session) => {
    console.log('📡 Session ID:', session.id);

    // Configure session
    realtimeClient.updateSession({
      modalities: ['text'],
      instructions: `You are an AI assistant integrated with agentic-flow for workflow orchestration.
        You can analyze conversations, detect patterns, and coordinate multi-agent workflows.`,
      temperature: 0.7,
    });
  });

  realtimeClient.on('response.text.delta', (delta) => {
    process.stdout.write(delta);
  });

  realtimeClient.on('response.done', () => {
    console.log('\n');
  });

  realtimeClient.on('midstream.analysis', (status) => {
    console.log('🧠 MidStream:', {
      messages: status.conversationHistorySize,
      avgReward: status.averageReward.toFixed(2),
    });
  });

  try {
    // Scenario 1: Simple conversation through proxy
    console.log('💬 Scenario 1: Proxied Conversation\n');
    console.log('User: Hello! I need help analyzing customer support patterns.\n');

    realtimeClient.sendText('Hello! I need help analyzing customer support patterns.');

    await new Promise(resolve => {
      realtimeClient.once('response.done', resolve);
    });

    await new Promise(resolve => setTimeout(resolve, 1000));

    // Scenario 2: Pattern analysis
    console.log('\nUser: Can you detect patterns in this conversation flow?\n');

    realtimeClient.sendText(`Can you analyze this conversation pattern:
      1. Customer: "I have a problem"
      2. Agent: "What's the issue?"
      3. Customer: "Can't login"
      4. Agent: "Let me help you reset your password"
      5. Customer: "Thank you, it works now"`);

    await new Promise(resolve => {
      realtimeClient.once('response.done', resolve);
    });

    await new Promise(resolve => setTimeout(resolve, 1000));

    // Get MidStream's pattern analysis
    console.log('\n📊 MidStream Pattern Analysis:');

    const agent = realtimeClient.getAgent();
    const testSequence = [
      'problem_report',
      'info_request',
      'problem_description',
      'solution_offer',
      'gratitude',
    ];

    const commonPattern = ['problem_report', 'info_request', 'problem_description'];
    const positions = agent.detectPattern(testSequence, commonPattern);

    console.log('  Pattern detected at positions:', positions);

    // Compare with another sequence
    const similarSequence = [
      'problem_report',
      'info_request',
      'problem_description',
      'solution_offer',
      'confirmation',
    ];

    const similarity = agent.compareSequences(testSequence, similarSequence, 'dtw');
    console.log('  Similarity to variant pattern:', similarity.toFixed(3));

    // Scenario 3: Workflow execution (if agentic-flow is configured)
    if (process.env.AGENTIC_FLOW_API_KEY) {
      console.log('\n🔄 Scenario 3: Workflow Orchestration\n');

      try {
        // Example workflow execution
        // In production, you'd have pre-configured workflows in agentic-flow
        const workflowResult = await proxyClient.executeWorkflow('conversation-analyzer', {
          conversation: realtimeClient.getConversation(),
          analysisType: 'pattern_detection',
        });

        console.log('Workflow result:', workflowResult);
      } catch (error: any) {
        console.log('  (Workflow not configured - this is expected in demo)');
      }
    }

    // Final comprehensive analysis
    console.log('\n═══════════════════════════════════════');
    console.log('📈 Final Comprehensive Analysis');
    console.log('═══════════════════════════════════════\n');

    const finalAnalysis = realtimeClient.getMidStreamAnalysis();
    console.log('MidStream Analysis:', JSON.stringify(finalAnalysis, null, 2));

    const status = agent.getStatus();
    console.log('\nAgent Status:');
    console.log('  - Conversation size:', status.conversationHistorySize);
    console.log('  - Average reward:', status.averageReward.toFixed(3));
    console.log('  - Meta-learning level:', status.metaLearning.currentLevel);

    // Behavior analysis
    if (status.rewardHistorySize > 5) {
      const behaviorAnalysis = agent.analyzeBehavior(
        Array(status.rewardHistorySize).fill(0.8)
      );

      console.log('\nBehavior Analysis:');
      console.log('  - Is stable:', behaviorAnalysis.isStable);
      console.log('  - Is chaotic:', behaviorAnalysis.isChaotic);
    }

    // Conversation insights
    const conversation = realtimeClient.getConversation();
    console.log('\nConversation Insights:');
    console.log('  - Total items:', conversation.length);
    console.log('  - User messages:', conversation.filter(i => i.role === 'user').length);
    console.log('  - Assistant messages:', conversation.filter(i => i.role === 'assistant').length);

    // Cleanup
    realtimeClient.disconnect();
    console.log('\n✓ Session ended gracefully');
    process.exit(0);
  } catch (error) {
    console.error('\n❌ Error:', error);
    realtimeClient.disconnect();
    process.exit(1);
  }
}

// Handle graceful shutdown
process.on('SIGINT', () => {
  console.log('\n\nShutting down...');
  process.exit(0);
});

main();
