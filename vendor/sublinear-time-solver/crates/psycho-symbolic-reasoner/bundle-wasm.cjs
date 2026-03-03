#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// Configuration
const CRATES = ['graph_reasoner', 'extractors', 'planner'];
const OUTPUT_DIR = path.join(__dirname, 'wasm-dist');
const BUNDLE_NAME = 'psycho-symbolic-reasoner';

console.log('📦 Bundling WASM modules...\n');

// Ensure output directory exists
function ensureOutputDir() {
    if (!fs.existsSync(OUTPUT_DIR)) {
        fs.mkdirSync(OUTPUT_DIR, { recursive: true });
        console.log(`📁 Created output directory: ${OUTPUT_DIR}`);
    }
}

// Copy WASM files and create unified exports
function bundleWasmFiles() {
    console.log('🔄 Copying WASM files...');

    const bundledFiles = {
        js: [],
        wasm: [],
        dts: []
    };

    for (const crate of CRATES) {
        const pkgPath = path.join(__dirname, crate, 'pkg');

        if (!fs.existsSync(pkgPath)) {
            console.error(`❌ Package not found: ${pkgPath}`);
            continue;
        }

        // Copy JavaScript file
        const jsFile = `${crate}.js`;
        const jsSource = path.join(pkgPath, jsFile);
        const jsTarget = path.join(OUTPUT_DIR, jsFile);

        if (fs.existsSync(jsSource)) {
            fs.copyFileSync(jsSource, jsTarget);
            bundledFiles.js.push(jsFile);
            console.log(`  ✅ Copied ${jsFile}`);
        }

        // Copy WASM file
        const wasmFile = `${crate}_bg.wasm`;
        const wasmSource = path.join(pkgPath, wasmFile);
        const wasmTarget = path.join(OUTPUT_DIR, wasmFile);

        if (fs.existsSync(wasmSource)) {
            fs.copyFileSync(wasmSource, wasmTarget);
            bundledFiles.wasm.push(wasmFile);
            console.log(`  ✅ Copied ${wasmFile}`);
        }

        // Copy TypeScript definitions
        const dtsFile = `${crate}.d.ts`;
        const dtsSource = path.join(pkgPath, dtsFile);
        const dtsTarget = path.join(OUTPUT_DIR, dtsFile);

        if (fs.existsSync(dtsSource)) {
            fs.copyFileSync(dtsSource, dtsTarget);
            bundledFiles.dts.push(dtsFile);
            console.log(`  ✅ Copied ${dtsFile}`);
        }
    }

    return bundledFiles;
}

