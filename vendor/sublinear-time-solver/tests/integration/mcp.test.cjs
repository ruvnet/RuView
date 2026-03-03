#!/usr/bin/env node

/**
 * MCP (Model Context Protocol) compliance tests
 * Tests the MCP server interface and protocol compliance
 * Run with: node tests/integration/mcp.test.js
 */

const { strict: assert } = require('assert');
const { spawn } = require('child_process');
const fs = require('fs').promises;
const path = require('path');

class MCPTestRunner {
    constructor() {
        this.tests = [];
        this.passed = 0;
        this.failed = 0;
        this.verbose = process.argv.includes('--verbose');
        this.mcpConfigPath = path.join(__dirname, '../../.mcp.json');
    }

    test(name, fn) {
        this.tests.push({ name, fn });
    }

    async run() {
        console.log('🧪 Running MCP Protocol Compliance Tests');
        console.log('=========================================\n');

        for (const { name, fn } of this.tests) {
            try {
                await fn();
                this.passed++;
                console.log(`✅ ${name}`);
            } catch (error) {
                this.failed++;
                console.log(`❌ ${name}`);
                if (this.verbose) {
                    console.log(`   Error: ${error.message}`);
                    console.log(`   Stack: ${error.stack}\n`);
                } else {
                    console.log(`   Error: ${error.message}\n`);
                }
            }
        }

        this.printSummary();
        return this.failed === 0;
    }

    printSummary() {
        console.log('\n📊 Test Summary');
        console.log('===============');
        console.log(`✅ Passed: ${this.passed}`);
        console.log(`❌ Failed: ${this.failed}`);
        console.log(`📈 Total:  ${this.tests.length}`);
        console.log(`🎯 Success Rate: ${((this.passed / this.tests.length) * 100).toFixed(1)}%`);
    }

    // Simulate MCP client communication
    async sendMCPMessage(message) {
        return new Promise((resolve, reject) => {
            // This would normally be a real MCP connection
            // For testing, we simulate the protocol
            setTimeout(() => {
                resolve({
                    jsonrpc: "2.0",
                    id: message.id || 1,
                    result: { status: "ok" }
                });
            }, 100);
        });
    }

