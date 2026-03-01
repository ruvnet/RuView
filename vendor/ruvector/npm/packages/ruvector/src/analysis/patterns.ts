/**
 * Pattern Extraction Module - Consolidated code pattern detection
 *
 * Single source of truth for extracting functions, imports, exports, etc.
 * Used by native-worker.ts and parallel-workers.ts
 */

import * as fs from 'fs';

export interface PatternMatch {
  type: 'function' | 'class' | 'import' | 'export' | 'todo' | 'variable' | 'type';
  match: string;
  file: string;
  line?: number;
}

export interface FilePatterns {
  file: string;
  language: string;
  functions: string[];
  classes: string[];
  imports: string[];
  exports: string[];
  todos: string[];
  variables: string[];
}

/**
 * Detect language from file extension
 */
export function detectLanguage(file: string): string {
  const ext = file.split('.').pop()?.toLowerCase() || '';
  const langMap: Record<string, string> = {
    ts: 'typescript', tsx: 'typescript', js: 'javascript', jsx: 'javascript',
    rs: 'rust', py: 'python', go: 'go', java: 'java', rb: 'ruby',
    cpp: 'cpp', c: 'c', h: 'c', hpp: 'cpp', cs: 'csharp',
    md: 'markdown', json: 'json', yaml: 'yaml', yml: 'yaml',
    sql: 'sql', sh: 'shell', bash: 'shell', zsh: 'shell',
  };
  return langMap[ext] || ext || 'unknown';
}

/**
 * Extract function names from content
 */
export function extractFunctions(content: string): string[] {
  const patterns = [
    /function\s+(\w+)/g,
    /const\s+(\w+)\s*=\s*(?:async\s*)?\([^)]*\)\s*=>/g,
    /let\s+(\w+)\s*=\s*(?:async\s*)?\([^)]*\)\s*=>/g,
    /(?:async\s+)?(?:public|private|protected)?\s+(\w+)\s*\([^)]*\)\s*[:{]/g,
    /(\w+)\s*:\s*(?:async\s*)?\([^)]*\)\s*=>/g,
    /def\s+(\w+)\s*\(/g,  // Python
    /fn\s+(\w+)\s*[<(]/g, // Rust
    /func\s+(\w+)\s*\(/g, // Go
  ];

  const funcs = new Set<string>();
  const reserved = new Set(['if', 'for', 'while', 'switch', 'catch', 'try', 'else', 'return', 'new', 'class', 'function', 'async', 'await']);

  for (const pattern of patterns) {
    const regex = new RegExp(pattern.source, pattern.flags);
    let match;
    while ((match = regex.exec(content)) !== null) {
      const name = match[1];
      if (name && !reserved.has(name) && name.length > 1) {
        funcs.add(name);
      }
    }
  }

  return Array.from(funcs);
}

/**
 * Extract class names from content
 */
export function extractClasses(content: string): string[] {
  const patterns = [
    /class\s+(\w+)/g,
    /interface\s+(\w+)/g,
    /type\s+(\w+)\s*=/g,
    /enum\s+(\w+)/g,
    /struct\s+(\w+)/g,
  ];

  const classes = new Set<string>();
  for (const pattern of patterns) {
    const regex = new RegExp(pattern.source, pattern.flags);
    let match;
    while ((match = regex.exec(content)) !== null) {
      if (match[1]) classes.add(match[1]);
    }
  }

  return Array.from(classes);
}

/**
 * Extract import statements from content
 */
export function extractImports(content: string): string[] {
  const patterns = [
    /import\s+.*?from\s+['"]([^'"]+)['"]/g,
    /import\s+['"]([^'"]+)['"]/g,
    /require\s*\(['"]([^'"]+)['"]\)/g,
    /from\s+(\w+)\s+import/g, // Python
    /use\s+(\w+(?:::\w+)*)/g, // Rust
  ];

  const imports: string[] = [];
  for (const pattern of patterns) {
    const regex = new RegExp(pattern.source, pattern.flags);
    let match;
    while ((match = regex.exec(content)) !== null) {
      if (match[1]) imports.push(match[1]);
    }
  }

  return [...new Set(imports)];
}

/**
 * Extract export statements from content
 */
export function extractExports(content: string): string[] {
  const patterns = [
    /export\s+(?:default\s+)?(?:class|function|const|let|var|interface|type|enum)\s+(\w+)/g,
    /export\s*\{\s*([^}]+)\s*\}/g,
    /module\.exports\s*=\s*(\w+)/g,
    /exports\.(\w+)\s*=/g,
    /pub\s+(?:fn|struct|enum|type)\s+(\w+)/g, // Rust
  ];

  const exports: string[] = [];
  for (const pattern of patterns) {
    const regex = new RegExp(pattern.source, pattern.flags);
    let match;
    while ((match = regex.exec(content)) !== null) {
      if (match[1]) {
        // Handle grouped exports: export { a, b, c }
        const names = match[1].split(',').map(s => s.trim().split(/\s+as\s+/)[0].trim());
        exports.push(...names.filter(n => n && /^\w+$/.test(n)));
      }
    }
  }

  return [...new Set(exports)];
}

/**
 * Extract TODO/FIXME comments from content
 */
export function extractTodos(content: string): string[] {
  const pattern = /\/\/\s*(TODO|FIXME|HACK|XXX|BUG|NOTE):\s*(.+)/gi;
  const todos: string[] = [];

  let match;
  while ((match = pattern.exec(content)) !== null) {
    todos.push(`${match[1]}: ${match[2].trim()}`);
  }

  return todos;
}

/**
 * Extract all patterns from a file
 */
export function extractAllPatterns(filePath: string, content?: string): FilePatterns {
  try {
    const fileContent = content ?? (fs.existsSync(filePath) ? fs.readFileSync(filePath, 'utf-8') : '');

    return {
      file: filePath,
      language: detectLanguage(filePath),
      functions: extractFunctions(fileContent),
      classes: extractClasses(fileContent),
      imports: extractImports(fileContent),
      exports: extractExports(fileContent),
      todos: extractTodos(fileContent),
      variables: [], // Could add variable extraction if needed
    };
  } catch {
    return {
      file: filePath,
      language: detectLanguage(filePath),
      functions: [],
      classes: [],
      imports: [],
      exports: [],
      todos: [],
      variables: [],
    };
  }
}

/**
 * Extract patterns from multiple files
 */
export function extractFromFiles(files: string[], maxFiles: number = 100): FilePatterns[] {
  return files.slice(0, maxFiles).map(f => extractAllPatterns(f));
}

/**
 * Convert FilePatterns to PatternMatch array (for native-worker compatibility)
 */
export function toPatternMatches(patterns: FilePatterns): PatternMatch[] {
  const matches: PatternMatch[] = [];

  for (const func of patterns.functions) {
    matches.push({ type: 'function', match: func, file: patterns.file });
  }
  for (const cls of patterns.classes) {
    matches.push({ type: 'class', match: cls, file: patterns.file });
  }
  for (const imp of patterns.imports) {
    matches.push({ type: 'import', match: imp, file: patterns.file });
  }
  for (const exp of patterns.exports) {
    matches.push({ type: 'export', match: exp, file: patterns.file });
  }
  for (const todo of patterns.todos) {
    matches.push({ type: 'todo', match: todo, file: patterns.file });
  }

  return matches;
}

export default {
  detectLanguage,
  extractFunctions,
  extractClasses,
  extractImports,
  extractExports,
  extractTodos,
  extractAllPatterns,
  extractFromFiles,
  toPatternMatches,
};
