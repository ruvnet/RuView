/**
 * Integration tests for MidStream
 */

import { MidStreamAgent } from '../agent';
import { WebSocketStreamServer, SSEStreamServer } from '../streaming';
import * as fs from 'fs';
import * as path from 'path';

describe('MidStream Integration Tests', () => {
  let agent: MidStreamAgent;

  beforeAll(() => {
    agent = new MidStreamAgent({
      maxHistory: 500,
      embeddingDim: 3,
    });
  });

  describe('End-to-End Conversation Analysis', () => {
    it('should process and analyze a complete conversation', () => {
      const conversation = [
        "Hello, I need help with the weather.",
        "Of course! Which city are you interested in?",
        "San Francisco please.",
        "The weather in San Francisco is currently 65°F and partly cloudy.",
        "Perfect, thank you!",
      ];

      // Process each message
      conversation.forEach(msg => {
        agent.processMessage(msg);
      });

      // Analyze the complete conversation
      const analysis = agent.analyzeConversation(conversation);

      expect(analysis).toBeDefined();
      expect(analysis.messageCount).toBe(5);
      expect(analysis.patterns).toBeDefined();
      expect(analysis.metaLearning).toBeDefined();

      // Check status
      const status = agent.getStatus();
      expect(status.conversationHistorySize).toBeGreaterThan(0);
    });

    it('should detect patterns in conversation flow', () => {
      const sequence = [
        'greeting',
        'weather_query',
        'location_query',
        'weather_response',
        'thanks',
      ];

      const pattern = ['weather_query', 'location_query'];

      const positions = agent.detectPattern(sequence, pattern);

      expect(positions.length).toBeGreaterThan(0);
      expect(positions[0]).toBe(1); // Pattern starts at index 1
    });
  });

  describe('Temporal Sequence Comparison', () => {
    it('should compare similar conversation patterns', () => {
      const pattern1 = [
        'greeting',
        'weather_query',
        'location_query',
        'response',
      ];

      const pattern2 = [
        'greeting',
        'weather_query',
        'location_query',
        'detailed_response',
      ];

      const similarity = agent.compareSequences(pattern1, pattern2, 'lcs');

      expect(similarity).toBeGreaterThan(0.7); // High similarity
    });

    it('should detect different conversation patterns', () => {
      const weatherPattern = [
        'greeting',
        'weather_query',
        'location',
        'response',
      ];

      const accountPattern = [
        'greeting',
        'account_query',
        'credentials',
        'verification',
      ];

      const similarity = agent.compareSequences(weatherPattern, accountPattern, 'dtw');

      expect(similarity).toBeLessThan(0.5); // Low similarity
    });
  });

  describe('Behavior Stability Analysis', () => {
    it('should detect stable learning behavior', () => {
      const stableRewards = Array(20).fill(0).map((_, i) =>
        0.8 + Math.sin(i * 0.1) * 0.05 // Stable with small oscillation
      );

      const analysis = agent.analyzeBehavior(stableRewards);

      expect(analysis.isStable).toBe(true);
      expect(analysis.isChaotic).toBe(false);
    });

    it('should detect chaotic behavior', () => {
      const chaoticRewards = Array(20).fill(0).map(() =>
        Math.random() // Completely random
      );

      const analysis = agent.analyzeBehavior(chaoticRewards);

      // Chaotic patterns should be detected
      expect(analysis.isChaotic).toBe(true);
    });
  });

  describe('Meta-Learning Progression', () => {
    it('should demonstrate meta-learning over multiple interactions', () => {
      agent.reset(); // Start fresh

      // Simulate learning from successful patterns
      for (let i = 0; i < 10; i++) {
        agent.learn(`Pattern ${i} is successful`, 0.85);
      }

      // Simulate learning from unsuccessful patterns
      for (let i = 0; i < 5; i++) {
        agent.learn(`Pattern ${i} failed`, 0.2);
      }

      const summary = agent.getMetaLearningSummary();

      expect(summary).toBeDefined();
      expect(summary.currentLevel).toBeDefined();

      const status = agent.getStatus();
      expect(status.averageReward).toBeGreaterThan(0);
      expect(status.rewardHistorySize).toBe(15);
    });
  });

  describe('Real-World Scenario: Customer Support', () => {
    it('should handle a customer support conversation', () => {
      const conversation = [
        'Hi, I have a problem with my order',
        'I apologize for the inconvenience. Can you provide your order number?',
        'Sure, it\'s ORDER-12345',
        'Thank you. I see your order was shipped yesterday. It should arrive in 2-3 days.',
        'Oh, I see. When can I expect tracking information?',
        'Tracking information has been sent to your email. Check your inbox.',
        'Found it! Thank you so much for your help.',
        'You\'re welcome! Is there anything else I can help you with?',
        'No, that\'s all. Have a great day!',
      ];

      // Process conversation
      const analysis = agent.analyzeConversation(conversation);

      expect(analysis.messageCount).toBe(9);

      // Extract intent flow
      const intents = [
        'problem_report',
        'info_request',
        'info_provided',
        'status_update',
        'followup_question',
        'solution_provided',
        'gratitude',
        'offer_help',
        'closure',
      ];

      // Check for common support patterns
      const supportPattern = ['problem_report', 'info_request', 'info_provided'];
      const positions = agent.detectPattern(intents, supportPattern);

      expect(positions.length).toBeGreaterThan(0);
    });
  });

  describe('Performance Benchmarking', () => {
    it('should process messages quickly', () => {
      const start = Date.now();

      for (let i = 0; i < 100; i++) {
        agent.processMessage(`Test message ${i}`);
      }

      const duration = Date.now() - start;

      // Should process 100 messages in under 1 second
      expect(duration).toBeLessThan(1000);

      const avgTime = duration / 100;
      console.log(`Average message processing time: ${avgTime.toFixed(2)}ms`);
    });

    it('should handle large conversations efficiently', () => {
      const largeConversation = Array(500).fill(0).map((_, i) =>
        `Message number ${i} in a very large conversation`
      );

      const start = Date.now();
      const analysis = agent.analyzeConversation(largeConversation);
      const duration = Date.now() - start;

      expect(analysis.messageCount).toBe(500);

      // Should analyze 500 messages in under 500ms
      expect(duration).toBeLessThan(500);

      console.log(`Large conversation analysis time: ${duration}ms`);
    });
  });

  describe('Streaming Server Integration', () => {
    let wsServer: WebSocketStreamServer;
    let sseServer: SSEStreamServer;

    beforeAll(async () => {
      // Use non-standard ports for testing
      wsServer = new WebSocketStreamServer(9001);
      sseServer = new SSEStreamServer(9002);

      await wsServer.start();
      await sseServer.start();
    });

    afterAll(async () => {
      await wsServer.stop();
      await sseServer.stop();
    });

    it('should start WebSocket server', () => {
      expect(wsServer).toBeDefined();
    });

    it('should start SSE server', () => {
      expect(sseServer).toBeDefined();
    });

    it('should broadcast to WebSocket clients', () => {
      const testData = {
        type: 'test',
        message: 'Hello from test',
      };

      // Should not throw
      expect(() => wsServer.broadcast(testData)).not.toThrow();
    });

    it('should broadcast to SSE clients', () => {
      const testData = {
        type: 'test',
        message: 'Hello from SSE test',
      };

      // Should not throw
      expect(() => sseServer.broadcast(testData)).not.toThrow();
    });
  });

  describe('File-based Examples', () => {
    const examplesDir = path.join(__dirname, '../../examples');

    it('should process example conversation1.json', () => {
      const filePath = path.join(examplesDir, 'conversation1.json');

      if (fs.existsSync(filePath)) {
        const messages = JSON.parse(fs.readFileSync(filePath, 'utf-8'));
        const analysis = agent.analyzeConversation(messages);

        expect(analysis.messageCount).toBeGreaterThan(0);
        expect(analysis.patterns).toBeDefined();
      }
    });

    it('should compare example sequences', () => {
      const seq1Path = path.join(examplesDir, 'sequence1.json');
      const seq2Path = path.join(examplesDir, 'sequence2.json');

      if (fs.existsSync(seq1Path) && fs.existsSync(seq2Path)) {
        const seq1 = JSON.parse(fs.readFileSync(seq1Path, 'utf-8'));
        const seq2 = JSON.parse(fs.readFileSync(seq2Path, 'utf-8'));

        const similarity = agent.compareSequences(seq1, seq2, 'dtw');

        expect(similarity).toBeGreaterThanOrEqual(0);
        expect(similarity).toBeLessThanOrEqual(1);
      }
    });
  });

  describe('Edge Cases and Error Handling', () => {
    it('should handle empty messages', () => {
      expect(() => agent.processMessage('')).not.toThrow();
    });

    it('should handle very long messages', () => {
      const longMessage = 'a'.repeat(10000);
      expect(() => agent.processMessage(longMessage)).not.toThrow();
    });

    it('should handle empty conversation analysis', () => {
      const result = agent.analyzeConversation([]);
      expect(result.messageCount).toBe(0);
    });

    it('should handle single message conversation', () => {
      const result = agent.analyzeConversation(['Hello']);
      expect(result.messageCount).toBe(1);
    });

    it('should handle empty sequences in comparison', () => {
      const similarity = agent.compareSequences([], [], 'dtw');
      expect(similarity).toBeGreaterThanOrEqual(0);
    });

    it('should handle empty rewards in behavior analysis', () => {
      const analysis = agent.analyzeBehavior([]);
      expect(analysis).toBeDefined();
    });
  });

  describe('Memory Management', () => {
    it('should respect max history limit', () => {
      const smallAgent = new MidStreamAgent({ maxHistory: 10 });

      // Add more than max history
      for (let i = 0; i < 50; i++) {
        smallAgent.processMessage(`Message ${i}`);
      }

      const status = smallAgent.getStatus();
      expect(status.conversationHistorySize).toBeLessThanOrEqual(10);
    });

    it('should successfully reset state', () => {
      // Add some data
      agent.processMessage('Test');
      agent.learn('Test', 0.8);

      // Reset
      agent.reset();

      // Verify clean state
      const status = agent.getStatus();
      expect(status.conversationHistorySize).toBe(0);
      expect(status.rewardHistorySize).toBe(0);
    });
  });
});