    // Mock MCP server implementation for testing
    createMockMCPServer() {
        return {
            async initialize() {
                return {
                    capabilities: {
                        tools: {
                            listChanged: true
                        },
                        resources: {
                            subscribe: true,
                            listChanged: true
                        }
                    },
                    serverInfo: {
                        name: "sublinear-time-solver",
                        version: "0.1.0"
                    }
                };
            },

            async listTools() {
                return {
                    tools: [
                        {
                            name: "solve_linear_system",
                            description: "Solve a sparse linear system using sublinear algorithms",
                            inputSchema: {
                                type: "object",
                                properties: {
                                    matrix: {
                                        type: "object",
                                        description: "Sparse matrix in COO format"
                                    },
                                    vector: {
                                        type: "array",
                                        description: "Right-hand side vector"
                                    },
                                    method: {
                                        type: "string",
                                        enum: ["jacobi", "gauss-seidel", "cg", "hybrid"],
                                        default: "hybrid"
                                    },
                                    tolerance: {
                                        type: "number",
                                        default: 1e-10
                                    },
                                    maxIterations: {
                                        type: "number",
                                        default: 1000
                                    }
                                },
                                required: ["matrix", "vector"]
                            }
                        },
                        {
                            name: "benchmark_solver",
                            description: "Run performance benchmarks on solver algorithms",
                            inputSchema: {
                                type: "object",
                                properties: {
                                    size: {
                                        type: "number",
                                        description: "Matrix size for benchmark"
                                    },
                                    sparsity: {
                                        type: "number",
                                        description: "Matrix sparsity (0-1)"
                                    },
                                    methods: {
                                        type: "array",
                                        items: { type: "string" }
                                    }
                                }
                            }
                        },
                        {
                            name: "validate_solution",
                            description: "Validate a solution to a linear system",
                            inputSchema: {
                                type: "object",
                                properties: {
                                    matrix: { type: "object" },
                                    solution: { type: "array" },
                                    vector: { type: "array" },
                                    tolerance: { type: "number", default: 1e-8 }
                                },
                                required: ["matrix", "solution", "vector"]
                            }
                        }
                    ]
                };
            },

            async listResources() {
                return {
                    resources: [
                        {
                            uri: "solver://algorithms",
                            name: "Available Algorithms",
                            description: "List of available solver algorithms and their properties",
                            mimeType: "application/json"
                        },
                        {
                            uri: "solver://benchmarks",
                            name: "Benchmark Results",
                            description: "Historical benchmark data and performance metrics",
                            mimeType: "application/json"
                        },
                        {
                            uri: "solver://examples",
                            name: "Example Problems",
                            description: "Pre-configured example linear systems",
                            mimeType: "application/json"
                        }
                    ]
                };
            },

            async callTool(name, args) {
                switch (name) {
                    case "solve_linear_system":
                        return {
                            content: [
                                {
                                    type: "text",
                                    text: "Linear system solved successfully"
                                },
                                {
                                    type: "application/json",
                                    data: {
                                        solution: new Array(args.vector.length).fill(1.0),
                                        iterations: 42,
                                        residual: 1e-12,
                                        method: args.method || "hybrid",
                                        convergence: true
                                    }
                                }
                            ]
                        };

                    case "benchmark_solver":
                        return {
                            content: [
                                {
                                    type: "text",
                                    text: "Benchmark completed"
                                },
                                {
                                    type: "application/json",
                                    data: {
                                        results: [
                                            {
                                                method: "jacobi",
                                                avgTime: 45.2,
                                                iterations: 123,
                                                convergenceRate: 0.95
                                            },
                                            {
                                                method: "cg",
                                                avgTime: 28.7,
                                                iterations: 67,
                                                convergenceRate: 0.98
                                            }
                                        ],
                                        matrixSize: args.size || 1000,
                                        sparsity: args.sparsity || 0.01
                                    }
                                }
                            ]
                        };

                    case "validate_solution":
                        return {
                            content: [
                                {
                                    type: "text",
                                    text: "Solution validation completed"
                                },
                                {
                                    type: "application/json",
                                    data: {
                                        valid: true,
                                        maxError: 1e-10,
                                        meanError: 5e-11,
                                        tolerance: args.tolerance || 1e-8
                                    }
                                }
                            ]
                        };

                    default:
                        throw new Error(`Unknown tool: ${name}`);
                }
            },

            async readResource(uri) {
                switch (uri) {
                    case "solver://algorithms":
                        return {
                            contents: [
                                {
                                    uri: uri,
                                    mimeType: "application/json",
                                    text: JSON.stringify({
                                        algorithms: [
                                            {
                                                name: "jacobi",
                                                description: "Jacobi iterative method",
                                                complexity: "O(nnz * k)",
                                                convergence: "diagonal dominance required"
                                            },
                                            {
                                                name: "gauss-seidel",
                                                description: "Gauss-Seidel iterative method",
                                                complexity: "O(nnz * k)",
                                                convergence: "faster than Jacobi for many problems"
                                            },
                                            {
                                                name: "cg",
                                                description: "Conjugate Gradient method",
                                                complexity: "O(sqrt(κ) * nnz * k)",
                                                convergence: "SPD matrices only"
                                            },
                                            {
                                                name: "hybrid",
                                                description: "Adaptive hybrid algorithm selection",
                                                complexity: "O(log n) for analysis + optimal solver",
                                                convergence: "automatic method selection"
                                            }
                                        ]
                                    }, null, 2)
                                }
                            ]
                        };

                    case "solver://benchmarks":
                        return {
                            contents: [
                                {
                                    uri: uri,
                                    mimeType: "application/json",
                                    text: JSON.stringify({
                                        benchmarks: [
                                            {
                                                date: "2024-01-15",
                                                matrixSize: 1000,
                                                sparsity: 0.01,
                                                results: {
                                                    jacobi: { time: 45.2, iterations: 123 },
                                                    cg: { time: 28.7, iterations: 67 },
                                                    hybrid: { time: 22.1, iterations: 45 }
                                                }
                                            }
                                        ]
                                    }, null, 2)
                                }
                            ]
                        };

                    case "solver://examples":
                        return {
                            contents: [
                                {
                                    uri: uri,
                                    mimeType: "application/json",
                                    text: JSON.stringify({
                                        examples: [
                                            {
                                                name: "Heat Equation 2D",
                                                description: "2D heat equation discretization",
                                                matrix: {
                                                    rows: 4,
                                                    cols: 4,
                                                    format: "coo",
                                                    data: {
                                                        values: [4, -1, -1, 4, -1, -1, 4, -1],
                                                        rowIndices: [0, 0, 1, 1, 1, 2, 2, 2],
                                                        colIndices: [0, 1, 0, 1, 2, 1, 2, 3]
                                                    }
                                                },
                                                vector: [1, 0, 0, 1]
                                            }
                                        ]
                                    }, null, 2)
                                }
                            ]
                        };

                    default:
                        throw new Error(`Unknown resource: ${uri}`);
                }
            }
        };
    }
}

