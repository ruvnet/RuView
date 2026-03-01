/**
 * Code Analysis Skill - Analyze, explain, and generate code
 *
 * Provides:
 * - Code explanation
 * - Bug detection hints
 * - Code generation
 * - Language detection
 */

import type {
  Skill,
  SkillExecutionContext,
  SkillStep,
  SkillExecutionResult,
} from '../../core/skill/index.js';

export const CodeExplainSkill: Skill = {
  id: 'code-explain',
  name: 'Explain Code',
  description: 'Analyze and explain code snippets',
  version: '1.0.0',
  triggers: [
    { type: 'keyword', value: 'explain this code', confidence: 0.95 },
    { type: 'keyword', value: 'what does this code do', confidence: 0.9 },
    { type: 'keyword', value: 'analyze code', confidence: 0.85 },
    { type: 'keyword', value: 'how does this work', confidence: 0.6 },
    { type: 'pattern', value: '```[\\s\\S]*```', confidence: 0.7 },
    { type: 'intent', value: 'code_explain', confidence: 0.95 },
  ],
  parameters: {
    type: 'object',
    properties: {
      code: {
        type: 'string',
        description: 'The code to explain',
      },
      language: {
        type: 'string',
        description: 'Programming language (auto-detected if not provided)',
      },
      detail: {
        type: 'string',
        description: 'Level of detail: brief, normal, detailed',
        default: 'normal',
      },
    },
    required: ['code'],
  },
  execute: codeExplainExecutor,
};

export const CodeGenerateSkill: Skill = {
  id: 'code-generate',
  name: 'Generate Code',
  description: 'Generate code from natural language description',
  version: '1.0.0',
  triggers: [
    { type: 'keyword', value: 'write code', confidence: 0.9 },
    { type: 'keyword', value: 'generate code', confidence: 0.9 },
    { type: 'keyword', value: 'create function', confidence: 0.85 },
    { type: 'keyword', value: 'implement', confidence: 0.6 },
    { type: 'keyword', value: 'code for', confidence: 0.7 },
    { type: 'intent', value: 'code_generate', confidence: 0.95 },
  ],
  parameters: {
    type: 'object',
    properties: {
      description: {
        type: 'string',
        description: 'Description of what the code should do',
      },
      language: {
        type: 'string',
        description: 'Target programming language',
        default: 'typescript',
      },
      style: {
        type: 'string',
        description: 'Code style: concise, documented, production',
        default: 'documented',
      },
    },
    required: ['description'],
  },
  execute: codeGenerateExecutor,
};

