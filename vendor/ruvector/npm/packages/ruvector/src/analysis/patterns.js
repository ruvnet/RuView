"use strict";
/**
 * Pattern Extraction Module - Consolidated code pattern detection
 *
 * Single source of truth for extracting functions, imports, exports, etc.
 * Used by native-worker.ts and parallel-workers.ts
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.detectLanguage = detectLanguage;
exports.extractFunctions = extractFunctions;
exports.extractClasses = extractClasses;
exports.extractImports = extractImports;
exports.extractExports = extractExports;
exports.extractTodos = extractTodos;
exports.extractAllPatterns = extractAllPatterns;
exports.extractFromFiles = extractFromFiles;
exports.toPatternMatches = toPatternMatches;
const fs = __importStar(require("fs"));
/**
 * Detect language from file extension
 */
function detectLanguage(file) {
    const ext = file.split('.').pop()?.toLowerCase() || '';
    const langMap = {
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
function extractFunctions(content) {
    const patterns = [
        /function\s+(\w+)/g,
        /const\s+(\w+)\s*=\s*(?:async\s*)?\([^)]*\)\s*=>/g,
        /let\s+(\w+)\s*=\s*(?:async\s*)?\([^)]*\)\s*=>/g,
        /(?:async\s+)?(?:public|private|protected)?\s+(\w+)\s*\([^)]*\)\s*[:{]/g,
        /(\w+)\s*:\s*(?:async\s*)?\([^)]*\)\s*=>/g,
        /def\s+(\w+)\s*\(/g, // Python
        /fn\s+(\w+)\s*[<(]/g, // Rust
        /func\s+(\w+)\s*\(/g, // Go
    ];
    const funcs = new Set();
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
function extractClasses(content) {
    const patterns = [
        /class\s+(\w+)/g,
        /interface\s+(\w+)/g,
        /type\s+(\w+)\s*=/g,
        /enum\s+(\w+)/g,
        /struct\s+(\w+)/g,
    ];
    const classes = new Set();
    for (const pattern of patterns) {
        const regex = new RegExp(pattern.source, pattern.flags);
        let match;
        while ((match = regex.exec(content)) !== null) {
            if (match[1])
                classes.add(match[1]);
        }
    }
    return Array.from(classes);
}
/**
 * Extract import statements from content
 */
function extractImports(content) {
    const patterns = [
        /import\s+.*?from\s+['"]([^'"]+)['"]/g,
        /import\s+['"]([^'"]+)['"]/g,
        /require\s*\(['"]([^'"]+)['"]\)/g,
        /from\s+(\w+)\s+import/g, // Python
        /use\s+(\w+(?:::\w+)*)/g, // Rust
    ];
    const imports = [];
    for (const pattern of patterns) {
        const regex = new RegExp(pattern.source, pattern.flags);
        let match;
        while ((match = regex.exec(content)) !== null) {
            if (match[1])
                imports.push(match[1]);
        }
    }
    return [...new Set(imports)];
}
/**
 * Extract export statements from content
 */
function extractExports(content) {
    const patterns = [
        /export\s+(?:default\s+)?(?:class|function|const|let|var|interface|type|enum)\s+(\w+)/g,
        /export\s*\{\s*([^}]+)\s*\}/g,
        /module\.exports\s*=\s*(\w+)/g,
        /exports\.(\w+)\s*=/g,
        /pub\s+(?:fn|struct|enum|type)\s+(\w+)/g, // Rust
    ];
    const exports = [];
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
function extractTodos(content) {
    const pattern = /\/\/\s*(TODO|FIXME|HACK|XXX|BUG|NOTE):\s*(.+)/gi;
    const todos = [];
    let match;
    while ((match = pattern.exec(content)) !== null) {
        todos.push(`${match[1]}: ${match[2].trim()}`);
    }
    return todos;
}
/**
 * Extract all patterns from a file
 */
function extractAllPatterns(filePath, content) {
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
    }
    catch {
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
function extractFromFiles(files, maxFiles = 100) {
    return files.slice(0, maxFiles).map(f => extractAllPatterns(f));
}
/**
 * Convert FilePatterns to PatternMatch array (for native-worker compatibility)
 */
function toPatternMatches(patterns) {
    const matches = [];
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
exports.default = {
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
//# sourceMappingURL=patterns.js.map