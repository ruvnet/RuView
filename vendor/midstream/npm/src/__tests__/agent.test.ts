/**
 * Tests for MidStream Agent
 */

import { MidStreamAgent } from '../agent';

describe('MidStreamAgent', () => {
  let agent: MidStreamAgent;

  beforeEach(() => {
    agent = new MidStreamAgent();
  });

  afterEach(() => {
    agent.reset();
  });

  describe('processMessage', () => {
    it('should process a single message', () => {
      const result = agent.processMessage('Hello, world!');
      expect(result).toBeDefined();
      expect(result.processed).toBe(true);
    });

    it('should maintain conversation history', () => {
      agent.processMessage('First message');
      agent.processMessage('Second message');

      const status = agent.getStatus();
      expect(status.conversationHistorySize).toBe(2);
    });

    it('should respect max history limit', () => {
      const smallAgent = new MidStreamAgent({ maxHistory: 5 });

      for (let i = 0; i < 10; i++) {
        smallAgent.processMessage(`Message ${i}`);
      }

      const status = smallAgent.getStatus();
      expect(status.conversationHistorySize).toBeLessThanOrEqual(5);
    });
  });

  describe('analyzeConversation', () => {
    it('should analyze a conversation', () => {
      const messages = [
        'Hello',
        'How are you?',
        'What is the weather?',
      ];

      const result = agent.analyzeConversation(messages);

      expect(result).toBeDefined();
      expect(result.messageCount).toBe(3);
      expect(result.patterns).toBeDefined();
      expect(result.metaLearning).toBeDefined();
    });

    it('should handle empty conversation', () => {
      const result = agent.analyzeConversation([]);
      expect(result.messageCount).toBe(0);
    });
  });

  describe('compareSequences', () => {
    it('should compare identical sequences', () => {
      const seq = ['a', 'b', 'c'];
      const similarity = agent.compareSequences(seq, seq, 'dtw');

      expect(similarity).toBeGreaterThan(0.9);
    });

    it('should compare different sequences', () => {
      const seq1 = ['a', 'b', 'c'];
      const seq2 = ['x', 'y', 'z'];

      const similarity = agent.compareSequences(seq1, seq2, 'dtw');

      expect(similarity).toBeLessThan(0.5);
    });

    it('should detect similarity in overlapping sequences', () => {
      const seq1 = ['a', 'b', 'c', 'd'];
      const seq2 = ['b', 'c', 'd', 'e'];

      const similarity = agent.compareSequences(seq1, seq2, 'dtw');

      expect(similarity).toBeGreaterThan(0.5);
    });
  });

  describe('detectPattern', () => {
    it('should detect pattern occurrences', () => {
      const sequence = ['a', 'b', 'c', 'a', 'b', 'c', 'd', 'a', 'b', 'c'];
      const pattern = ['a', 'b', 'c'];

      const positions = agent.detectPattern(sequence, pattern);

      expect(positions).toEqual([0, 3, 7]);
    });

    it('should return empty array for non-existent pattern', () => {
      const sequence = ['a', 'b', 'c'];
      const pattern = ['x', 'y'];

      const positions = agent.detectPattern(sequence, pattern);

      expect(positions).toEqual([]);
    });

    it('should handle empty pattern', () => {
      const sequence = ['a', 'b', 'c'];
      const pattern: string[] = [];

      const positions = agent.detectPattern(sequence, pattern);

      expect(positions).toEqual([]);
    });
  });

  describe('analyzeBehavior', () => {
    it('should detect stable behavior', () => {
      const stableRewards = [0.8, 0.81, 0.79, 0.80, 0.81];
      const analysis = agent.analyzeBehavior(stableRewards);

      expect(analysis.isStable).toBe(true);
      expect(analysis.isChaotic).toBe(false);
    });

    it('should detect chaotic behavior', () => {
      const chaoticRewards = [0.1, 0.9, 0.2, 0.8, 0.3, 0.7];
      const analysis = agent.analyzeBehavior(chaoticRewards);

      expect(analysis.isChaotic).toBe(true);
      expect(analysis.isStable).toBe(false);
    });
  });

  describe('learn', () => {
    it('should perform meta-learning', () => {
      agent.learn('Pattern A works well', 0.9);
      agent.learn('Pattern B is suboptimal', 0.3);

      const summary = agent.getMetaLearningSummary();

      expect(summary).toBeDefined();
    });
  });

  describe('getStatus', () => {
    it('should return agent status', () => {
      agent.processMessage('Test message');
      agent.learn('Test learning', 0.8);

      const status = agent.getStatus();

      expect(status.conversationHistorySize).toBeGreaterThan(0);
      expect(status.rewardHistorySize).toBeGreaterThan(0);
      expect(status.config).toBeDefined();
      expect(status.metaLearning).toBeDefined();
    });

    it('should calculate average reward', () => {
      agent.learn('A', 0.8);
      agent.learn('B', 0.6);
      agent.learn('C', 0.9);

      const status = agent.getStatus();

      expect(status.averageReward).toBeCloseTo((0.8 + 0.6 + 0.9) / 3, 2);
    });
  });

  describe('reset', () => {
    it('should clear all history', () => {
      agent.processMessage('Test');
      agent.learn('Test', 0.8);

      agent.reset();

      const status = agent.getStatus();

      expect(status.conversationHistorySize).toBe(0);
      expect(status.rewardHistorySize).toBe(0);
    });
  });
});
