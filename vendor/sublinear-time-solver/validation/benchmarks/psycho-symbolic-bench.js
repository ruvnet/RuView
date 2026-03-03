import Benchmark from 'benchmark';
import { performance } from 'perf_hooks';
import chalk from 'chalk';
import Table from 'cli-table3';
import stats from 'stats-lite';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

class PsychoSymbolicReasoner {
    constructor() {
        this.knowledgeGraph = new Map();
        this.rules = [];
        this.goals = new Map();
        this.cache = new Map();
        this.initializeKnowledgeBase();
    }

    initializeKnowledgeBase() {
        for (let i = 0; i < 1000; i++) {
            this.knowledgeGraph.set(`entity_${i}`, {
                properties: Array(10).fill(0).map((_, j) => `prop_${j}`),
                relations: Array(5).fill(0).map((_, j) => `entity_${(i + j + 1) % 1000}`)
            });
        }

        for (let i = 0; i < 100; i++) {
            this.rules.push({
                condition: (entity) => entity.properties.length > 5,
                action: (entity) => ({ ...entity, inferred: true })
            });
        }
    }

    simpleQuery(entityId) {
        const start = performance.now();
        const result = this.knowledgeGraph.get(entityId);
        const end = performance.now();
        return { result, time: end - start };
    }

    complexReasoning(entityId, depth = 3) {
        const start = performance.now();
        const visited = new Set();
        const results = [];

        const traverse = (id, currentDepth) => {
            if (currentDepth <= 0 || visited.has(id)) return;
            visited.add(id);

            const entity = this.knowledgeGraph.get(id);
            if (entity) {
                for (const rule of this.rules) {
                    if (rule.condition(entity)) {
                        results.push(rule.action(entity));
                    }
                }

                for (const relation of entity.relations || []) {
                    traverse(relation, currentDepth - 1);
                }
            }
        };

        traverse(entityId, depth);
        const end = performance.now();
        return { results, time: end - start };
    }

    graphTraversal(startId, targetId) {
        const start = performance.now();
        const visited = new Set();
        const queue = [[startId, []]];

        while (queue.length > 0) {
            const [currentId, path] = queue.shift();

            if (currentId === targetId) {
                const end = performance.now();
                return { path: [...path, currentId], time: end - start };
            }

            if (!visited.has(currentId)) {
                visited.add(currentId);
                const entity = this.knowledgeGraph.get(currentId);

                if (entity && entity.relations) {
                    for (const relation of entity.relations) {
                        queue.push([relation, [...path, currentId]]);
                    }
                }
            }
        }

        const end = performance.now();
        return { path: null, time: end - start };
    }

    goapPlanning(initialState, goalState, maxSteps = 10) {
        const start = performance.now();
        const actions = [
            { name: 'move', cost: 1, effect: (state) => ({ ...state, position: state.position + 1 }) },
            { name: 'pickup', cost: 2, effect: (state) => ({ ...state, hasItem: true }) },
            { name: 'drop', cost: 1, effect: (state) => ({ ...state, hasItem: false }) },
            { name: 'unlock', cost: 3, effect: (state) => ({ ...state, doorOpen: true }) }
        ];

        const plan = [];
        let currentState = { ...initialState };
        let steps = 0;

        while (steps < maxSteps && JSON.stringify(currentState) !== JSON.stringify(goalState)) {
            const validActions = actions.filter(action => {
                const nextState = action.effect(currentState);
                return Object.keys(goalState).some(key =>
                    nextState[key] !== currentState[key] && nextState[key] === goalState[key]
                );
            });

            if (validActions.length === 0) break;

            const selectedAction = validActions.reduce((min, action) =>
                action.cost < min.cost ? action : min
            );

            plan.push(selectedAction.name);
            currentState = selectedAction.effect(currentState);
            steps++;
        }

        const end = performance.now();
        return { plan, time: end - start };
    }
}

