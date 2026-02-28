"use strict";
/**
 * AST Parser - Tree-sitter based code parsing
 *
 * Provides real AST parsing for accurate code analysis,
 * replacing regex-based heuristics with proper parsing.
 *
 * Supports: TypeScript, JavaScript, Python, Rust, Go, Java, C/C++
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
exports.CodeParser = void 0;
exports.isTreeSitterAvailable = isTreeSitterAvailable;
exports.getCodeParser = getCodeParser;
exports.initCodeParser = initCodeParser;
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
// Try to load tree-sitter
let Parser = null;
let languages = new Map();
let parserError = null;
async function loadTreeSitter() {
    if (Parser)
        return true;
    if (parserError)
        return false;
    try {
        // Dynamic require to avoid TypeScript errors
        Parser = require('tree-sitter');
        return true;
    }
    catch (e) {
        parserError = new Error(`tree-sitter not installed: ${e.message}\n` +
            `Install with: npm install tree-sitter tree-sitter-typescript tree-sitter-javascript tree-sitter-python`);
        return false;
    }
}
async function loadLanguage(lang) {
    if (languages.has(lang))
        return languages.get(lang);
    const langPackages = {
        typescript: 'tree-sitter-typescript',
        javascript: 'tree-sitter-javascript',
        python: 'tree-sitter-python',
        rust: 'tree-sitter-rust',
        go: 'tree-sitter-go',
        java: 'tree-sitter-java',
        c: 'tree-sitter-c',
        cpp: 'tree-sitter-cpp',
        ruby: 'tree-sitter-ruby',
        php: 'tree-sitter-php',
    };
    const pkg = langPackages[lang];
    if (!pkg)
        return null;
    try {
        const langModule = await Promise.resolve(`${pkg}`).then(s => __importStar(require(s)));
        const language = langModule.default || langModule;
        // Handle TypeScript which exports tsx and typescript
        if (lang === 'typescript' && language.typescript) {
            languages.set(lang, language.typescript);
            languages.set('tsx', language.tsx);
            return language.typescript;
        }
        languages.set(lang, language);
        return language;
    }
    catch {
        return null;
    }
}
function isTreeSitterAvailable() {
    try {
        require.resolve('tree-sitter');
        return true;
    }
    catch {
        return false;
    }
}
// ============================================================================
// Parser
// ============================================================================
class CodeParser {
    constructor() {
        this.parser = null;
        this.initialized = false;
    }
    async init() {
        if (this.initialized)
            return true;
        const loaded = await loadTreeSitter();
        if (!loaded)
            return false;
        this.parser = new Parser();
        this.initialized = true;
        return true;
    }
    /**
     * Detect language from file extension
     */
    detectLanguage(file) {
        const ext = path.extname(file).toLowerCase();
        const langMap = {
            '.ts': 'typescript',
            '.tsx': 'tsx',
            '.js': 'javascript',
            '.jsx': 'javascript',
            '.mjs': 'javascript',
            '.cjs': 'javascript',
            '.py': 'python',
            '.rs': 'rust',
            '.go': 'go',
            '.java': 'java',
            '.c': 'c',
            '.h': 'c',
            '.cpp': 'cpp',
            '.cc': 'cpp',
            '.cxx': 'cpp',
            '.hpp': 'cpp',
            '.rb': 'ruby',
            '.php': 'php',
        };
        return langMap[ext] || 'unknown';
    }
    /**
     * Parse a file and return the AST
     */
    async parse(file, content) {
        if (!this.initialized) {
            await this.init();
        }
        if (!this.parser)
            return null;
        const lang = this.detectLanguage(file);
        const language = await loadLanguage(lang);
        if (!language)
            return null;
        this.parser.setLanguage(language);
        const code = content ?? (fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '');
        const tree = this.parser.parse(code);
        return this.convertNode(tree.rootNode);
    }
    convertNode(node) {
        return {
            type: node.type,
            text: node.text,
            startPosition: node.startPosition,
            endPosition: node.endPosition,
            children: node.children?.map((c) => this.convertNode(c)) || [],
        };
    }
    /**
     * Analyze a file for functions, classes, imports, etc.
     */
    async analyze(file, content) {
        const start = performance.now();
        const lang = this.detectLanguage(file);
        const code = content ?? (fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '');
        // Try tree-sitter first, fall back to regex
        if (this.initialized && this.parser) {
            const language = await loadLanguage(lang);
            if (language) {
                this.parser.setLanguage(language);
                const tree = this.parser.parse(code);
                return this.analyzeTree(file, lang, tree.rootNode, code, start);
            }
        }
        // Regex fallback
        return this.analyzeWithRegex(file, lang, code, start);
    }
    analyzeTree(file, lang, root, code, start) {
        const imports = [];
        const exports = [];
        const functions = [];
        const classes = [];
        const variables = [];
        const types = [];
        const visit = (node) => {
            // Imports
            if (node.type === 'import_statement' || node.type === 'import_declaration') {
                const imp = this.parseImport(node, lang);
                if (imp)
                    imports.push(imp);
            }
            // Exports
            if (node.type.includes('export')) {
                const exp = this.parseExport(node, lang);
                if (exp)
                    exports.push(exp);
            }
            // Functions
            if (node.type.includes('function') || node.type === 'method_definition' || node.type === 'arrow_function') {
                const fn = this.parseFunction(node, code, lang);
                if (fn)
                    functions.push(fn);
            }
            // Classes
            if (node.type === 'class_declaration' || node.type === 'class') {
                const cls = this.parseClass(node, code, lang);
                if (cls)
                    classes.push(cls);
            }
            // Variables
            if (node.type === 'variable_declarator' || node.type === 'assignment') {
                const name = this.getIdentifierName(node);
                if (name)
                    variables.push(name);
            }
            // Type definitions
            if (node.type === 'type_alias_declaration' || node.type === 'interface_declaration') {
                const name = this.getIdentifierName(node);
                if (name)
                    types.push(name);
            }
            // Recurse
            for (const child of node.children || []) {
                visit(child);
            }
        };
        visit(root);
        const lines = code.split('\n').length;
        const complexity = this.calculateComplexity(code);
        return {
            file,
            language: lang,
            imports,
            exports,
            functions,
            classes,
            variables,
            types,
            complexity,
            lines,
            parseTime: performance.now() - start,
        };
    }
    parseImport(node, lang) {
        try {
            const source = this.findChild(node, 'string')?.text?.replace(/['"]/g, '') || '';
            const named = [];
            let defaultImport;
            let namespace;
            // Find import specifiers
            const specifiers = this.findChild(node, 'import_clause') || node;
            for (const child of specifiers.children || []) {
                if (child.type === 'identifier') {
                    defaultImport = child.text;
                }
                else if (child.type === 'namespace_import') {
                    namespace = this.getIdentifierName(child) || undefined;
                }
                else if (child.type === 'named_imports') {
                    for (const spec of child.children || []) {
                        if (spec.type === 'import_specifier') {
                            named.push(this.getIdentifierName(spec) || '');
                        }
                    }
                }
            }
            return {
                source,
                default: defaultImport,
                named: named.filter(Boolean),
                namespace,
                type: 'esm',
            };
        }
        catch {
            return null;
        }
    }
    parseExport(node, lang) {
        try {
            if (node.type === 'export_statement') {
                const declaration = this.findChild(node, 'declaration');
                if (declaration) {
                    const name = this.getIdentifierName(declaration);
                    return { name: name || 'default', type: node.text.includes('default') ? 'default' : 'named' };
                }
            }
            return null;
        }
        catch {
            return null;
        }
    }
    parseFunction(node, code, lang) {
        try {
            const name = this.getIdentifierName(node) || '<anonymous>';
            const params = [];
            let returnType;
            const isAsync = node.text.includes('async');
            const isExported = node.parent?.type?.includes('export');
            // Get parameters
            const paramsNode = this.findChild(node, 'formal_parameters') || this.findChild(node, 'parameters');
            if (paramsNode) {
                for (const param of paramsNode.children || []) {
                    if (param.type === 'identifier' || param.type === 'required_parameter') {
                        params.push(this.getIdentifierName(param) || '');
                    }
                }
            }
            // Get return type
            const returnNode = this.findChild(node, 'type_annotation');
            if (returnNode) {
                returnType = returnNode.text.replace(/^:\s*/, '');
            }
            // Calculate complexity
            const bodyText = this.findChild(node, 'statement_block')?.text || '';
            const complexity = this.calculateComplexity(bodyText);
            // Find function calls
            const calls = [];
            const callRegex = /(\w+)\s*\(/g;
            let match;
            while ((match = callRegex.exec(bodyText)) !== null) {
                if (!['if', 'for', 'while', 'switch', 'catch', 'function'].includes(match[1])) {
                    calls.push(match[1]);
                }
            }
            return {
                name,
                params: params.filter(Boolean),
                returnType,
                async: isAsync,
                exported: isExported,
                startLine: node.startPosition.row + 1,
                endLine: node.endPosition.row + 1,
                complexity,
                calls: [...new Set(calls)],
            };
        }
        catch {
            return null;
        }
    }
    parseClass(node, code, lang) {
        try {
            const name = this.getIdentifierName(node) || '<anonymous>';
            let extendsClass;
            const implementsList = [];
            const methods = [];
            const properties = [];
            // Get extends/implements
            const heritage = this.findChild(node, 'class_heritage');
            if (heritage) {
                const extendsNode = this.findChild(heritage, 'extends_clause');
                if (extendsNode) {
                    extendsClass = this.getIdentifierName(extendsNode) || undefined;
                }
            }
            // Get methods and properties
            const body = this.findChild(node, 'class_body');
            if (body) {
                for (const member of body.children || []) {
                    if (member.type === 'method_definition') {
                        const method = this.parseFunction(member, code, lang);
                        if (method)
                            methods.push(method);
                    }
                    else if (member.type === 'field_definition' || member.type === 'public_field_definition') {
                        const propName = this.getIdentifierName(member);
                        if (propName)
                            properties.push(propName);
                    }
                }
            }
            return {
                name,
                extends: extendsClass,
                implements: implementsList,
                methods,
                properties,
                exported: node.parent?.type?.includes('export'),
                startLine: node.startPosition.row + 1,
                endLine: node.endPosition.row + 1,
            };
        }
        catch {
            return null;
        }
    }
    findChild(node, type) {
        if (!node.children)
            return null;
        for (const child of node.children) {
            if (child.type === type)
                return child;
            const found = this.findChild(child, type);
            if (found)
                return found;
        }
        return null;
    }
    getIdentifierName(node) {
        if (node.type === 'identifier')
            return node.text;
        if (!node.children)
            return null;
        for (const child of node.children) {
            if (child.type === 'identifier' || child.type === 'property_identifier') {
                return child.text;
            }
        }
        return null;
    }
    calculateComplexity(code) {
        const patterns = [
            /\bif\b/g,
            /\belse\b/g,
            /\bfor\b/g,
            /\bwhile\b/g,
            /\bcase\b/g,
            /\bcatch\b/g,
            /\?\s*[^:]/g, // ternary
            /&&/g,
            /\|\|/g,
        ];
        let complexity = 1;
        for (const pattern of patterns) {
            complexity += (code.match(pattern) || []).length;
        }
        return complexity;
    }
    analyzeWithRegex(file, lang, code, start) {
        const lines = code.split('\n');
        const imports = [];
        const exports = [];
        const functions = [];
        const classes = [];
        const variables = [];
        const types = [];
        // Regex patterns
        const importRegex = /import\s+(?:(\w+)\s*,?\s*)?(?:\{([^}]+)\}\s*)?(?:\*\s+as\s+(\w+)\s*)?from\s+['"]([^'"]+)['"]/g;
        const requireRegex = /(?:const|let|var)\s+(?:(\w+)|\{([^}]+)\})\s*=\s*require\s*\(['"]([^'"]+)['"]\)/g;
        const exportRegex = /export\s+(?:(default)\s+)?(?:(class|function|const|let|var|interface|type)\s+)?(\w+)?/g;
        const functionRegex = /(?:export\s+)?(?:async\s+)?function\s+(\w+)\s*\(([^)]*)\)/g;
        const arrowRegex = /(?:const|let|var)\s+(\w+)\s*=\s*(?:async\s*)?\([^)]*\)\s*=>/g;
        const classRegex = /(?:export\s+)?class\s+(\w+)(?:\s+extends\s+(\w+))?/g;
        const typeRegex = /(?:export\s+)?(?:type|interface)\s+(\w+)/g;
        // Parse imports
        let match;
        while ((match = importRegex.exec(code)) !== null) {
            imports.push({
                source: match[4],
                default: match[1],
                named: match[2] ? match[2].split(',').map(s => s.trim().split(/\s+as\s+/)[0]) : [],
                namespace: match[3],
                type: 'esm',
            });
        }
        while ((match = requireRegex.exec(code)) !== null) {
            imports.push({
                source: match[3],
                default: match[1],
                named: match[2] ? match[2].split(',').map(s => s.trim()) : [],
                type: 'commonjs',
            });
        }
        // Parse exports
        while ((match = exportRegex.exec(code)) !== null) {
            if (match[3]) {
                exports.push({
                    name: match[3],
                    type: match[1] === 'default' ? 'default' : 'named',
                });
            }
        }
        // Parse functions
        while ((match = functionRegex.exec(code)) !== null) {
            functions.push({
                name: match[1],
                params: match[2].split(',').map(p => p.trim().split(/[:\s]/)[0]).filter(Boolean),
                async: code.substring(match.index - 10, match.index).includes('async'),
                exported: code.substring(match.index - 10, match.index).includes('export'),
                startLine: code.substring(0, match.index).split('\n').length,
                endLine: 0,
                complexity: 1,
                calls: [],
            });
        }
        while ((match = arrowRegex.exec(code)) !== null) {
            functions.push({
                name: match[1],
                params: [],
                async: code.substring(match.index, match.index + 50).includes('async'),
                exported: false,
                startLine: code.substring(0, match.index).split('\n').length,
                endLine: 0,
                complexity: 1,
                calls: [],
            });
        }
        // Parse classes
        while ((match = classRegex.exec(code)) !== null) {
            classes.push({
                name: match[1],
                extends: match[2],
                implements: [],
                methods: [],
                properties: [],
                exported: code.substring(match.index - 10, match.index).includes('export'),
                startLine: code.substring(0, match.index).split('\n').length,
                endLine: 0,
            });
        }
        // Parse types
        while ((match = typeRegex.exec(code)) !== null) {
            types.push(match[1]);
        }
        return {
            file,
            language: lang,
            imports,
            exports,
            functions,
            classes,
            variables,
            types,
            complexity: this.calculateComplexity(code),
            lines: lines.length,
            parseTime: performance.now() - start,
        };
    }
    /**
     * Get all symbols (functions, classes, types) in a file
     */
    async getSymbols(file) {
        const analysis = await this.analyze(file);
        return [
            ...analysis.functions.map(f => f.name),
            ...analysis.classes.map(c => c.name),
            ...analysis.types,
            ...analysis.variables,
        ];
    }
    /**
     * Get the call graph for a file
     */
    async getCallGraph(file) {
        const analysis = await this.analyze(file);
        const graph = new Map();
        for (const fn of analysis.functions) {
            graph.set(fn.name, fn.calls);
        }
        for (const cls of analysis.classes) {
            for (const method of cls.methods) {
                graph.set(`${cls.name}.${method.name}`, method.calls);
            }
        }
        return graph;
    }
}
exports.CodeParser = CodeParser;
// ============================================================================
// Singleton
// ============================================================================
let parserInstance = null;
function getCodeParser() {
    if (!parserInstance) {
        parserInstance = new CodeParser();
    }
    return parserInstance;
}
async function initCodeParser() {
    const parser = getCodeParser();
    await parser.init();
    return parser;
}
exports.default = CodeParser;
//# sourceMappingURL=ast-parser.js.map