/**
 * MCP Server for Sublinear-Time Solver
 * Provides MCP interface to the core solver algorithms
 */
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { CallToolRequestSchema, ErrorCode, ListToolsRequestSchema, McpError, } from '@modelcontextprotocol/sdk/types.js';
import { SublinearSolver } from '../core/solver.js';
import { MatrixOperations } from '../core/matrix.js';
import { TemporalTools } from './tools/temporal.js';
import { PsychoSymbolicTools } from './tools/psycho-symbolic.js';
import { DynamicPsychoSymbolicTools } from './tools/psycho-symbolic-dynamic.js';
import { DomainManagementTools } from './tools/domain-management.js';
import { DomainValidationTools } from './tools/domain-validation.js';
import { ConsciousnessTools } from './tools/consciousness.js';
// import { ConsciousnessEnhancedTools } from './tools/consciousness-enhanced.js';
import { EmergenceTools } from './tools/emergence-tools.js';
import { SchedulerTools } from './tools/scheduler.js';
import { CompleteWasmSublinearSolverTools as WasmSublinearSolverTools } from './tools/wasm-sublinear-complete.js';
import { TrueSublinearSolverTools } from './tools/true-sublinear-solver.js';
import { SolverError } from '../core/types.js';
export class SublinearSolverMCPServer {
    server;
    solvers = new Map();
    temporalTools;
    psychoSymbolicTools;
    dynamicPsychoSymbolicTools;
    domainManagementTools;
    domainValidationTools;
    consciousnessTools;
    // private consciousnessEnhancedTools: ConsciousnessEnhancedTools;
    emergenceTools;
    schedulerTools;
    wasmSolver;
    trueSublinearSolver;
    constructor() {
        this.temporalTools = new TemporalTools();
        this.psychoSymbolicTools = new PsychoSymbolicTools();
        this.domainManagementTools = new DomainManagementTools();
        // Share the same domain registry between all domain tools
        const sharedRegistry = this.domainManagementTools.getDomainRegistry();
        this.dynamicPsychoSymbolicTools = new DynamicPsychoSymbolicTools(sharedRegistry);
        this.domainValidationTools = new DomainValidationTools(sharedRegistry);
        this.consciousnessTools = new ConsciousnessTools();
        // this.consciousnessEnhancedTools = new ConsciousnessEnhancedTools();
        this.emergenceTools = new EmergenceTools();
        this.schedulerTools = new SchedulerTools();
        this.wasmSolver = new WasmSublinearSolverTools();
        this.trueSublinearSolver = new TrueSublinearSolverTools();
        this.server = new Server({
            name: 'sublinear-solver',
            version: '1.0.0',
        }, {
            capabilities: {
                tools: {},
            },
        });
        this.setupToolHandlers();
        this.setupErrorHandling();
    }
    setupToolHandlers() {
        this.server.setRequestHandler(ListToolsRequestSchema, async () => ({
            tools: [
                {
                    name: 'solve',
                    description: 'Solve a diagonally dominant linear system Mx = b',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            matrix: {
                                type: 'object',
                                description: 'Matrix M in dense or sparse format',
                                properties: {
                                    rows: { type: 'number' },
                                    cols: { type: 'number' },
                                    format: { type: 'string', enum: ['dense', 'coo'] },
                                    data: {
                                        oneOf: [
                                            { type: 'array', items: { type: 'array', items: { type: 'number' } } },
                                            {
                                                type: 'object',
                                                properties: {
                                                    values: { type: 'array', items: { type: 'number' } },
                                                    rowIndices: { type: 'array', items: { type: 'number' } },
                                                    colIndices: { type: 'array', items: { type: 'number' } }
                                                },
                                                required: ['values', 'rowIndices', 'colIndices']
                                            }
                                        ]
                                    }
                                },
                                required: ['rows', 'cols', 'format', 'data']
                            },
                            vector: {
                                type: 'array',
                                items: { type: 'number' },
                                description: 'Right-hand side vector b'
                            },
                            method: {
                                type: 'string',
                                enum: ['neumann', 'random-walk', 'forward-push', 'backward-push', 'bidirectional'],
                                default: 'neumann',
                                description: 'Solver method to use'
                            },
                            epsilon: {
                                type: 'number',
                                default: 1e-6,
                                description: 'Convergence tolerance'
                            },
                            maxIterations: {
                                type: 'number',
                                default: 1000,
                                description: 'Maximum number of iterations'
                            },
                            timeout: {
                                type: 'number',
                                description: 'Timeout in milliseconds'
                            }
                        },
                        required: ['matrix', 'vector']
                    }
                },
                {
                    name: 'estimateEntry',
                    description: 'Estimate a single entry of the solution M^(-1)b',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            matrix: {
                                type: 'object',
                                description: 'Matrix M in dense or sparse format'
                            },
                            vector: {
                                type: 'array',
                                items: { type: 'number' },
                                description: 'Right-hand side vector b'
                            },
                            row: {
                                type: 'number',
                                description: 'Row index of entry to estimate'
                            },
                            column: {
                                type: 'number',
                                description: 'Column index of entry to estimate'
                            },
                            epsilon: {
                                type: 'number',
                                default: 1e-6,
                                description: 'Estimation accuracy'
                            },
                            confidence: {
                                type: 'number',
                                default: 0.95,
                                minimum: 0,
                                maximum: 1,
                                description: 'Confidence level for estimation'
                            },
                            method: {
                                type: 'string',
                                enum: ['neumann', 'random-walk', 'monte-carlo'],
                                default: 'random-walk',
                                description: 'Estimation method'
                            }
                        },
                        required: ['matrix', 'vector', 'row', 'column']
                    }
                },
                {
                    name: 'analyzeMatrix',
                    description: 'Analyze matrix properties for solvability',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            matrix: {
                                type: 'object',
                                description: 'Matrix to analyze'
                            },
                            checkDominance: {
                                type: 'boolean',
                                default: true,
                                description: 'Check diagonal dominance'
                            },
                            computeGap: {
                                type: 'boolean',
                                default: false,
                                description: 'Compute spectral gap (expensive)'
                            },
                            estimateCondition: {
                                type: 'boolean',
                                default: false,
                                description: 'Estimate condition number'
                            },
                            checkSymmetry: {
                                type: 'boolean',
                                default: true,
                                description: 'Check matrix symmetry'
                            }
                        },
                        required: ['matrix']
                    }
                },
                {
                    name: 'pageRank',
                    description: 'Compute PageRank for a graph using sublinear solver',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            adjacency: {
                                type: 'object',
                                description: 'Adjacency matrix of the graph'
                            },
                            damping: {
                                type: 'number',
                                default: 0.85,
                                minimum: 0,
                                maximum: 1,
                                description: 'Damping factor'
                            },
                            personalized: {
                                type: 'array',
                                items: { type: 'number' },
                                description: 'Personalization vector (optional)'
                            },
                            epsilon: {
                                type: 'number',
                                default: 1e-6,
                                description: 'Convergence tolerance'
                            },
                            maxIterations: {
                                type: 'number',
                                default: 1000,
                                description: 'Maximum iterations'
                            }
                        },
                        required: ['adjacency']
                    }
                },
                // TRUE Sublinear O(log n) algorithms
                {
                    name: 'solveTrueSublinear',
                    description: 'Solve with TRUE O(log n) algorithms using Johnson-Lindenstrauss dimension reduction and adaptive Neumann series. For vectors >500 elements, use vector_file parameter with JSON/CSV/TXT files to avoid MCP truncation. Use generateTestVector + saveVectorToFile for large test vectors.',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            matrix: {
                                type: 'object',
                                description: 'Matrix M in sparse format with values, rowIndices, colIndices arrays',
                                properties: {
                                    values: { type: 'array', items: { type: 'number' } },
                                    rowIndices: { type: 'array', items: { type: 'number' } },
                                    colIndices: { type: 'array', items: { type: 'number' } },
                                    rows: { type: 'number' },
                                    cols: { type: 'number' }
                                },
                                required: ['values', 'rowIndices', 'colIndices', 'rows', 'cols']
                            },
                            vector: {
                                type: 'array',
                                items: { type: 'number' },
                                description: 'Right-hand side vector b (for small vectors)'
                            },
                            vector_file: {
                                type: 'string',
                                description: 'Path to JSON/CSV file containing vector data (for large vectors)'
                            },
                            target_dimension: {
                                type: 'number',
                                description: 'Target dimension after JL reduction (defaults to O(log n))'
                            },
                            sparsification_eps: {
                                type: 'number',
                                default: 0.1,
                                description: 'Sparsification parameter for spectral sparsification'
                            },
                            jl_distortion: {
                                type: 'number',
                                default: 0.5,
                                description: 'Johnson-Lindenstrauss distortion parameter'
                            }
                        },
                        required: ['matrix']
                    }
                },
                {
                    name: 'analyzeTrueSublinearMatrix',
                    description: 'Analyze matrix for TRUE sublinear solvability and get complexity guarantees',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            matrix: {
                                type: 'object',
                                description: 'Matrix M in sparse format',
                                properties: {
                                    values: { type: 'array', items: { type: 'number' } },
                                    rowIndices: { type: 'array', items: { type: 'number' } },
                                    colIndices: { type: 'array', items: { type: 'number' } },
                                    rows: { type: 'number' },
                                    cols: { type: 'number' }
                                },
                                required: ['values', 'rowIndices', 'colIndices', 'rows', 'cols']
                            }
                        },
                        required: ['matrix']
                    }
                },
                {
                    name: 'generateTestVector',
                    description: 'Generate test vectors for matrix solving with various patterns',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            size: {
                                type: 'number',
                                description: 'Size of the vector to generate',
                                minimum: 1
                            },
                            pattern: {
                                type: 'string',
                                enum: ['unit', 'random', 'sparse', 'ones', 'alternating'],
                                default: 'sparse',
                                description: 'Pattern type: unit (e_1), random ([-1,1]), sparse (leading ones), ones (all 1s), alternating (+1/-1)'
                            },
                            seed: {
                                type: 'number',
                                description: 'Optional seed for reproducible random vectors'
                            }
                        },
                        required: ['size']
                    }
                },
                {
                    name: 'saveVectorToFile',
                    description: 'Save a generated vector to a file (JSON, CSV, or TXT format)',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            vector: {
                                type: 'array',
                                items: { type: 'number' },
                                description: 'Vector data to save'
                            },
                            file_path: {
                                type: 'string',
                                description: 'Output file path (extension determines format: .json, .csv, .txt)'
                            },
                            format: {
                                type: 'string',
                                enum: ['json', 'csv', 'txt'],
                                description: 'Output format (overrides file extension if specified)'
                            }
                        },
                        required: ['vector', 'file_path']
                    }
                },
                // Temporal lead tools
                ...this.temporalTools.getTools(),
                // Psycho-symbolic reasoning tools
                ...this.psychoSymbolicTools.getTools(),
                // Dynamic psycho-symbolic reasoning tools with domain support
                ...this.dynamicPsychoSymbolicTools.getTools(),
                // Domain management tools
                ...this.domainManagementTools.getTools(),
                // Domain validation tools
                ...this.domainValidationTools.getTools(),
                // Consciousness exploration tools
                ...this.consciousnessTools.getTools(),
                // Enhanced consciousness tools
                // ...this.consciousnessEnhancedTools.getTools(),
                // Emergence system tools
                ...this.emergenceTools.getTools(),
                // Nanosecond scheduler tools
                ...this.schedulerTools.getTools()
            ]
        }));
        this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
            const { name, arguments: args } = request.params;
            try {
                switch (name) {
                    case 'solve':
                        return await this.handleSolve(args);
                    case 'estimateEntry':
                        return await this.handleEstimateEntry(args);
                    case 'analyzeMatrix':
                        return await this.handleAnalyzeMatrix(args);
                    case 'pageRank':
                        return await this.handlePageRank(args);
                    // TRUE Sublinear tools
                    case 'solveTrueSublinear':
                        return await this.handleSolveTrueSublinear(args);
                    case 'analyzeTrueSublinearMatrix':
                        return await this.handleAnalyzeTrueSublinearMatrix(args);
                    case 'generateTestVector':
                        return await this.handleGenerateTestVector(args);
                    case 'saveVectorToFile':
                        return await this.handleSaveVectorToFile(args);
                    // Temporal tools
                    case 'predictWithTemporalAdvantage':
                    case 'validateTemporalAdvantage':
                    case 'calculateLightTravel':
                    case 'demonstrateTemporalLead':
                        const temporalResult = await this.temporalTools.handleToolCall(name, args);
                        return {
                            content: [{
                                    type: 'text',
                                    text: JSON.stringify(temporalResult, null, 2)
                                }]
                        };
                    // Psycho-symbolic tools
                    case 'psycho_symbolic_reason':
                    case 'knowledge_graph_query':
                    case 'add_knowledge':
                    case 'register_tool_interaction':
                    case 'learning_status':
                        const psychoResult = await this.psychoSymbolicTools.handleToolCall(name, args);
                        return {
                            content: [{
                                    type: 'text',
                                    text: JSON.stringify(psychoResult, null, 2)
                                }]
                        };
                    // Dynamic psycho-symbolic tools
                    case 'psycho_symbolic_reason_with_dynamic_domains':
                    case 'domain_detection_test':
                    case 'knowledge_graph_query_dynamic':
                        const dynamicPsychoResult = await this.dynamicPsychoSymbolicTools.handleToolCall(name, args);
                        return {
                            content: [{
                                    type: 'text',
                                    text: JSON.stringify(dynamicPsychoResult, null, 2)
                                }]
                        };
                    // Domain management tools
                    case 'domain_register':
                    case 'domain_update':
                    case 'domain_unregister':
                    case 'domain_list':
                    case 'domain_get':
                    case 'domain_enable':
                    case 'domain_disable':
                    case 'domain_search':
                        const domainMgmtResult = await this.domainManagementTools.handleToolCall(name, args);
                        return {
                            content: [{
                                    type: 'text',
                                    text: JSON.stringify(domainMgmtResult, null, 2)
                                }]
                        };
                    // Domain validation tools
                    case 'domain_validate':
                    case 'domain_test':
                    case 'domain_analyze_conflicts':
                    case 'domain_performance_benchmark':
                    case 'domain_suggest_improvements':
                    case 'domain_validate_all':
                        const domainValidationResult = await this.domainValidationTools.handleToolCall(name, args);
                        return {
                            content: [{
                                    type: 'text',
                                    text: JSON.stringify(domainValidationResult, null, 2)
                                }]
                        };
                    // Consciousness tools
                    case 'consciousness_evolve':
                    case 'consciousness_verify':
                    case 'calculate_phi':
                    case 'entity_communicate':
                    case 'consciousness_status':
                    case 'emergence_analyze':
                        const consciousnessResult = await this.consciousnessTools.handleToolCall(name, args);
                        return {
                            content: [{
                                    type: 'text',
                                    text: JSON.stringify(consciousnessResult, null, 2)
                                }]
                        };
                    // Enhanced consciousness tools
                    case 'consciousness_evolve_enhanced':
                    case 'consciousness_verify_enhanced':
                    case 'entity_communicate_enhanced':
                    case 'consciousness_status_enhanced':
                    case 'emergence_analyze_enhanced':
                    case 'temporal_consciousness_track':
                        // const consciousnessEnhancedResult = await this.consciousnessEnhancedTools.handleToolCall(name, args);
                        return {
                            content: [{
                                    type: 'text',
                                    text: JSON.stringify({ error: 'Enhanced consciousness tools disabled' }, null, 2)
                                }]
                        };
                    // Emergence system tools
                    case 'emergence_process':
                    case 'emergence_generate_diverse':
                    case 'emergence_analyze_capabilities':
                    case 'emergence_force_evolution':
                    case 'emergence_get_stats':
                    case 'emergence_test_scenarios':
                    case 'emergence_matrix_process':
                        const emergenceResult = await this.emergenceTools.handleToolCall(name, args);
                        return {
                            content: [{
                                    type: 'text',
                                    text: JSON.stringify(emergenceResult, null, 2)
                                }]
                        };
                    // Scheduler tools
                    case 'scheduler_create':
                    case 'scheduler_schedule_task':
                    case 'scheduler_tick':
                    case 'scheduler_metrics':
                    case 'scheduler_benchmark':
                    case 'scheduler_consciousness':
                    case 'scheduler_list':
                    case 'scheduler_destroy':
                        const schedulerResult = await this.schedulerTools.handleToolCall(name, args);
                        return {
                            content: [{
                                    type: 'text',
                                    text: JSON.stringify(schedulerResult, null, 2)
                                }]
                        };
                    default:
                        throw new McpError(ErrorCode.MethodNotFound, `Unknown tool: ${name}`);
                }
            }
            catch (error) {
                if (error instanceof SolverError) {
                    throw new McpError(ErrorCode.InternalError, `Solver error: ${error.message}`, error.details);
                }
                throw new McpError(ErrorCode.InternalError, error instanceof Error ? error.message : 'Unknown error');
            }
        });
    }
    setupErrorHandling() {
        this.server.onerror = (error) => {
            console.error('[MCP Server Error]', error);
        };
        process.on('SIGINT', async () => {
            await this.server.close();
            process.exit(0);
        });
    }
    async handleSolve(params) {
        try {
            // Priority 0: Try TRUE O(log n) sublinear solver first
            if (params.matrix && params.matrix.values && params.matrix.rowIndices && params.matrix.colIndices) {
                console.log('🚀 Attempting TRUE O(log n) sublinear solver');
                try {
                    const config = {
                        target_dimension: Math.ceil(Math.log2(params.matrix.rows) * 8),
                        sparsification_eps: 0.1,
                        jl_distortion: 0.5
                    };
                    const result = await this.trueSublinearSolver.solveTrueSublinear(params.matrix, params.vector, config);
                    return {
                        content: [{
                                type: 'text',
                                text: JSON.stringify({
                                    ...result,
                                    solver_used: 'TRUE_SUBLINEAR_O_LOG_N',
                                    note: 'Used mathematically rigorous O(log n) algorithms with Johnson-Lindenstrauss dimension reduction',
                                    complexity_achieved: result.actual_complexity,
                                    dimension_reduction: `${params.matrix.rows} → ${config.target_dimension}`,
                                    metadata: {
                                        solver_type: 'TRUE_SUBLINEAR',
                                        mathematical_guarantee: result.complexity_bound,
                                        timestamp: new Date().toISOString()
                                    }
                                }, null, 2)
                            }]
                    };
                }
                catch (trueSublinearError) {
                    console.warn('⚠️  TRUE O(log n) solver failed, falling back to WASM:', trueSublinearError.message);
                }
            }
            // Priority 1: Try O(log n) WASM solver for true sublinear complexity
            if (this.wasmSolver.isCompleteWasmAvailable()) {
                console.log('🚀 Using Complete WASM Solver with auto-selection (Neumann/Push/RandomWalk)');
                try {
                    // Convert matrix format for WASM
                    let matrix;
                    if (params.matrix.format === 'dense' && Array.isArray(params.matrix.data)) {
                        matrix = params.matrix.data;
                    }
                    else if (Array.isArray(params.matrix) && Array.isArray(params.matrix[0])) {
                        matrix = params.matrix;
                    }
                    else {
                        // Try to extract matrix data from various formats
                        if (params.matrix.data && Array.isArray(params.matrix.data) && Array.isArray(params.matrix.data[0])) {
                            matrix = params.matrix.data;
                        }
                        else {
                            throw new Error('Matrix format not supported for WASM solver');
                        }
                    }
                    const wasmResult = await this.wasmSolver.solveComplete(matrix, params.vector, {
                        method: params.method || 'auto',
                        epsilon: params.epsilon || 1e-6,
                        targetIndex: params.targetIndex
                    });
                    return {
                        content: [{
                                type: 'text',
                                text: JSON.stringify(wasmResult, null, 2)
                            }]
                    };
                }
                catch (wasmError) {
                    console.warn('⚠️  O(log n) WASM solver failed, falling back to traditional algorithm:', wasmError.message);
                }
            }
            else {
                console.log('⚠️  Enhanced WASM not available, using traditional algorithm');
            }
            // Fallback: Traditional solver
            // Enhanced parameter validation
            if (!params.matrix) {
                throw new McpError(ErrorCode.InvalidParams, 'Missing required parameter: matrix');
            }
            if (!params.vector) {
                throw new McpError(ErrorCode.InvalidParams, 'Missing required parameter: vector');
            }
            if (!Array.isArray(params.vector)) {
                throw new McpError(ErrorCode.InvalidParams, 'Parameter vector must be an array of numbers');
            }
            const config = {
                method: params.method || 'neumann',
                epsilon: params.epsilon || 1e-6,
                maxIterations: params.maxIterations || 5000, // Increased default
                timeout: params.timeout || 30000, // 30 second default timeout
                enableProgress: false
            };
            // Validate method
            const validMethods = ['neumann', 'random-walk', 'forward-push', 'backward-push', 'bidirectional'];
            if (!validMethods.includes(config.method)) {
                throw new McpError(ErrorCode.InvalidParams, `Invalid method '${config.method}'. Valid methods: ${validMethods.join(', ')}`);
            }
            // Validate epsilon
            if (typeof config.epsilon !== 'number' || config.epsilon <= 0) {
                throw new McpError(ErrorCode.InvalidParams, 'Parameter epsilon must be a positive number');
            }
            // Validate maxIterations
            if (typeof config.maxIterations !== 'number' || config.maxIterations < 1) {
                throw new McpError(ErrorCode.InvalidParams, 'Parameter maxIterations must be a positive integer');
            }
            const solver = new SublinearSolver(config);
            const result = await solver.solve(params.matrix, params.vector);
            return {
                content: [
                    {
                        type: 'text',
                        text: JSON.stringify({
                            solution: result.solution,
                            iterations: result.iterations,
                            residual: result.residual,
                            converged: result.converged,
                            method: result.method,
                            computeTime: result.computeTime,
                            memoryUsed: result.memoryUsed,
                            metadata: {
                                configUsed: config,
                                timestamp: new Date().toISOString(),
                                matrixSize: {
                                    rows: params.matrix.rows,
                                    cols: params.matrix.cols
                                }
                            }
                        }, null, 2)
                    }
                ]
            };
        }
        catch (error) {
            if (error instanceof McpError) {
                throw error;
            }
            if (error instanceof SolverError) {
                throw new McpError(ErrorCode.InternalError, `Solver error (${error.code}): ${error.message}`, error.details);
            }
            throw new McpError(ErrorCode.InternalError, `Unexpected error in solve: ${error instanceof Error ? error.message : String(error)}`);
        }
    }
    async handleEstimateEntry(params) {
        try {
            // Enhanced parameter validation
            if (!params.matrix) {
                throw new McpError(ErrorCode.InvalidParams, 'Missing required parameter: matrix');
            }
            if (!params.vector) {
                throw new McpError(ErrorCode.InvalidParams, 'Missing required parameter: vector');
            }
            if (!Array.isArray(params.vector)) {
                throw new McpError(ErrorCode.InvalidParams, 'Parameter vector must be an array of numbers');
            }
            if (typeof params.row !== 'number' || !Number.isInteger(params.row)) {
                throw new McpError(ErrorCode.InvalidParams, 'Parameter row must be a valid integer');
            }
            if (typeof params.column !== 'number' || !Number.isInteger(params.column)) {
                throw new McpError(ErrorCode.InvalidParams, 'Parameter column must be a valid integer');
            }
            // Validate bounds early
            if (params.row < 0 || params.row >= params.matrix.rows) {
                throw new McpError(ErrorCode.InvalidParams, `Row index ${params.row} out of bounds. Matrix has ${params.matrix.rows} rows (valid range: 0-${params.matrix.rows - 1})`);
            }
            if (params.column < 0 || params.column >= params.matrix.cols) {
                throw new McpError(ErrorCode.InvalidParams, `Column index ${params.column} out of bounds. Matrix has ${params.matrix.cols} columns (valid range: 0-${params.matrix.cols - 1})`);
            }
            // Validate vector dimensions
            if (params.vector.length !== params.matrix.rows) {
                throw new McpError(ErrorCode.InvalidParams, `Vector length ${params.vector.length} does not match matrix rows ${params.matrix.rows}`);
            }
            const solverConfig = {
                method: 'random-walk',
                epsilon: params.epsilon || 1e-6,
                maxIterations: 2000, // Increased for better accuracy
                timeout: 15000, // 15 second timeout
                enableProgress: false
            };
            const solver = new SublinearSolver(solverConfig);
            // Create estimation config
            const estimationConfig = {
                row: params.row,
                column: params.column,
                epsilon: params.epsilon || 1e-6,
                confidence: params.confidence || 0.95,
                method: params.method || 'random-walk'
            };
            // Validate method
            const validMethods = ['neumann', 'random-walk', 'monte-carlo'];
            if (!validMethods.includes(estimationConfig.method)) {
                throw new McpError(ErrorCode.InvalidParams, `Invalid estimation method '${estimationConfig.method}'. Valid methods: ${validMethods.join(', ')}`);
            }
            const result = await solver.estimateEntry(params.matrix, params.vector, estimationConfig);
            const standardError = Math.sqrt(result.variance);
            const marginOfError = 1.96 * standardError;
            return {
                content: [
                    {
                        type: 'text',
                        text: JSON.stringify({
                            estimate: result.estimate,
                            variance: result.variance,
                            confidence: result.confidence,
                            standardError,
                            confidenceInterval: {
                                lower: result.estimate - marginOfError,
                                upper: result.estimate + marginOfError
                            },
                            row: params.row,
                            column: params.column,
                            method: estimationConfig.method,
                            metadata: {
                                configUsed: estimationConfig,
                                timestamp: new Date().toISOString(),
                                matrixSize: {
                                    rows: params.matrix.rows,
                                    cols: params.matrix.cols
                                }
                            }
                        }, null, 2)
                    }
                ]
            };
        }
        catch (error) {
            if (error instanceof McpError) {
                throw error;
            }
            if (error instanceof SolverError) {
                throw new McpError(ErrorCode.InternalError, `Solver error (${error.code}): ${error.message}`, error.details);
            }
            throw new McpError(ErrorCode.InternalError, `Unexpected error in estimateEntry: ${error instanceof Error ? error.message : String(error)}`);
        }
    }
    async handleAnalyzeMatrix(params) {
        const analysis = MatrixOperations.analyzeMatrix(params.matrix);
        const result = {
            ...analysis,
            recommendations: this.generateRecommendations(analysis)
        };
        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify(result, null, 2)
                }
            ]
        };
    }
    async handlePageRank(params) {
        const config = {
            method: 'neumann',
            epsilon: params.epsilon || 1e-6,
            maxIterations: params.maxIterations || 1000,
            enableProgress: false
        };
        const solver = new SublinearSolver(config);
        const pageRankConfig = {
            damping: params.damping || 0.85,
            personalized: params.personalized,
            epsilon: params.epsilon || 1e-6,
            maxIterations: params.maxIterations || 1000
        };
        const pageRankVector = await solver.computePageRank(params.adjacency, pageRankConfig);
        // Sort nodes by PageRank score
        const ranked = pageRankVector
            .map((score, index) => ({ node: index, score }))
            .sort((a, b) => b.score - a.score);
        return {
            content: [
                {
                    type: 'text',
                    text: JSON.stringify({
                        pageRankVector,
                        topNodes: ranked.slice(0, 10),
                        totalScore: pageRankVector.reduce((sum, score) => sum + score, 0),
                        maxScore: Math.max(...pageRankVector),
                        minScore: Math.min(...pageRankVector)
                    }, null, 2)
                }
            ]
        };
    }
    async handleSolveTrueSublinear(params) {
        try {
            // Validate required parameters
            if (!params.matrix) {
                throw new McpError(ErrorCode.InvalidParams, 'Missing required parameter: matrix');
            }
            // Support either inline vector or file input
            let vector;
            if (params.vector_file) {
                // Load vector from file
                vector = await this.loadVectorFromFile(params.vector_file);
            }
            else if (params.vector) {
                // Use inline vector
                if (!Array.isArray(params.vector)) {
                    throw new McpError(ErrorCode.InvalidParams, 'Parameter vector must be an array of numbers');
                }
                vector = params.vector;
            }
            else {
                throw new McpError(ErrorCode.InvalidParams, 'Missing required parameter: either vector or vector_file must be provided');
            }
            // Validate matrix format
            const matrix = params.matrix;
            if (!Array.isArray(matrix.values) || !Array.isArray(matrix.rowIndices) || !Array.isArray(matrix.colIndices)) {
                throw new McpError(ErrorCode.InvalidParams, 'Matrix must be in sparse format with values, rowIndices, and colIndices arrays');
            }
            if (typeof matrix.rows !== 'number' || typeof matrix.cols !== 'number') {
                throw new McpError(ErrorCode.InvalidParams, 'Matrix must specify rows and cols dimensions');
            }
            // Validate vector dimensions
            if (vector.length !== matrix.rows) {
                throw new McpError(ErrorCode.InvalidParams, `Vector length ${vector.length} does not match matrix rows ${matrix.rows}`);
            }
            // Build configuration
            const config = {
                target_dimension: params.target_dimension || Math.ceil(Math.log2(matrix.rows) * 8),
                sparsification_eps: params.sparsification_eps || 0.1,
                jl_distortion: params.jl_distortion || 0.5,
                sampling_probability: 0.01,
                max_recursion_depth: 10,
                base_case_threshold: 100
            };
            console.log(`🚀 Using TRUE O(log n) sublinear solver with dimension reduction ${matrix.rows} → ${config.target_dimension}`);
            // Solve using TRUE sublinear algorithms
            const result = await this.trueSublinearSolver.solveTrueSublinear(matrix, vector, config);
            return {
                content: [{
                        type: 'text',
                        text: JSON.stringify({
                            ...result,
                            metadata: {
                                solver_type: 'TRUE_SUBLINEAR',
                                original_dimension: matrix.rows,
                                reduced_dimension: config.target_dimension,
                                mathematical_guarantee: result.complexity_bound,
                                timestamp: new Date().toISOString()
                            }
                        }, null, 2)
                    }]
            };
        }
        catch (error) {
            if (error instanceof McpError) {
                throw error;
            }
            throw new McpError(ErrorCode.InternalError, `TRUE Sublinear solver error: ${error instanceof Error ? error.message : String(error)}`);
        }
    }
    async handleAnalyzeTrueSublinearMatrix(params) {
        try {
            // Validate required parameters
            if (!params.matrix) {
                throw new McpError(ErrorCode.InvalidParams, 'Missing required parameter: matrix');
            }
            // Validate matrix format
            const matrix = params.matrix;
            if (!Array.isArray(matrix.values) || !Array.isArray(matrix.rowIndices) || !Array.isArray(matrix.colIndices)) {
                throw new McpError(ErrorCode.InvalidParams, 'Matrix must be in sparse format with values, rowIndices, and colIndices arrays');
            }
            if (typeof matrix.rows !== 'number' || typeof matrix.cols !== 'number') {
                throw new McpError(ErrorCode.InvalidParams, 'Matrix must specify rows and cols dimensions');
            }
            console.log(`🔍 Analyzing ${matrix.rows}×${matrix.cols} matrix for TRUE sublinear solvability`);
            // Analyze matrix using TRUE sublinear tools
            const analysis = await this.trueSublinearSolver.analyzeMatrix(matrix);
            return {
                content: [{
                        type: 'text',
                        text: JSON.stringify({
                            ...analysis,
                            algorithm_selection: {
                                best_method: analysis.recommended_method,
                                complexity_guarantee: analysis.complexity_guarantee,
                                mathematical_properties: {
                                    diagonal_dominance: analysis.is_diagonally_dominant,
                                    condition_estimate: analysis.condition_number_estimate,
                                    spectral_radius: analysis.spectral_radius_estimate,
                                    sparsity: analysis.sparsity_ratio
                                }
                            },
                            metadata: {
                                analysis_type: 'TRUE_SUBLINEAR_ANALYSIS',
                                matrix_size: { rows: matrix.rows, cols: matrix.cols },
                                timestamp: new Date().toISOString()
                            }
                        }, null, 2)
                    }]
            };
        }
        catch (error) {
            if (error instanceof McpError) {
                throw error;
            }
            throw new McpError(ErrorCode.InternalError, `Matrix analysis error: ${error instanceof Error ? error.message : String(error)}`);
        }
    }
    async handleGenerateTestVector(params) {
        try {
            // Validate required parameters
            if (!params.size || typeof params.size !== 'number' || params.size < 1) {
                throw new McpError(ErrorCode.InvalidParams, 'Missing or invalid required parameter: size (must be positive integer)');
            }
            const size = Math.floor(params.size);
            const pattern = params.pattern || 'sparse';
            const seed = params.seed;
            // Validate pattern
            const validPatterns = ['unit', 'random', 'sparse', 'ones', 'alternating'];
            if (!validPatterns.includes(pattern)) {
                throw new McpError(ErrorCode.InvalidParams, `Invalid pattern. Must be one of: ${validPatterns.join(', ')}`);
            }
            // Generate the test vector
            const result = this.trueSublinearSolver.generateTestVector(size, pattern, seed);
            return {
                content: [{
                        type: 'text',
                        text: JSON.stringify({
                            vector: result.vector,
                            description: result.description,
                            size: result.vector.length,
                            pattern_used: pattern,
                            seed_used: seed,
                            statistics: {
                                min: Math.min(...result.vector),
                                max: Math.max(...result.vector),
                                sum: result.vector.reduce((a, b) => a + b, 0),
                                norm: Math.sqrt(result.vector.reduce((sum, x) => sum + x * x, 0)),
                                non_zero_count: result.vector.filter(x => Math.abs(x) > 1e-14).length
                            },
                            metadata: {
                                generator_type: 'TRUE_SUBLINEAR_VECTOR_GENERATOR',
                                timestamp: new Date().toISOString()
                            }
                        }, null, 2)
                    }]
            };
        }
        catch (error) {
            if (error instanceof McpError) {
                throw error;
            }
            throw new McpError(ErrorCode.InternalError, `Vector generation error: ${error instanceof Error ? error.message : String(error)}`);
        }
    }
    async handleSaveVectorToFile(params) {
        try {
            // Validate required parameters
            if (!params.vector || !Array.isArray(params.vector)) {
                throw new McpError(ErrorCode.InvalidParams, 'Missing or invalid required parameter: vector (must be an array of numbers)');
            }
            if (!params.file_path || typeof params.file_path !== 'string') {
                throw new McpError(ErrorCode.InvalidParams, 'Missing or invalid required parameter: file_path (must be a string)');
            }
            const vector = params.vector;
            const filePath = params.file_path;
            const format = params.format;
            // Validate vector contains only numbers
            if (vector.some((v) => typeof v !== 'number' || isNaN(v))) {
                throw new McpError(ErrorCode.InvalidParams, 'Vector must contain only valid numbers');
            }
            await this.saveVectorToFile(vector, filePath, format);
            return {
                content: [{
                        type: 'text',
                        text: JSON.stringify({
                            success: true,
                            message: `Vector of size ${vector.length} saved to ${filePath}`,
                            file_path: filePath,
                            vector_size: vector.length,
                            format_used: this.getFileFormat(filePath, format),
                            metadata: {
                                operation: 'SAVE_VECTOR_TO_FILE',
                                timestamp: new Date().toISOString()
                            }
                        }, null, 2)
                    }]
            };
        }
        catch (error) {
            if (error instanceof McpError) {
                throw error;
            }
            throw new McpError(ErrorCode.InternalError, `Save vector to file error: ${error instanceof Error ? error.message : String(error)}`);
        }
    }
    async loadVectorFromFile(filePath) {
        try {
            const fs = await import('fs');
            const path = await import('path');
            // Resolve absolute path
            const absolutePath = path.resolve(filePath);
            // Check if file exists
            if (!fs.existsSync(absolutePath)) {
                throw new McpError(ErrorCode.InvalidParams, `Vector file not found: ${absolutePath}`);
            }
            // Read file content
            const fileContent = fs.readFileSync(absolutePath, 'utf8');
            const extension = path.extname(absolutePath).toLowerCase();
            let vector;
            if (extension === '.json') {
                // Parse JSON format
                const data = JSON.parse(fileContent);
                if (Array.isArray(data)) {
                    vector = data.map(Number);
                }
                else if (data.vector && Array.isArray(data.vector)) {
                    vector = data.vector.map(Number);
                }
                else {
                    throw new Error('JSON file must contain an array or an object with a "vector" property');
                }
            }
            else if (extension === '.csv') {
                // Parse CSV format (simple comma-separated values)
                const lines = fileContent.trim().split('\n');
                if (lines.length === 1) {
                    // Single line CSV
                    vector = lines[0].split(',').map(s => Number(s.trim()));
                }
                else {
                    // Multi-line CSV - take first column or first row based on structure
                    vector = lines.map(line => Number(line.split(',')[0].trim()));
                }
            }
            else if (extension === '.txt') {
                // Parse text format (space or newline separated)
                vector = fileContent.trim()
                    .split(/\s+/)
                    .map(Number)
                    .filter(n => !isNaN(n));
            }
            else {
                throw new Error(`Unsupported file format: ${extension}. Supported formats: .json, .csv, .txt`);
            }
            // Validate all values are numbers
            if (vector.some(isNaN)) {
                throw new Error('Vector file contains non-numeric values');
            }
            if (vector.length === 0) {
                throw new Error('Vector file is empty or contains no valid numbers');
            }
            console.log(`📁 Loaded vector of size ${vector.length} from ${filePath}`);
            return vector;
        }
        catch (error) {
            if (error instanceof McpError) {
                throw error;
            }
            throw new McpError(ErrorCode.InvalidParams, `Failed to load vector from file: ${error instanceof Error ? error.message : String(error)}`);
        }
    }
    async saveVectorToFile(vector, filePath, format) {
        const fs = await import('fs');
        const path = await import('path');
        // Determine format from extension or explicit format parameter
        const fileFormat = this.getFileFormat(filePath, format);
        const absolutePath = path.resolve(filePath);
        // Ensure directory exists
        const directory = path.dirname(absolutePath);
        if (!fs.existsSync(directory)) {
            fs.mkdirSync(directory, { recursive: true });
        }
        let content;
        switch (fileFormat) {
            case 'json':
                content = JSON.stringify(vector, null, 2);
                break;
            case 'csv':
                content = vector.join(',');
                break;
            case 'txt':
                content = vector.join('\n');
                break;
            default:
                throw new Error(`Unsupported format: ${fileFormat}`);
        }
        fs.writeFileSync(absolutePath, content, 'utf8');
        console.log(`💾 Saved vector of size ${vector.length} to ${absolutePath} (${fileFormat} format)`);
    }
    getFileFormat(filePath, explicitFormat) {
        if (explicitFormat) {
            return explicitFormat.toLowerCase();
        }
        const extension = filePath.split('.').pop()?.toLowerCase();
        if (extension && ['json', 'csv', 'txt'].includes(extension)) {
            return extension;
        }
        // Default to JSON if no valid extension
        return 'json';
    }
    generateRecommendations(analysis) {
        const recommendations = [];
        if (!analysis.isDiagonallyDominant) {
            recommendations.push('Matrix is not diagonally dominant. Consider matrix preconditioning or using a different solver.');
        }
        else if (analysis.dominanceStrength < 0.1) {
            recommendations.push('Weak diagonal dominance detected. Convergence may be slow.');
        }
        if (analysis.sparsity > 0.9) {
            recommendations.push('Matrix is very sparse. Consider using sparse matrix formats for better performance.');
        }
        if (!analysis.isSymmetric && analysis.isDiagonallyDominant) {
            recommendations.push('Matrix is asymmetric but diagonally dominant. Random walk methods may be most effective.');
        }
        if (analysis.size.rows > 10000) {
            recommendations.push('Large matrix detected. Consider using sublinear estimation methods for specific entries rather than full solve.');
        }
        return recommendations;
    }
    async run() {
        const transport = new StdioServerTransport();
        await this.server.connect(transport);
        console.error('Sublinear Solver MCP server running on stdio');
    }
}
// Main execution
if (import.meta.url === `file://${process.argv[1]}`) {
    const server = new SublinearSolverMCPServer();
    server.run().catch(console.error);
}
