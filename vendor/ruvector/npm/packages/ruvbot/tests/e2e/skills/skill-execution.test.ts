/**
 * Skill Execution - E2E Tests
 *
 * End-to-end tests for skill execution flows
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { createSkill } from '../../factories';
import { createMockSlackApp, type MockSlackBoltApp } from '../../mocks/slack.mock';
import { createMockRuVectorBindings } from '../../mocks/wasm.mock';

// Skill execution types
interface SkillInput {
  skill: string;
  params: Record<string, unknown>;
}

interface SkillOutput {
  success: boolean;
  result: unknown;
  error?: string;
  executionTime: number;
}

// Mock Skill Executor
class MockSkillExecutor {
  private skills: Map<string, {
    handler: (params: Record<string, unknown>) => Promise<unknown>;
    timeout: number;
  }> = new Map();

  registerSkill(
    name: string,
    handler: (params: Record<string, unknown>) => Promise<unknown>,
    timeout: number = 30000
  ): void {
    this.skills.set(name, { handler, timeout });
  }

  async execute(input: SkillInput): Promise<SkillOutput> {
    const skill = this.skills.get(input.skill);
    if (!skill) {
      return {
        success: false,
        result: null,
        error: `Skill '${input.skill}' not found`,
        executionTime: 0
      };
    }

    const startTime = Date.now();

    try {
      const result = await Promise.race([
        skill.handler(input.params),
        this.createTimeout(skill.timeout)
      ]);

      return {
        success: true,
        result,
        executionTime: Date.now() - startTime
      };
    } catch (error) {
      return {
        success: false,
        result: null,
        error: error instanceof Error ? error.message : 'Unknown error',
        executionTime: Date.now() - startTime
      };
    }
  }

  private createTimeout(ms: number): Promise<never> {
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Skill execution timed out')), ms);
    });
  }
}

// Mock Skill-enabled Bot
class MockSkillBot {
  private app: MockSlackBoltApp;
  private executor: MockSkillExecutor;
  private ruvector: ReturnType<typeof createMockRuVectorBindings>;

  constructor() {
    this.app = createMockSlackApp();
    this.executor = new MockSkillExecutor();
    this.ruvector = createMockRuVectorBindings();
    this.registerSkills();
    this.setupHandlers();
  }

  getApp(): MockSlackBoltApp {
    return this.app;
  }

  getExecutor(): MockSkillExecutor {
    return this.executor;
  }

  async processMessage(message: {
    text: string;
    channel: string;
    user: string;
    ts: string;
    thread_ts?: string;
  }): Promise<void> {
    await this.app.processMessage(message);
  }

  private registerSkills(): void {
    // Code generation skill
    this.executor.registerSkill('code-generation', async (params) => {
      const { language, description } = params;
      await new Promise(resolve => setTimeout(resolve, 50)); // Simulate processing

      const templates: Record<string, string> = {
        javascript: `// ${description}\nfunction example() {\n  // Implementation\n}`,
        python: `# ${description}\ndef example():\n    # Implementation\n    pass`,
        typescript: `// ${description}\nfunction example(): void {\n  // Implementation\n}`
      };

      return {
        code: templates[language as string] || templates.javascript,
        language
      };
    });

    // Test generation skill
    this.executor.registerSkill('test-generation', async (params) => {
      const { code, framework } = params;
      await new Promise(resolve => setTimeout(resolve, 50));

      return {
        tests: `describe('Generated Tests', () => {\n  it('should work', () => {\n    expect(true).toBe(true);\n  });\n});`,
        framework: framework || 'jest',
        coverage: 85
      };
    });

    // Vector search skill
    this.executor.registerSkill('vector-search', async (params) => {
      const { query, topK } = params;
      const results = await this.ruvector.search(query as string, topK as number || 5);

      return {
        results,
        query,
        count: results.length
      };
    });

    // Code review skill
    this.executor.registerSkill('code-review', async (params) => {
      const { code } = params;
      await new Promise(resolve => setTimeout(resolve, 100));

      return {
        issues: [
          { type: 'warning', message: 'Consider adding error handling', line: 5 },
          { type: 'suggestion', message: 'Variable could be const', line: 2 }
        ],
        score: 85,
        summary: 'Code looks good with minor improvements suggested'
      };
    });

    // Documentation skill
    this.executor.registerSkill('generate-docs', async (params) => {
      const { code, format } = params;
      await new Promise(resolve => setTimeout(resolve, 75));

      return {
        documentation: `## Function Documentation\n\nThis function does something useful.\n\n### Parameters\n- param1: Description`,
        format: format || 'markdown'
      };
    });
  }

  private setupHandlers(): void {
    // Handle code generation
    this.app.message(/generate.*code.*in\s+(\w+)/i, async ({ message, say }) => {
      const languageMatch = (message as any).text.match(/in\s+(\w+)/i);
      const language = languageMatch ? languageMatch[1].toLowerCase() : 'javascript';

      await say({
        channel: (message as any).channel,
        text: `Generating ${language} code...`,
        thread_ts: (message as any).ts
      });

      const result = await this.executor.execute({
        skill: 'code-generation',
        params: {
          language,
          description: (message as any).text
        }
      });

      if (result.success) {
        const output = result.result as { code: string };
        await say({
          channel: (message as any).channel,
          text: `\`\`\`${language}\n${output.code}\n\`\`\``,
          thread_ts: (message as any).ts
        });
      } else {
        await say({
          channel: (message as any).channel,
          text: `Error: ${result.error}`,
          thread_ts: (message as any).ts
        });
      }
    });

    // Handle test generation
    this.app.message(/generate.*tests?|write.*tests?/i, async ({ message, say }) => {
      await say({
        channel: (message as any).channel,
        text: 'Generating tests...',
        thread_ts: (message as any).ts
      });

      const result = await this.executor.execute({
        skill: 'test-generation',
        params: {
          code: 'function example() {}',
          framework: 'vitest'
        }
      });

      if (result.success) {
        const output = result.result as { tests: string; coverage: number };
        await say({
          channel: (message as any).channel,
          text: `\`\`\`typescript\n${output.tests}\n\`\`\`\nEstimated coverage: ${output.coverage}%`,
          thread_ts: (message as any).ts
        });
      }
    });

    // Handle code review
    this.app.message(/review.*code|check.*code/i, async ({ message, say }) => {
      await say({
        channel: (message as any).channel,
        text: 'Reviewing code...',
        thread_ts: (message as any).ts
      });

      const result = await this.executor.execute({
        skill: 'code-review',
        params: {
          code: '// Sample code for review'
        }
      });

      if (result.success) {
        const output = result.result as { summary: string; score: number; issues: unknown[] };
        await say({
          channel: (message as any).channel,
          text: `Code Review Results:\n- Score: ${output.score}/100\n- Issues: ${output.issues.length}\n\n${output.summary}`,
          thread_ts: (message as any).ts
        });
      }
    });

    // Handle search
    this.app.message(/search.*for|find.*about/i, async ({ message, say }) => {
      const result = await this.executor.execute({
        skill: 'vector-search',
        params: {
          query: (message as any).text,
          topK: 5
        }
      });

      if (result.success) {
        const output = result.result as { count: number };
        await say({
          channel: (message as any).channel,
          text: `Found ${output.count} results`,
          thread_ts: (message as any).ts
        });
      }
    });

    // Handle documentation
    this.app.message(/generate.*docs|document.*this/i, async ({ message, say }) => {
      await say({
        channel: (message as any).channel,
        text: 'Generating documentation...',
        thread_ts: (message as any).ts
      });

      const result = await this.executor.execute({
        skill: 'generate-docs',
        params: {
          code: 'function example() {}',
          format: 'markdown'
        }
      });

      if (result.success) {
        const output = result.result as { documentation: string };
        await say({
          channel: (message as any).channel,
          text: output.documentation,
          thread_ts: (message as any).ts
        });
      }
    });
  }
}

describe('E2E: Skill Execution', () => {
  let bot: MockSkillBot;

  beforeEach(() => {
    bot = new MockSkillBot();
  });

  describe('Code Generation Skill', () => {
    it('should generate JavaScript code', async () => {
      await bot.processMessage({
        text: 'Generate code in JavaScript for a hello world function',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      const messages = bot.getApp().client.getMessageLog();
      expect(messages.some(m => m.text?.includes('Generating'))).toBe(true);
      expect(messages.some(m => m.text?.includes('```javascript'))).toBe(true);
    });

    it('should generate Python code', async () => {
      await bot.processMessage({
        text: 'Generate code in Python for data processing',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      const messages = bot.getApp().client.getMessageLog();
      expect(messages.some(m => m.text?.includes('```python'))).toBe(true);
    });

    it('should generate TypeScript code', async () => {
      await bot.processMessage({
        text: 'Generate code in TypeScript for a type-safe function',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      const messages = bot.getApp().client.getMessageLog();
      expect(messages.some(m => m.text?.includes('```typescript'))).toBe(true);
    });
  });

  describe('Test Generation Skill', () => {
    it('should generate tests', async () => {
      await bot.processMessage({
        text: 'Generate tests for this function',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      const messages = bot.getApp().client.getMessageLog();
      expect(messages.some(m => m.text?.includes('describe'))).toBe(true);
      expect(messages.some(m => m.text?.includes('coverage'))).toBe(true);
    });
  });

  describe('Code Review Skill', () => {
    it('should review code and provide feedback', async () => {
      await bot.processMessage({
        text: 'Review this code for me',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      const messages = bot.getApp().client.getMessageLog();
      expect(messages.some(m => m.text?.includes('Review Results'))).toBe(true);
      expect(messages.some(m => m.text?.includes('Score'))).toBe(true);
    });
  });

  describe('Vector Search Skill', () => {
    it('should search and return results', async () => {
      await bot.processMessage({
        text: 'Search for React patterns',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      const messages = bot.getApp().client.getMessageLog();
      expect(messages.some(m => m.text?.includes('results'))).toBe(true);
    });
  });

  describe('Documentation Skill', () => {
    it('should generate documentation', async () => {
      await bot.processMessage({
        text: 'Generate docs for this function',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      const messages = bot.getApp().client.getMessageLog();
      expect(messages.some(m => m.text?.includes('Documentation'))).toBe(true);
    });
  });
});

describe('E2E: Skill Chaining', () => {
  let executor: MockSkillExecutor;

  beforeEach(() => {
    executor = new MockSkillExecutor();

    // Register skills for chaining
    executor.registerSkill('analyze', async (params) => {
      return { analyzed: true, data: params.input };
    });

    executor.registerSkill('transform', async (params) => {
      return { transformed: true, original: params.data };
    });

    executor.registerSkill('output', async (params) => {
      return { result: `Processed: ${JSON.stringify(params.transformed)}` };
    });
  });

  it('should chain multiple skills together', async () => {
    // Step 1: Analyze
    const step1 = await executor.execute({
      skill: 'analyze',
      params: { input: 'raw data' }
    });
    expect(step1.success).toBe(true);

    // Step 2: Transform
    const step2 = await executor.execute({
      skill: 'transform',
      params: { data: step1.result }
    });
    expect(step2.success).toBe(true);

    // Step 3: Output
    const step3 = await executor.execute({
      skill: 'output',
      params: { transformed: step2.result }
    });
    expect(step3.success).toBe(true);
    expect((step3.result as any).result).toContain('Processed');
  });
});

describe('E2E: Skill Error Handling', () => {
  let executor: MockSkillExecutor;

  beforeEach(() => {
    executor = new MockSkillExecutor();

    executor.registerSkill('failing-skill', async () => {
      throw new Error('Skill failed intentionally');
    });

    executor.registerSkill('slow-skill', async () => {
      await new Promise(resolve => setTimeout(resolve, 5000));
      return { result: 'Should not reach' };
    }, 100); // 100ms timeout
  });

  it('should handle skill errors gracefully', async () => {
    const result = await executor.execute({
      skill: 'failing-skill',
      params: {}
    });

    expect(result.success).toBe(false);
    expect(result.error).toBe('Skill failed intentionally');
  });

  it('should handle skill timeout', async () => {
    const result = await executor.execute({
      skill: 'slow-skill',
      params: {}
    });

    expect(result.success).toBe(false);
    expect(result.error).toContain('timed out');
  });

  it('should handle non-existent skill', async () => {
    const result = await executor.execute({
      skill: 'non-existent',
      params: {}
    });

    expect(result.success).toBe(false);
    expect(result.error).toContain('not found');
  });
});

describe('E2E: Skill Execution Metrics', () => {
  let executor: MockSkillExecutor;

  beforeEach(() => {
    executor = new MockSkillExecutor();

    executor.registerSkill('timed-skill', async (params) => {
      const delay = (params.delay as number) || 50;
      await new Promise(resolve => setTimeout(resolve, delay));
      return { executed: true };
    });
  });

  it('should track execution time', async () => {
    const result = await executor.execute({
      skill: 'timed-skill',
      params: { delay: 100 }
    });

    expect(result.executionTime).toBeGreaterThanOrEqual(100);
    expect(result.executionTime).toBeLessThan(200);
  });

  it('should report zero execution time for immediate failures', async () => {
    const result = await executor.execute({
      skill: 'non-existent',
      params: {}
    });

    expect(result.executionTime).toBe(0);
  });
});