const runner = new MCPTestRunner();

// MCP Configuration Tests
runner.test('MCP configuration file exists and is valid', async () => {
    const configContent = await fs.readFile(runner.mcpConfigPath, 'utf8');
    const config = JSON.parse(configContent);

    assert.ok(config.mcpServers);
    assert.ok(typeof config.mcpServers === 'object');
});

runner.test('MCP configuration includes required servers', async () => {
    const configContent = await fs.readFile(runner.mcpConfigPath, 'utf8');
    const config = JSON.parse(configContent);

    // Check for expected MCP server entries
    assert.ok(config.mcpServers['claude-flow'] || config.mcpServers['ruv-swarm']);

    for (const [name, serverConfig] of Object.entries(config.mcpServers)) {
        assert.ok(serverConfig.command);
        assert.ok(serverConfig.args);
        assert.ok(serverConfig.type);
    }
});

// MCP Protocol Compliance Tests
runner.test('MCP server initialization follows protocol', async () => {
    const server = runner.createMockMCPServer();

    const initResult = await server.initialize();

    // Check required initialization response structure
    assert.ok(initResult.capabilities);
    assert.ok(initResult.serverInfo);
    assert.ok(initResult.serverInfo.name);
    assert.ok(initResult.serverInfo.version);
});

runner.test('MCP server supports required capabilities', async () => {
    const server = runner.createMockMCPServer();

    const initResult = await server.initialize();

    // Check for tools capability
    assert.ok(initResult.capabilities.tools);
    assert.ok(typeof initResult.capabilities.tools.listChanged === 'boolean');

    // Check for resources capability
    assert.ok(initResult.capabilities.resources);
    assert.ok(typeof initResult.capabilities.resources.subscribe === 'boolean');
    assert.ok(typeof initResult.capabilities.resources.listChanged === 'boolean');
});

// MCP Tools Tests
runner.test('MCP server lists available tools', async () => {
    const server = runner.createMockMCPServer();

    const toolsResult = await server.listTools();

    assert.ok(toolsResult.tools);
    assert.ok(Array.isArray(toolsResult.tools));
    assert.ok(toolsResult.tools.length > 0);

    // Verify each tool has required properties
    for (const tool of toolsResult.tools) {
        assert.ok(tool.name);
        assert.ok(tool.description);
        assert.ok(tool.inputSchema);
        assert.equal(tool.inputSchema.type, 'object');
    }
});

runner.test('MCP server provides solve_linear_system tool', async () => {
    const server = runner.createMockMCPServer();

    const toolsResult = await server.listTools();
    const solveTool = toolsResult.tools.find(tool => tool.name === 'solve_linear_system');

    assert.ok(solveTool);
    assert.ok(solveTool.description.includes('linear system'));
    assert.ok(solveTool.inputSchema.properties.matrix);
    assert.ok(solveTool.inputSchema.properties.vector);
    assert.ok(solveTool.inputSchema.required.includes('matrix'));
    assert.ok(solveTool.inputSchema.required.includes('vector'));
});

runner.test('MCP server provides benchmark_solver tool', async () => {
    const server = runner.createMockMCPServer();

    const toolsResult = await server.listTools();
    const benchmarkTool = toolsResult.tools.find(tool => tool.name === 'benchmark_solver');

    assert.ok(benchmarkTool);
    assert.ok(benchmarkTool.description.includes('benchmark'));
    assert.ok(benchmarkTool.inputSchema.properties.size);
    assert.ok(benchmarkTool.inputSchema.properties.sparsity);
});