async function runBenchmarks() {
    console.log(chalk.cyan('\n=== Psycho-Symbolic Reasoner Performance Benchmarks ===\n'));

    const reasoner = new PsychoSymbolicReasoner();
    const results = {
        timestamp: new Date().toISOString(),
        system: 'Psycho-Symbolic Reasoner',
        environment: {
            node: process.version,
            platform: process.platform,
            arch: process.arch,
            cpu: process.cpuUsage()
        },
        benchmarks: {}
    };

    const warmupIterations = 1000;
    console.log(chalk.yellow(`Warming up with ${warmupIterations} iterations...\n`));

    for (let i = 0; i < warmupIterations; i++) {
        reasoner.simpleQuery('entity_0');
        reasoner.complexReasoning('entity_0');
        reasoner.graphTraversal('entity_0', 'entity_500');
        reasoner.goapPlanning(
            { position: 0, hasItem: false, doorOpen: false },
            { position: 5, hasItem: true, doorOpen: true }
        );
    }

    const benchmarkSuite = new Benchmark.Suite();

    const tests = [
        {
            name: 'Simple Query',
            fn: () => reasoner.simpleQuery('entity_42')
        },
        {
            name: 'Complex Reasoning',
            fn: () => reasoner.complexReasoning('entity_42', 3)
        },
        {
            name: 'Graph Traversal',
            fn: () => reasoner.graphTraversal('entity_0', 'entity_500')
        },
        {
            name: 'GOAP Planning',
            fn: () => reasoner.goapPlanning(
                { position: 0, hasItem: false, doorOpen: false },
                { position: 5, hasItem: true, doorOpen: true }
            )
        }
    ];

    for (const test of tests) {
        const timings = [];
        const iterations = 10000;

        console.log(chalk.green(`Running: ${test.name}`));

        for (let i = 0; i < iterations; i++) {
            const start = performance.now();
            test.fn();
            const end = performance.now();
            timings.push(end - start);
        }

        const mean = stats.mean(timings);
        const median = stats.median(timings);
        const stdev = stats.stdev(timings);
        const percentile95 = stats.percentile(timings, 0.95);
        const percentile99 = stats.percentile(timings, 0.99);

        results.benchmarks[test.name] = {
            iterations,
            mean: mean.toFixed(3),
            median: median.toFixed(3),
            stdev: stdev.toFixed(3),
            min: Math.min(...timings).toFixed(3),
            max: Math.max(...timings).toFixed(3),
            p95: percentile95.toFixed(3),
            p99: percentile99.toFixed(3),
            unit: 'ms'
        };

        console.log(chalk.gray(`  Mean: ${mean.toFixed(3)}ms | Median: ${median.toFixed(3)}ms | StdDev: ${stdev.toFixed(3)}ms`));
    }

    const highResolutionTest = () => {
        const iterations = 100000;
        const timings = [];

        console.log(chalk.green(`\nHigh-resolution timing test (${iterations} iterations)`));

        for (let i = 0; i < iterations; i++) {
            const start = process.hrtime.bigint();
            reasoner.simpleQuery(`entity_${i % 1000}`);
            const end = process.hrtime.bigint();
            timings.push(Number(end - start) / 1000000);
        }

        return {
            mean: stats.mean(timings),
            median: stats.median(timings),
            min: Math.min(...timings),
            max: Math.max(...timings)
        };
    };

    results.highResolution = highResolutionTest();

    const table = new Table({
        head: ['Operation', 'Mean (ms)', 'Median (ms)', 'P95 (ms)', 'P99 (ms)', 'Min (ms)', 'Max (ms)'],
        colWidths: [20, 12, 12, 12, 12, 12, 12]
    });

    for (const [name, data] of Object.entries(results.benchmarks)) {
        table.push([
            name,
            data.mean,
            data.median,
            data.p95,
            data.p99,
            data.min,
            data.max
        ]);
    }

    console.log(chalk.cyan('\n=== Performance Summary ===\n'));
    console.log(table.toString());

    const resultsDir = path.join(__dirname, '..', 'results');
    if (!fs.existsSync(resultsDir)) {
        fs.mkdirSync(resultsDir, { recursive: true });
    }

    const filename = `psycho-symbolic-${Date.now()}.json`;
    fs.writeFileSync(
        path.join(resultsDir, filename),
        JSON.stringify(results, null, 2)
    );

    console.log(chalk.green(`\n✓ Results saved to: results/${filename}`));

    return results;
}

if (import.meta.url === `file://${process.argv[1]}`) {
    runBenchmarks().catch(console.error);
}

export { PsychoSymbolicReasoner, runBenchmarks };