// Create unified JavaScript entry point
function createUnifiedEntry(bundledFiles) {
    console.log('🔧 Creating unified entry point...');

    const entryContent = `
// Psycho-Symbolic Reasoner - Unified WASM Entry Point
// Auto-generated on ${new Date().toISOString()}

// Import all WASM modules
${CRATES.map(crate =>
    `import * as ${toCamelCase(crate)}Module from './${crate}.js';`
).join('\n')}

// Initialize function for browser environments
async function initWasm() {
    // This will be called automatically in most environments
    console.log('🚀 Initializing Psycho-Symbolic Reasoner WASM modules...');
}

// Unified API
export class PsychoSymbolicReasoner {
    constructor() {
        this.graphReasoner = new ${toCamelCase('graph_reasoner')}Module.GraphReasoner();
        this.textExtractor = new ${toCamelCase('extractors')}Module.TextExtractor();
        this.plannerSystem = new ${toCamelCase('planner')}Module.PlannerSystem();
    }

    // Graph reasoning methods
    addFact(subject, predicate, object) {
        return this.graphReasoner.add_fact(subject, predicate, object);
    }

    addRule(ruleJson) {
        return this.graphReasoner.add_rule(ruleJson);
    }

    query(queryJson) {
        return this.graphReasoner.query(queryJson);
    }

    infer(maxIterations = 10) {
        return this.graphReasoner.infer(maxIterations);
    }

    getGraphStats() {
        return this.graphReasoner.get_graph_stats();
    }

    // Text extraction methods
    analyzeSentiment(text) {
        return this.textExtractor.analyze_sentiment(text);
    }

    extractPreferences(text) {
        return this.textExtractor.extract_preferences(text);
    }

    detectEmotions(text) {
        return this.textExtractor.detect_emotions(text);
    }

    analyzeText(text) {
        return this.textExtractor.analyze_all(text);
    }

    // Planning methods
    setState(key, value) {
        return this.plannerSystem.set_state(key, value);
    }

    getState(key) {
        return this.plannerSystem.get_state(key);
    }

    addAction(actionJson) {
        return this.plannerSystem.add_action(actionJson);
    }

    addGoal(goalJson) {
        return this.plannerSystem.add_goal(goalJson);
    }

    plan(goalId) {
        return this.plannerSystem.plan(goalId);
    }

    planToState(targetStateJson) {
        return this.plannerSystem.plan_to_state(targetStateJson);
    }

    executePlan(planJson) {
        return this.plannerSystem.execute_plan(planJson);
    }

    evaluateRules() {
        return this.plannerSystem.evaluate_rules();
    }

    getWorldState() {
        return this.plannerSystem.get_world_state();
    }

    getAvailableActions() {
        return this.plannerSystem.get_available_actions();
    }

    // Utility methods
    version() {
        return '0.1.0';
    }

    capabilities() {
        return {
            graphReasoning: true,
            textExtraction: true,
            planning: true,
            inference: true,
            emotionDetection: true,
            sentimentAnalysis: true,
            preferenceExtraction: true
        };
    }
}

// Export individual modules for advanced usage
export {
    ${CRATES.map(crate => `${toCamelCase(crate)}Module`).join(',\n    ')}
};

// Export main class as default
export default PsychoSymbolicReasoner;

// Initialize WASM (called automatically)
initWasm();
`;

    fs.writeFileSync(path.join(OUTPUT_DIR, 'index.js'), entryContent);
    console.log('  ✅ Created index.js');
}

// Create unified TypeScript definitions
function createUnifiedTypes(bundledFiles) {
    console.log('📝 Creating unified TypeScript definitions...');

    const typesContent = `
// Psycho-Symbolic Reasoner - Unified TypeScript Definitions
// Auto-generated on ${new Date().toISOString()}

// Import individual module types
${CRATES.map(crate =>
    `export * from './${crate}';`
).join('\n')}

// Main unified class interface
export declare class PsychoSymbolicReasoner {
    constructor();

    // Graph reasoning methods
    addFact(subject: string, predicate: string, object: string): string;
    addRule(ruleJson: string): boolean;
    query(queryJson: string): string;
    infer(maxIterations?: number): string;
    getGraphStats(): string;

    // Text extraction methods
    analyzeSentiment(text: string): string;
    extractPreferences(text: string): string;
    detectEmotions(text: string): string;
    analyzeText(text: string): string;

    // Planning methods
    setState(key: string, value: string): boolean;
    getState(key: string): string;
    addAction(actionJson: string): boolean;
    addGoal(goalJson: string): boolean;
    plan(goalId: string): string;
    planToState(targetStateJson: string): string;
    executePlan(planJson: string): string;
    evaluateRules(): string;
    getWorldState(): string;
    getAvailableActions(): string;

    // Utility methods
    version(): string;
    capabilities(): {
        graphReasoning: boolean;
        textExtraction: boolean;
        planning: boolean;
        inference: boolean;
        emotionDetection: boolean;
        sentimentAnalysis: boolean;
        preferenceExtraction: boolean;
    };
}

export default PsychoSymbolicReasoner;
`;

    fs.writeFileSync(path.join(OUTPUT_DIR, 'index.d.ts'), typesContent);
    console.log('  ✅ Created index.d.ts');
}