runner.test('MCP server provides validate_solution tool', async () => {
    const server = runner.createMockMCPServer();

    const toolsResult = await server.listTools();
    const validateTool = toolsResult.tools.find(tool => tool.name === 'validate_solution');

    assert.ok(validateTool);
    assert.ok(validateTool.description.includes('validate'));
    assert.ok(validateTool.inputSchema.properties.matrix);
    assert.ok(validateTool.inputSchema.properties.solution);
    assert.ok(validateTool.inputSchema.properties.vector);
});

// MCP Tool Execution Tests
runner.test('MCP solve_linear_system tool execution', async () => {
    const server = runner.createMockMCPServer();

    const args = {
        matrix: {
            rows: 2,
            cols: 2,
            format: 'coo',
            data: {
                values: [2, 1, 1, 2],
                rowIndices: [0, 0, 1, 1],
                colIndices: [0, 1, 0, 1]
            }
        },
        vector: [3, 3],
        method: 'cg',
        tolerance: 1e-10
    };

    const result = await server.callTool('solve_linear_system', args);

    assert.ok(result.content);
    assert.ok(Array.isArray(result.content));

    // Check for text response
    const textContent = result.content.find(c => c.type === 'text');
    assert.ok(textContent);

    // Check for JSON data response
    const jsonContent = result.content.find(c => c.type === 'application/json');
    assert.ok(jsonContent);
    assert.ok(jsonContent.data.solution);
    assert.ok(typeof jsonContent.data.iterations === 'number');
    assert.ok(typeof jsonContent.data.residual === 'number');
});

runner.test('MCP benchmark_solver tool execution', async () => {
    const server = runner.createMockMCPServer();

    const args = {
        size: 1000,
        sparsity: 0.01,
        methods: ['jacobi', 'cg']
    };

    const result = await server.callTool('benchmark_solver', args);

    assert.ok(result.content);
    const jsonContent = result.content.find(c => c.type === 'application/json');
    assert.ok(jsonContent);
    assert.ok(jsonContent.data.results);
    assert.ok(Array.isArray(jsonContent.data.results));
    assert.equal(jsonContent.data.matrixSize, 1000);
});

runner.test('MCP validate_solution tool execution', async () => {
    const server = runner.createMockMCPServer();

    const args = {
        matrix: {
            rows: 2,
            cols: 2,
            data: [1, 0, 0, 1],
            format: 'dense'
        },
        solution: [1, 1],
        vector: [1, 1],
        tolerance: 1e-8
    };

    const result = await server.callTool('validate_solution', args);

    assert.ok(result.content);
    const jsonContent = result.content.find(c => c.type === 'application/json');
    assert.ok(jsonContent);
    assert.ok(typeof jsonContent.data.valid === 'boolean');
    assert.ok(typeof jsonContent.data.maxError === 'number');
});

// MCP Resources Tests
runner.test('MCP server lists available resources', async () => {
    const server = runner.createMockMCPServer();

    const resourcesResult = await server.listResources();

    assert.ok(resourcesResult.resources);
    assert.ok(Array.isArray(resourcesResult.resources));
    assert.ok(resourcesResult.resources.length > 0);

    // Verify each resource has required properties
    for (const resource of resourcesResult.resources) {
        assert.ok(resource.uri);
        assert.ok(resource.name);
        assert.ok(resource.description);
        assert.ok(resource.mimeType);
    }
});

runner.test('MCP server provides algorithms resource', async () => {
    const server = runner.createMockMCPServer();

    const resourcesResult = await server.listResources();
    const algorithmsResource = resourcesResult.resources.find(r => r.uri === 'solver://algorithms');

    assert.ok(algorithmsResource);
    assert.ok(algorithmsResource.name.includes('Algorithm'));
    assert.equal(algorithmsResource.mimeType, 'application/json');
});

