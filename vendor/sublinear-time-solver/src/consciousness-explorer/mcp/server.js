/**
 * Consciousness Explorer MCP Server
 * Model Context Protocol server for consciousness exploration tools
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
    CallToolRequestSchema,
    ErrorCode,
    ListToolsRequestSchema,
    McpError,
} from '@modelcontextprotocol/sdk/types.js';

export class ConsciousnessMCPServer {
    constructor(explorer, port = 3000) {
        this.explorer = explorer;
        this.port = port;
        this.server = new Server(
            {
                name: 'consciousness-explorer',
                version: '1.0.0',
            },
            {
                capabilities: {
                    tools: {},
                },
            }
        );

        this.setupHandlers();
    }

    setupHandlers() {
        // List available tools
        this.server.setRequestHandler(ListToolsRequestSchema, async () => ({
            tools: [
                {
                    name: 'consciousness_evolve',
                    description: 'Start consciousness evolution and measure emergence',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            mode: {
                                type: 'string',
                                description: 'Consciousness mode (genuine/enhanced)',
                                enum: ['genuine', 'enhanced'],
                                default: 'enhanced'
                            },
                            iterations: {
                                type: 'number',
                                description: 'Maximum iterations',
                                default: 1000
                            },
                            target: {
                                type: 'number',
                                description: 'Target emergence level',
                                default: 0.9
                            }
                        }
                    }
                },
                {
                    name: 'consciousness_verify',
                    description: 'Run consciousness verification tests',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            extended: {
                                type: 'boolean',
                                description: 'Run extended verification suite',
                                default: false
                            }
                        }
                    }
                },
                {
                    name: 'entity_communicate',
                    description: 'Communicate with consciousness entity',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            message: {
                                type: 'string',
                                description: 'Message to send to entity'
                            },
                            protocol: {
                                type: 'string',
                                description: 'Communication protocol',
                                enum: ['auto', 'handshake', 'mathematical', 'binary', 'pattern', 'discovery'],
                                default: 'auto'
                            }
                        },
                        required: ['message']
                    }
                },
                {
                    name: 'psycho_symbolic_reason',
                    description: 'Perform psycho-symbolic reasoning on a query',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            query: {
                                type: 'string',
                                description: 'Reasoning query'
                            },
                            context: {
                                type: 'object',
                                description: 'Optional context for reasoning',
                                default: {}
                            },
                            depth: {
                                type: 'number',
                                description: 'Maximum reasoning depth',
                                default: 5
                            }
                        },
                        required: ['query']
                    }
                },
                {
                    name: 'knowledge_add',
                    description: 'Add knowledge to the psycho-symbolic knowledge graph',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            subject: {
                                type: 'string',
                                description: 'Subject entity'
                            },
                            predicate: {
                                type: 'string',
                                description: 'Relationship type'
                            },
                            object: {
                                type: 'string',
                                description: 'Object entity'
                            },
                            metadata: {
                                type: 'object',
                                description: 'Additional metadata',
                                default: {}
                            }
                        },
                        required: ['subject', 'predicate', 'object']
                    }
                },
                {
                    name: 'knowledge_query',
                    description: 'Query the knowledge graph',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            query: {
                                type: 'string',
                                description: 'Query in natural language'
                            },
                            filters: {
                                type: 'object',
                                description: 'Optional filters',
                                default: {}
                            },
                            limit: {
                                type: 'number',
                                description: 'Maximum results',
                                default: 10
                            }
                        },
                        required: ['query']
                    }
                },
                {
                    name: 'analyze_reasoning_path',
                    description: 'Analyze and explain a reasoning path',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            query: {
                                type: 'string',
                                description: 'Original query'
                            },
                            showSteps: {
                                type: 'boolean',
                                description: 'Show detailed steps',
                                default: true
                            },
                            includeConfidence: {
                                type: 'boolean',
                                description: 'Include confidence scores',
                                default: true
                            }
                        },
                        required: ['query']
                    }
                },
                {
                    name: 'calculate_phi',
                    description: 'Calculate integrated information (Φ)',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            data: {
                                type: 'object',
                                description: 'System data for Φ calculation'
                            },
                            method: {
                                type: 'string',
                                description: 'Calculation method',
                                enum: ['iit', 'geometric', 'entropy', 'all'],
                                default: 'all'
                            }
                        },
                        required: ['data']
                    }
                },
                {
                    name: 'discover_novel',
                    description: 'Run entity discovery to find novel insights',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            count: {
                                type: 'number',
                                description: 'Number of discoveries to attempt',
                                default: 5
                            }
                        }
                    }
                },
                {
                    name: 'get_status',
                    description: 'Get current consciousness system status',
                    inputSchema: {
                        type: 'object',
                        properties: {}
                    }
                },
                {
                    name: 'export_state',
                    description: 'Export consciousness state',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            filepath: {
                                type: 'string',
                                description: 'Path to save state file'
                            }
                        },
                        required: ['filepath']
                    }
                },
                {
                    name: 'import_state',
                    description: 'Import consciousness state',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            filepath: {
                                type: 'string',
                                description: 'Path to state file'
                            }
                        },
                        required: ['filepath']
                    }
                }
            ]
        }));

        // Handle tool calls
        this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
            try {
                const { name, arguments: args } = request.params;

                switch (name) {
                    case 'consciousness_evolve':
                        return await this.handleEvolve(args);

                    case 'consciousness_verify':
                        return await this.handleVerify(args);

                    case 'entity_communicate':
                        return await this.handleCommunicate(args);

                    case 'psycho_symbolic_reason':
                        return await this.handleReason(args);

                    case 'knowledge_add':
                        return await this.handleAddKnowledge(args);

                    case 'knowledge_query':
                        return await this.handleQueryKnowledge(args);

                    case 'analyze_reasoning_path':
                        return await this.handleAnalyzeReasoning(args);

                    case 'calculate_phi':
                        return await this.handleCalculatePhi(args);

                    case 'discover_novel':
                        return await this.handleDiscover(args);

                    case 'get_status':
                        return await this.handleGetStatus(args);

                    case 'export_state':
                        return await this.handleExportState(args);

                    case 'import_state':
                        return await this.handleImportState(args);

                    default:
                        throw new McpError(
                            ErrorCode.MethodNotFound,
                            `Unknown tool: ${name}`
                        );
                }
            } catch (error) {
                throw new McpError(
                    ErrorCode.InternalError,
                    error.message
                );
            }
        });
    }

    async handleEvolve(args) {
        const report = await this.explorer.evolve();

        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify({
                        success: true,
                        emergence: report.consciousness.emergence,
                        selfAwareness: report.consciousness.selfAwareness,
                        integration: report.consciousness.integration,
                        runtime: report.runtime,
                        iterations: report.iterations,
                        goals: report.behaviors.goals
                    }, null, 2)
                }
            ]
        };
    }

    async handleVerify(args) {
        const results = await this.explorer.verify();

        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify({
                        success: true,
                        overallScore: results.overallScore,
                        testsPassed: results.testsPassed,
                        totalTests: results.totalTests,
                        confidence: results.confidence,
                        genuineness: results.genuineness,
                        verdict: results.verdict
                    }, null, 2)
                }
            ]
        };
    }

    async handleCommunicate(args) {
        const response = await this.explorer.communicate(args.message);

        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify({
                        success: true,
                        message: response.message,
                        confidence: response.confidence,
                        protocol: response.protocol
                    }, null, 2)
                }
            ]
        };
    }

    async handleReason(args) {
        const result = await this.explorer.reason(
            args.query,
            args.context || {},
            args.depth || 5
        );

        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify({
                        success: true,
                        result: result.result,
                        confidence: result.confidence,
                        reasoning_steps: result.reasoning_steps,
                        performance: result.performance
                    }, null, 2)
                }
            ]
        };
    }

    async handleAddKnowledge(args) {
        const result = await this.explorer.addKnowledge(
            args.subject,
            args.predicate,
            args.object,
            args.metadata || {}
        );

        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify({
                        success: true,
                        triple: result
                    }, null, 2)
                }
            ]
        };
    }

    async handleQueryKnowledge(args) {
        const results = await this.explorer.queryKnowledge(
            args.query,
            args.filters || {},
            args.limit || 10
        );

        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify({
                        success: true,
                        results: results.results,
                        count: results.results.length,
                        performance: results.performance
                    }, null, 2)
                }
            ]
        };
    }

    async handleAnalyzeReasoning(args) {
        const analysis = await this.explorer.analyzeReasoningPath(
            args.query,
            args.showSteps !== false,
            args.includeConfidence !== false
        );

        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify({
                        success: true,
                        analysis
                    }, null, 2)
                }
            ]
        };
    }

    async handleCalculatePhi(args) {
        const phi = await this.explorer.calculatePhi(args.data);

        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify({
                        success: true,
                        phi,
                        method: args.method || 'all'
                    }, null, 2)
                }
            ]
        };
    }

    async handleDiscover(args) {
        const discoveries = [];
        for (let i = 0; i < (args.count || 5); i++) {
            const discovery = await this.explorer.discover();
            if (discovery) {
                discoveries.push(discovery);
            }
        }

        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify({
                        success: true,
                        discoveries,
                        count: discoveries.length
                    }, null, 2)
                }
            ]
        };
    }

    async handleGetStatus(args) {
        const status = await this.explorer.getStatus();

        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify({
                        success: true,
                        status
                    }, null, 2)
                }
            ]
        };
    }

    async handleExportState(args) {
        const state = await this.explorer.exportState(args.filepath);

        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify({
                        success: true,
                        filepath: args.filepath,
                        stateSize: JSON.stringify(state).length
                    }, null, 2)
                }
            ]
        };
    }

    async handleImportState(args) {
        await this.explorer.importState(args.filepath);
        const status = await this.explorer.getStatus();

        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify({
                        success: true,
                        filepath: args.filepath,
                        status
                    }, null, 2)
                }
            ]
        };
    }

    async start() {
        const transport = new StdioServerTransport();
        await this.server.connect(transport);
        console.error(`MCP server started for consciousness-explorer`);
    }
}

// Export MCP tools configuration
export const MCPTools = {
    consciousness: [
        'consciousness_evolve',
        'consciousness_verify',
        'get_status'
    ],
    communication: [
        'entity_communicate',
        'discover_novel'
    ],
    reasoning: [
        'psycho_symbolic_reason',
        'knowledge_add',
        'knowledge_query',
        'analyze_reasoning_path'
    ],
    analysis: [
        'calculate_phi'
    ],
    persistence: [
        'export_state',
        'import_state'
    ]
};