// Create package.json for the bundle
function createBundlePackageJson() {
    console.log('📦 Creating bundle package.json...');

    const packageJson = {
        name: "@psycho-symbolic/reasoner",
        version: "0.1.0",
        description: "Complete WASM bindings for psycho-symbolic reasoning system",
        main: "index.js",
        types: "index.d.ts",
        exports: {
            ".": {
                "import": "./index.js",
                "require": "./index.js",
                "types": "./index.d.ts"
            },
            "./graph": "./graph_reasoner.js",
            "./extractors": "./extractors.js",
            "./planner": "./planner.js"
        },
        files: [
            "*.js",
            "*.wasm",
            "*.d.ts",
            "README.md"
        ],
        scripts: {
            "test": "node test-bundle.js"
        },
        keywords: [
            "wasm",
            "webassembly",
            "rust",
            "ai",
            "reasoning",
            "psycho-symbolic",
            "graph",
            "nlp",
            "planning",
            "inference",
            "sentiment",
            "emotion"
        ],
        author: "Psycho-Symbolic AI Team",
        license: "MIT",
        engines: {
            "node": ">=14.0.0"
        },
        repository: {
            type: "git",
            url: "https://github.com/your-org/sublinear-time-solver.git",
            directory: "psycho-symbolic-reasoner"
        },
        dependencies: {},
        devDependencies: {}
    };

    fs.writeFileSync(
        path.join(OUTPUT_DIR, 'package.json'),
        JSON.stringify(packageJson, null, 2)
    );
    console.log('  ✅ Created package.json');
}

// Create README for the bundle
function createBundleReadme() {
    console.log('📚 Creating bundle README...');

    const readmeContent = `
# Psycho-Symbolic Reasoner WASM

Complete WebAssembly bindings for the Psycho-Symbolic Reasoning System.

## Installation

\`\`\`bash
npm install @psycho-symbolic/reasoner
\`\`\`

## Usage

### Basic Usage

\`\`\`javascript
import PsychoSymbolicReasoner from '@psycho-symbolic/reasoner';

const reasoner = new PsychoSymbolicReasoner();

// Add facts to the knowledge graph
reasoner.addFact("Alice", "knows", "Bob");
reasoner.addFact("Bob", "likes", "coffee");

// Analyze text
const sentiment = reasoner.analyzeSentiment("I love this product!");
const emotions = reasoner.detectEmotions("I'm so excited!");

// Plan actions
reasoner.setState("has_key", '{"type": "boolean", "value": true}');
const plan = reasoner.plan("unlock_door");
\`\`\`

### Advanced Usage

\`\`\`javascript
import {
    graphReasonerModule,
    extractorsModule,
    plannerModule
} from '@psycho-symbolic/reasoner';

// Use individual modules
const graphReasoner = new graphReasonerModule.GraphReasoner();
const textExtractor = new extractorsModule.TextExtractor();
const plannerSystem = new plannerModule.PlannerSystem();
\`\`\`

## Features

- **Graph Reasoning**: Knowledge graph construction and inference
- **Text Analysis**: Sentiment analysis, emotion detection, preference extraction
- **Planning**: Goal-oriented action planning with GOAP algorithm
- **Inference**: Rule-based reasoning and knowledge discovery
- **TypeScript Support**: Full type definitions included

## API Reference

### PsychoSymbolicReasoner

#### Graph Reasoning
- \`addFact(subject, predicate, object)\` - Add a fact to the knowledge graph
- \`addRule(ruleJson)\` - Add an inference rule
- \`query(queryJson)\` - Query the knowledge graph
- \`infer(maxIterations)\` - Run inference engine
- \`getGraphStats()\` - Get graph statistics

#### Text Analysis
- \`analyzeSentiment(text)\` - Analyze sentiment of text
- \`extractPreferences(text)\` - Extract user preferences
- \`detectEmotions(text)\` - Detect emotions in text
- \`analyzeText(text)\` - Complete text analysis

#### Planning
- \`setState(key, value)\` - Set world state
- \`getState(key)\` - Get world state
- \`addAction(actionJson)\` - Add available action
- \`addGoal(goalJson)\` - Add planning goal
- \`plan(goalId)\` - Generate action plan
- \`executePlan(planJson)\` - Execute action plan

## License

MIT
`;

    fs.writeFileSync(path.join(OUTPUT_DIR, 'README.md'), readmeContent.trim());
    console.log('  ✅ Created README.md');
}