// Language detection patterns
const LANGUAGE_PATTERNS: Array<{
  language: string;
  patterns: RegExp[];
}> = [
  {
    language: 'python',
    patterns: [/\bdef\s+\w+\s*\(/, /\bimport\s+\w+/, /\bclass\s+\w+:/, /print\(/],
  },
  {
    language: 'javascript',
    patterns: [/\bfunction\s+\w+\s*\(/, /\bconst\s+\w+\s*=/, /\blet\s+\w+\s*=/, /=>\s*\{/],
  },
  {
    language: 'typescript',
    patterns: [/:\s*(string|number|boolean|any)\b/, /\binterface\s+\w+/, /\btype\s+\w+\s*=/],
  },
  {
    language: 'rust',
    patterns: [/\bfn\s+\w+\s*\(/, /\blet\s+mut\s+/, /\bimpl\s+/, /->\s*(Self|&)/],
  },
  {
    language: 'go',
    patterns: [/\bfunc\s+\w+\s*\(/, /\bpackage\s+\w+/, /\bgo\s+\w+\(/],
  },
  {
    language: 'java',
    patterns: [/\bpublic\s+class\s+/, /\bprivate\s+\w+\s+\w+/, /\bSystem\.out\.print/],
  },
  {
    language: 'cpp',
    patterns: [/#include\s*</, /\bstd::/, /\bint\s+main\s*\(/],
  },
  {
    language: 'html',
    patterns: [/<html/, /<div/, /<\/\w+>/],
  },
  {
    language: 'css',
    patterns: [/\{\s*[\w-]+\s*:/, /@media\s+/, /\.([\w-]+)\s*\{/],
  },
  {
    language: 'sql',
    patterns: [/\bSELECT\s+/i, /\bFROM\s+/i, /\bWHERE\s+/i, /\bINSERT\s+INTO\s+/i],
  },
];

function detectLanguage(code: string): string {
  const scores: Record<string, number> = {};

  for (const { language, patterns } of LANGUAGE_PATTERNS) {
    scores[language] = 0;
    for (const pattern of patterns) {
      if (pattern.test(code)) {
        scores[language]++;
      }
    }
  }

  const sorted = Object.entries(scores).sort((a, b) => b[1] - a[1]);
  if (sorted.length > 0 && sorted[0][1] > 0) {
    return sorted[0][0];
  }

  return 'unknown';
}

function getCodeStats(code: string): { lines: number; chars: number; functions: number } {
  const lines = code.split('\n').length;
  const chars = code.length;
  const functionMatches = code.match(/\b(function|def|fn|func)\s+\w+/g);
  const functions = functionMatches ? functionMatches.length : 0;

  return { lines, chars, functions };
}

async function* codeExplainExecutor(
  context: SkillExecutionContext,
  params: Record<string, unknown>
): AsyncGenerator<SkillStep, SkillExecutionResult, void> {
  const code = params.code as string;
  let language = params.language as string | undefined;
  const detail = (params.detail as string) || 'normal';

  yield {
    type: 'message',
    content: 'Analyzing code...',
  };

  yield {
    type: 'progress',
    progress: 30,
  };

  // Detect language if not provided
  if (!language) {
    language = detectLanguage(code);
  }

  const stats = getCodeStats(code);

  yield {
    type: 'progress',
    progress: 60,
  };

  // Build analysis report
  let analysis = `**Code Analysis**\n\n`;
  analysis += `- **Language**: ${language}\n`;
  analysis += `- **Lines**: ${stats.lines}\n`;
  analysis += `- **Characters**: ${stats.chars}\n`;
  if (stats.functions > 0) {
    analysis += `- **Functions**: ${stats.functions}\n`;
  }
  analysis += '\n';

  // Add structural hints based on language
  const hints: string[] = [];

  if (language === 'javascript' || language === 'typescript') {
    if (code.includes('async')) hints.push('Uses async/await for asynchronous operations');
    if (code.includes('=>')) hints.push('Uses arrow functions');
    if (code.includes('class ')) hints.push('Defines classes (OOP pattern)');
    if (code.includes('export')) hints.push('Exports modules (ES6 modules)');
  }

  if (language === 'python') {
    if (code.includes('async def')) hints.push('Uses async/await for asynchronous operations');
    if (code.includes('@')) hints.push('Uses decorators');
    if (code.includes('class ')) hints.push('Defines classes (OOP pattern)');
    if (code.includes('with ')) hints.push('Uses context managers');
  }

  if (language === 'rust') {
    if (code.includes('async fn')) hints.push('Uses async functions');
    if (code.includes('impl ')) hints.push('Implements traits or methods');
    if (code.includes('unsafe ')) hints.push('Contains unsafe code blocks');
    if (code.includes('Result<')) hints.push('Uses Result for error handling');
  }

  if (hints.length > 0) {
    analysis += `**Observations**:\n`;
    for (const hint of hints) {
      analysis += `- ${hint}\n`;
    }
    analysis += '\n';
  }

  analysis += `*For a detailed explanation of what this code does, the LLM will analyze the logic and provide a comprehensive breakdown.*`;

  yield {
    type: 'progress',
    progress: 100,
  };

  yield {
    type: 'message',
    content: analysis,
  };

  return {
    success: true,
    output: {
      language,
      stats,
      hints,
      code: code.substring(0, 500),
    },
    message: `Analyzed ${language} code (${stats.lines} lines)`,
    memoriesToStore: [
      {
        content: `Code analysis: ${language} code with ${stats.lines} lines, ${stats.functions} functions`,
        type: 'procedural',
      },
    ],
  };
}

async function* codeGenerateExecutor(
  context: SkillExecutionContext,
  params: Record<string, unknown>
): AsyncGenerator<SkillStep, SkillExecutionResult, void> {
  const description = params.description as string;
  const language = (params.language as string) || 'typescript';
  const style = (params.style as string) || 'documented';

  yield {
    type: 'message',
    content: `Generating ${language} code based on your description...`,
  };

  yield {
    type: 'progress',
    progress: 50,
  };

  // The actual code generation will be done by the LLM
  // This skill prepares the context and provides guidance

  const prompt = `Generate ${style} ${language} code that: ${description}`;

  yield {
    type: 'progress',
    progress: 100,
  };

  yield {
    type: 'message',
    content: `I'll generate ${language} code for: "${description}"\n\nThe response will include the code with ${style === 'documented' ? 'comments and documentation' : style === 'production' ? 'production-ready structure' : 'concise implementation'}.`,
  };

  return {
    success: true,
    output: {
      description,
      language,
      style,
      prompt,
    },
    message: `Prepared code generation request for ${language}`,
  };
}

export const CodeSkills = [CodeExplainSkill, CodeGenerateSkill];
export default CodeSkills;
