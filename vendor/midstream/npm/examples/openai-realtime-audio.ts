/**
 * Example: OpenAI Realtime API with Audio
 *
 * Demonstrates audio streaming with OpenAI Realtime API
 * and real-time transcription analysis
 */

import { OpenAIRealtimeClient, createDefaultSessionConfig, audioToBase64 } from '../src/openai-realtime.js';
import * as fs from 'fs';
import * as dotenv from 'dotenv';

dotenv.config();

async function main() {
  const client = new OpenAIRealtimeClient({
    apiKey: process.env.OPENAI_API_KEY!,
    model: process.env.OPENAI_REALTIME_MODEL,
    voice: 'alloy',
  });

  // Track transcriptions
  let userTranscript = '';
  let assistantTranscript = '';
  const audioChunks: string[] = [];

  // Event listeners
  client.on('connected', () => {
    console.log('✓ Connected to OpenAI Realtime API (Audio Mode)');
  });

  client.on('session.created', (session) => {
    console.log('✓ Session created:', session.id);

    // Configure for audio
    client.updateSession({
      ...createDefaultSessionConfig(),
      modalities: ['text', 'audio'],
      voice: 'alloy',
      instructions: 'You are a voice assistant that helps analyze conversation patterns.',
    });
  });

  // Handle transcriptions
  client.on('conversation.item.input_audio_transcription.completed', (data) => {
    userTranscript = data.transcript;
    console.log('\n🎤 User (transcribed):', userTranscript);
  });

  client.on('response.audio_transcript.delta', (delta) => {
    process.stdout.write(delta);
    assistantTranscript += delta;
  });

  client.on('response.audio_transcript.done', (transcript) => {
    console.log('\n');
    console.log('🔊 Assistant (transcript):', transcript);
  });

  // Handle audio chunks
  client.on('response.audio.delta', (delta) => {
    audioChunks.push(delta);
  });

  client.on('response.audio.done', (data) => {
    console.log('✓ Audio response complete');

    // Optionally save audio to file
    if (audioChunks.length > 0) {
      const audioData = Buffer.from(audioChunks.join(''), 'base64');
      fs.writeFileSync('response_audio.pcm', audioData);
      console.log('  → Audio saved to response_audio.pcm');
      audioChunks.length = 0;
    }
  });

  client.on('response.done', () => {
    console.log('✓ Response completed\n');

    // MidStream analysis
    const analysis = client.getMidStreamAnalysis();
    console.log('📊 Conversation Analysis:', {
      messages: analysis.messageCount,
      patterns: analysis.patterns?.length || 0,
    });
  });

  client.on('error', (error) => {
    console.error('❌ Error:', error.message);
  });

  // Connect
  try {
    await client.connect();

    console.log('\n🎙️  Audio Mode Demonstration');
    console.log('═══════════════════════════════════════\n');

    // For demo purposes, we'll send text and receive audio
    // In a real app, you'd stream audio from a microphone

    // Demo 1: Send text, receive audio
    console.log('Sending text message (will receive audio response)...\n');
    client.sendText('Hello! Please tell me about conversation patterns.');

    await new Promise(resolve => {
      client.once('response.done', resolve);
    });

    await new Promise(resolve => setTimeout(resolve, 2000));

    // Demo 2: Simulate audio input (in real app, this would be mic audio)
    console.log('Simulating audio input...\n');

    // In a real application, you would:
    // 1. Capture audio from microphone in PCM16 format
    // 2. Convert to base64
    // 3. Send chunks via client.sendAudio()
    // 4. Commit when done speaking

    // For this demo, we'll send another text message
    client.sendText('Can you explain Dynamic Time Warping?');

    await new Promise(resolve => {
      client.once('response.done', resolve);
    });

    // Final analysis
    await new Promise(resolve => setTimeout(resolve, 1000));

    console.log('\n═══════════════════════════════════════');
    console.log('📈 Final Analysis');
    console.log('═══════════════════════════════════════\n');

    const conversation = client.getConversation();
    console.log(`Total conversation items: ${conversation.length}`);

    const agent = client.getAgent();
    const status = agent.getStatus();

    console.log('\n📊 MidStream Metrics:');
    console.log(`  - Messages processed: ${status.conversationHistorySize}`);
    console.log(`  - Reward history: ${status.rewardHistorySize}`);
    console.log(`  - Average reward: ${status.averageReward.toFixed(3)}`);

    // Cleanup
    client.disconnect();
    process.exit(0);
  } catch (error) {
    console.error('❌ Fatal error:', error);
    process.exit(1);
  }
}

// Handle graceful shutdown
process.on('SIGINT', () => {
  console.log('\n\nShutting down...');
  process.exit(0);
});

main();