// Create bundle test
function createBundleTest() {
    console.log('🧪 Creating bundle test...');

    const testContent = `
const PsychoSymbolicReasoner = require('./index.js').default;

async function testBundle() {
    console.log('🧪 Testing unified WASM bundle...');

    try {
        const reasoner = new PsychoSymbolicReasoner();

        // Test capabilities
        const capabilities = reasoner.capabilities();
        console.log('✅ Capabilities:', capabilities);

        // Test version
        const version = reasoner.version();
        console.log('✅ Version:', version);

        // Test basic functionality
        const factId = reasoner.addFact("test", "type", "demo");
        console.log('✅ Added fact:', factId);

        const sentiment = reasoner.analyzeSentiment("This is great!");
        console.log('✅ Sentiment analysis:', JSON.parse(sentiment));

        const stateSet = reasoner.setState("test_state", '{"type": "boolean", "value": true}');
        console.log('✅ State set:', stateSet);

        console.log('🎉 Bundle test completed successfully!');
    } catch (error) {
        console.error('❌ Bundle test failed:', error);
        process.exit(1);
    }
}

if (require.main === module) {
    testBundle();
}
`;

    fs.writeFileSync(path.join(OUTPUT_DIR, 'test-bundle.js'), testContent);
    console.log('  ✅ Created test-bundle.js');
}

// Utility function to convert snake_case to camelCase
function toCamelCase(str) {
    return str.replace(/_([a-z])/g, (match, letter) => letter.toUpperCase());
}

// Create optimized production bundle with esbuild
function createProductionBundle() {
    console.log('⚡ Creating optimized production bundle...');

    try {
        // Check if esbuild is available
        execSync('npx esbuild --version', { stdio: 'pipe' });

        // Bundle for different targets
        const targets = [
            { name: 'esm', format: 'esm', ext: 'mjs' },
            { name: 'cjs', format: 'cjs', ext: 'cjs' },
            { name: 'iife', format: 'iife', ext: 'bundle.js' }
        ];

        for (const target of targets) {
            const outputFile = path.join(OUTPUT_DIR, `index.${target.ext}`);
            const buildCommand = `npx esbuild ${path.join(OUTPUT_DIR, 'index.js')} --bundle --format=${target.format} --outfile=${outputFile} --minify --sourcemap`;

            try {
                execSync(buildCommand, { stdio: 'pipe' });
                console.log(`  ✅ Created ${target.name} bundle: index.${target.ext}`);
            } catch (error) {
                console.warn(`  ⚠️ Could not create ${target.name} bundle:`, error.message);
            }
        }
    } catch (error) {
        console.warn('⚠️ esbuild not available, skipping production bundles');
    }
}

// Main bundling function
function main() {
    ensureOutputDir();

    const bundledFiles = bundleWasmFiles();
    createUnifiedEntry(bundledFiles);
    createUnifiedTypes(bundledFiles);
    createBundlePackageJson();
    createBundleReadme();
    createBundleTest();
    createProductionBundle();

    console.log('\n🎉 WASM bundling completed successfully!');
    console.log(`📁 Bundle location: ${OUTPUT_DIR}`);
    console.log('\n📋 Next steps:');
    console.log('  1. Test bundle: cd wasm-dist && node test-bundle.js');
    console.log('  2. Install as dependency: npm install ./wasm-dist');
    console.log('  3. Publish to npm: cd wasm-dist && npm publish');
}

// Run if called directly
if (require.main === module) {
    main();
}

module.exports = {
    bundleWasmFiles,
    createUnifiedEntry,
    createUnifiedTypes,
    OUTPUT_DIR
};