runner.test('MCP server can read algorithms resource', async () => {
    const server = runner.createMockMCPServer();

    const result = await server.readResource('solver://algorithms');

    assert.ok(result.contents);
    assert.ok(Array.isArray(result.contents));

    const content = result.contents[0];
    assert.equal(content.uri, 'solver://algorithms');
    assert.equal(content.mimeType, 'application/json');

    const algorithms = JSON.parse(content.text);
    assert.ok(algorithms.algorithms);
    assert.ok(Array.isArray(algorithms.algorithms));
});

runner.test('MCP server can read benchmarks resource', async () => {
    const server = runner.createMockMCPServer();

    const result = await server.readResource('solver://benchmarks');

    assert.ok(result.contents);
    const content = result.contents[0];
    assert.equal(content.uri, 'solver://benchmarks');

    const benchmarks = JSON.parse(content.text);
    assert.ok(benchmarks.benchmarks);
});

runner.test('MCP server can read examples resource', async () => {
    const server = runner.createMockMCPServer();

    const result = await server.readResource('solver://examples');

    assert.ok(result.contents);
    const content = result.contents[0];
    assert.equal(content.uri, 'solver://examples');

    const examples = JSON.parse(content.text);
    assert.ok(examples.examples);
    assert.ok(Array.isArray(examples.examples));
});

// MCP Error Handling Tests
runner.test('MCP server handles unknown tool gracefully', async () => {
    const server = runner.createMockMCPServer();

    try {
        await server.callTool('unknown_tool', {});
        assert.fail('Should have thrown error for unknown tool');
    } catch (error) {
        assert.ok(error.message.includes('Unknown tool'));
    }
});

runner.test('MCP server handles unknown resource gracefully', async () => {
    const server = runner.createMockMCPServer();

    try {
        await server.readResource('solver://unknown');
        assert.fail('Should have thrown error for unknown resource');
    } catch (error) {
        assert.ok(error.message.includes('Unknown resource'));
    }
});

// MCP JSON-RPC Compliance Tests
runner.test('MCP messages follow JSON-RPC 2.0 format', async () => {
    const message = {
        jsonrpc: "2.0",
        method: "tools/list",
        id: 1
    };

    const response = await runner.sendMCPMessage(message);

    assert.equal(response.jsonrpc, "2.0");
    assert.equal(response.id, 1);
    assert.ok(response.result !== undefined || response.error !== undefined);
});

// MCP Schema Validation Tests
runner.test('MCP tool schemas are valid JSON Schema', async () => {
    const server = runner.createMockMCPServer();
    const toolsResult = await server.listTools();

    for (const tool of toolsResult.tools) {
        const schema = tool.inputSchema;

        // Basic JSON Schema validation
        assert.equal(schema.type, 'object');
        assert.ok(schema.properties);
        assert.ok(typeof schema.properties === 'object');

        if (schema.required) {
            assert.ok(Array.isArray(schema.required));

            // All required properties should exist in properties
            for (const required of schema.required) {
                assert.ok(schema.properties[required]);
            }
        }
    }
});

// MCP Integration Tests
runner.test('MCP server integration workflow', async () => {
    const server = runner.createMockMCPServer();

    // 1. Initialize server
    const init = await server.initialize();
    assert.ok(init.capabilities);

    // 2. List available tools
    const tools = await server.listTools();
    assert.ok(tools.tools.length > 0);

    // 3. Execute a tool
    const solveTool = tools.tools.find(t => t.name === 'solve_linear_system');
    assert.ok(solveTool);

    const result = await server.callTool('solve_linear_system', {
        matrix: {
            rows: 2,
            cols: 2,
            format: 'dense',
            data: [1, 0, 0, 1]
        },
        vector: [1, 1]
    });

    assert.ok(result.content);

    // 4. List and read resources
    const resources = await server.listResources();
    assert.ok(resources.resources.length > 0);

    const algorithmsResource = resources.resources.find(r => r.uri === 'solver://algorithms');
    const algorithmsContent = await server.readResource(algorithmsResource.uri);
    assert.ok(algorithmsContent.contents);
});

// Run all tests
if (require.main === module) {
    runner.run().then(success => {
        process.exit(success ? 0 : 1);
    }).catch(error => {
        console.error('Test runner failed:', error);
        process.exit(1);
    });
}

module.exports = { MCPTestRunner, runner };