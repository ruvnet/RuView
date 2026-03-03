#!/usr/bin/env node
"use strict";
/**
 * Strange Loops MCP Server
 * Provides nano-agent, quantum-classical hybrid computing, and temporal prediction tools
 */
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
var __generator = (this && this.__generator) || function (thisArg, body) {
    var _ = { label: 0, sent: function() { if (t[0] & 1) throw t[1]; return t[1]; }, trys: [], ops: [] }, f, y, t, g = Object.create((typeof Iterator === "function" ? Iterator : Object).prototype);
    return g.next = verb(0), g["throw"] = verb(1), g["return"] = verb(2), typeof Symbol === "function" && (g[Symbol.iterator] = function() { return this; }), g;
    function verb(n) { return function (v) { return step([n, v]); }; }
    function step(op) {
        if (f) throw new TypeError("Generator is already executing.");
        while (g && (g = 0, op[0] && (_ = 0)), _) try {
            if (f = 1, y && (t = op[0] & 2 ? y["return"] : op[0] ? y["throw"] || ((t = y["return"]) && t.call(y), 0) : y.next) && !(t = t.call(y, op[1])).done) return t;
            if (y = 0, t) op = [op[0] & 2, t.value];
            switch (op[0]) {
                case 0: case 1: t = op; break;
                case 4: _.label++; return { value: op[1], done: false };
                case 5: _.label++; y = op[1]; op = [0]; continue;
                case 7: op = _.ops.pop(); _.trys.pop(); continue;
                default:
                    if (!(t = _.trys, t = t.length > 0 && t[t.length - 1]) && (op[0] === 6 || op[0] === 2)) { _ = 0; continue; }
                    if (op[0] === 3 && (!t || (op[1] > t[0] && op[1] < t[3]))) { _.label = op[1]; break; }
                    if (op[0] === 6 && _.label < t[1]) { _.label = t[1]; t = op; break; }
                    if (t && _.label < t[2]) { _.label = t[2]; _.ops.push(op); break; }
                    if (t[2]) _.ops.pop();
                    _.trys.pop(); continue;
            }
            op = body.call(thisArg, _);
        } catch (e) { op = [6, e]; y = 0; } finally { f = t = 0; }
        if (op[0] & 5) throw op[1]; return { value: op[0] ? op[1] : void 0, done: true };
    }
};
Object.defineProperty(exports, "__esModule", { value: true });
var index_js_1 = require("@modelcontextprotocol/sdk/server/index.js");
var stdio_js_1 = require("@modelcontextprotocol/sdk/server/stdio.js");
var types_js_1 = require("@modelcontextprotocol/sdk/types.js");
// Import our Strange Loop library
var StrangeLoop = require('../lib/strange-loop.js');
var StrangeLoopsMCPServer = /** @class */ (function () {
    function StrangeLoopsMCPServer() {
        this.isInitialized = false;
        this.server = new index_js_1.Server({
            name: 'strange-loops',
            version: '0.1.0',
        }, {
            capabilities: {
                tools: {},
            },
        });
        this.setupHandlers();
    }
    StrangeLoopsMCPServer.prototype.setupHandlers = function () {
        var _this = this;
        // List available tools
        this.server.setRequestHandler(types_js_1.ListToolsRequestSchema, function () { return __awaiter(_this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                return [2 /*return*/, {
                        tools: [
                            {
                                name: 'nano_swarm_create',
                                description: 'Create a nano-agent swarm with specified configuration',
                                inputSchema: {
                                    type: 'object',
                                    properties: {
                                        agentCount: {
                                            type: 'number',
                                            description: 'Number of agents in the swarm',
                                            default: 1000,
                                            minimum: 1,
                                            maximum: 100000
                                        },
                                        topology: {
                                            type: 'string',
                                            description: 'Swarm topology',
                                            enum: ['mesh', 'hierarchical', 'ring', 'star'],
                                            default: 'mesh'
                                        },
                                        tickDurationNs: {
                                            type: 'number',
                                            description: 'Tick duration in nanoseconds',
                                            default: 25000
                                        }
                                    }
                                }
                            },
                            {
                                name: 'nano_swarm_run',
                                description: 'Run nano-agent swarm simulation for specified duration',
                                inputSchema: {
                                    type: 'object',
                                    properties: {
                                        durationMs: {
                                            type: 'number',
                                            description: 'Simulation duration in milliseconds',
                                            default: 5000,
                                            minimum: 100
                                        }
                                    },
                                    required: ['durationMs']
                                }
                            },
                            {
                                name: 'quantum_container_create',
                                description: 'Create a quantum container for quantum-classical hybrid computing',
                                inputSchema: {
                                    type: 'object',
                                    properties: {
                                        qubits: {
                                            type: 'number',
                                            description: 'Number of qubits',
                                            default: 3,
                                            minimum: 1,
                                            maximum: 16
                                        }
                                    }
                                }
                            },
                            {
                                name: 'quantum_superposition',
                                description: 'Create quantum superposition across all states',
                                inputSchema: {
                                    type: 'object',
                                    properties: {
                                        qubits: {
                                            type: 'number',
                                            description: 'Number of qubits for superposition',
                                            default: 3
                                        }
                                    }
                                }
                            },
                            {
                                name: 'quantum_measure',
                                description: 'Measure quantum state (collapses superposition)',
                                inputSchema: {
                                    type: 'object',
                                    properties: {
                                        qubits: {
                                            type: 'number',
                                            description: 'Number of qubits in system',
                                            default: 3
                                        }
                                    }
                                }
                            },
                            {
                                name: 'temporal_predictor_create',
                                description: 'Create temporal predictor for future state prediction',
                                inputSchema: {
                                    type: 'object',
                                    properties: {
                                        horizonNs: {
                                            type: 'number',
                                            description: 'Prediction horizon in nanoseconds',
                                            default: 10000000
                                        },
                                        historySize: {
                                            type: 'number',
                                            description: 'History buffer size',
                                            default: 500
                                        }
                                    }
                                }
                            },
                            {
                                name: 'temporal_predict',
                                description: 'Predict future values based on current input',
                                inputSchema: {
                                    type: 'object',
                                    properties: {
                                        currentValues: {
                                            type: 'array',
                                            items: { type: 'number' },
                                            description: 'Current input values for prediction'
                                        },
                                        horizonNs: {
                                            type: 'number',
                                            description: 'Prediction horizon',
                                            default: 10000000
                                        }
                                    },
                                    required: ['currentValues']
                                }
                            },
                            {
                                name: 'consciousness_evolve',
                                description: 'Evolve temporal consciousness one step',
                                inputSchema: {
                                    type: 'object',
                                    properties: {
                                        maxIterations: {
                                            type: 'number',
                                            description: 'Maximum evolution iterations',
                                            default: 1000
                                        },
                                        enableQuantum: {
                                            type: 'boolean',
                                            description: 'Enable quantum integration',
                                            default: true
                                        }
                                    }
                                }
                            },
                            {
                                name: 'system_info',
                                description: 'Get Strange Loops system information and capabilities',
                                inputSchema: {
                                    type: 'object',
                                    properties: {}
                                }
                            },
                            {
                                name: 'benchmark_run',
                                description: 'Run comprehensive performance benchmark',
                                inputSchema: {
                                    type: 'object',
                                    properties: {
                                        agentCount: {
                                            type: 'number',
                                            description: 'Number of agents for benchmark',
                                            default: 1000
                                        },
                                        durationMs: {
                                            type: 'number',
                                            description: 'Benchmark duration in milliseconds',
                                            default: 5000
                                        }
                                    }
                                }
                            }
                        ]
                    }];
            });
        }); });
        // Handle tool calls
        this.server.setRequestHandler(types_js_1.CallToolRequestSchema, function (request) { return __awaiter(_this, void 0, void 0, function () {
            var _a, name, args, _b, swarm, swarm, results, quantum, quantum, quantum, measurement, predictor, predictor, currentValues, prediction, consciousness, state, info, results, error_1;
            return __generator(this, function (_c) {
                switch (_c.label) {
                    case 0:
                        _a = request.params, name = _a.name, args = _a.arguments;
                        _c.label = 1;
                    case 1:
                        _c.trys.push([1, 32, , 33]);
                        if (!!this.isInitialized) return [3 /*break*/, 3];
                        return [4 /*yield*/, StrangeLoop.init()];
                    case 2:
                        _c.sent();
                        this.isInitialized = true;
                        _c.label = 3;
                    case 3:
                        _b = name;
                        switch (_b) {
                            case 'nano_swarm_create': return [3 /*break*/, 4];
                            case 'nano_swarm_run': return [3 /*break*/, 6];
                            case 'quantum_container_create': return [3 /*break*/, 9];
                            case 'quantum_superposition': return [3 /*break*/, 11];
                            case 'quantum_measure': return [3 /*break*/, 14];
                            case 'temporal_predictor_create': return [3 /*break*/, 18];
                            case 'temporal_predict': return [3 /*break*/, 20];
                            case 'consciousness_evolve': return [3 /*break*/, 23];
                            case 'system_info': return [3 /*break*/, 26];
                            case 'benchmark_run': return [3 /*break*/, 28];
                        }
                        return [3 /*break*/, 30];
                    case 4: return [4 /*yield*/, StrangeLoop.createSwarm({
                            agentCount: (args === null || args === void 0 ? void 0 : args.agentCount) || 1000,
                            topology: (args === null || args === void 0 ? void 0 : args.topology) || 'mesh',
                            tickDurationNs: (args === null || args === void 0 ? void 0 : args.tickDurationNs) || 25000
                        })];
                    case 5:
                        swarm = _c.sent();
                        return [2 /*return*/, {
                                content: [
                                    {
                                        type: 'text',
                                        text: JSON.stringify({
                                            success: true,
                                            swarm: {
                                                agentCount: swarm.config.agentCount,
                                                topology: swarm.config.topology,
                                                tickDurationNs: swarm.config.tickDurationNs,
                                                agents: swarm.agents.length
                                            },
                                            message: "Created nano-agent swarm with ".concat(swarm.config.agentCount, " agents")
                                        }, null, 2)
                                    }
                                ]
                            }];
                    case 6: return [4 /*yield*/, StrangeLoop.createSwarm({
                            agentCount: 1000,
                            topology: 'mesh'
                        })];
                    case 7:
                        swarm = _c.sent();
                        return [4 /*yield*/, swarm.run((args === null || args === void 0 ? void 0 : args.durationMs) || 5000)];
                    case 8:
                        results = _c.sent();
                        return [2 /*return*/, {
                                content: [
                                    {
                                        type: 'text',
                                        text: JSON.stringify({
                                            success: true,
                                            results: {
                                                totalTicks: results.totalTicks,
                                                agentCount: results.agentCount,
                                                runtimeNs: results.runtimeNs,
                                                ticksPerSecond: Math.round(results.ticksPerSecond),
                                                budgetViolations: results.budgetViolations,
                                                avgCyclesPerTick: Math.round(results.avgCyclesPerTick)
                                            },
                                            message: "Executed ".concat(results.totalTicks, " ticks at ").concat(Math.round(results.ticksPerSecond), " ticks/sec")
                                        }, null, 2)
                                    }
                                ]
                            }];
                    case 9: return [4 /*yield*/, StrangeLoop.createQuantumContainer((args === null || args === void 0 ? void 0 : args.qubits) || 3)];
                    case 10:
                        quantum = _c.sent();
                        return [2 /*return*/, {
                                content: [
                                    {
                                        type: 'text',
                                        text: JSON.stringify({
                                            success: true,
                                            quantum: {
                                                qubits: quantum.qubits,
                                                states: quantum.states,
                                                isInSuperposition: quantum.isInSuperposition
                                            },
                                            message: "Created quantum container with ".concat(quantum.qubits, " qubits (").concat(quantum.states, " states)")
                                        }, null, 2)
                                    }
                                ]
                            }];
                    case 11: return [4 /*yield*/, StrangeLoop.createQuantumContainer((args === null || args === void 0 ? void 0 : args.qubits) || 3)];
                    case 12:
                        quantum = _c.sent();
                        return [4 /*yield*/, quantum.createSuperposition()];
                    case 13:
                        _c.sent();
                        return [2 /*return*/, {
                                content: [
                                    {
                                        type: 'text',
                                        text: JSON.stringify({
                                            success: true,
                                            quantum: {
                                                qubits: quantum.qubits,
                                                states: quantum.states,
                                                isInSuperposition: quantum.isInSuperposition
                                            },
                                            message: "Created superposition across ".concat(quantum.states, " quantum states")
                                        }, null, 2)
                                    }
                                ]
                            }];
                    case 14: return [4 /*yield*/, StrangeLoop.createQuantumContainer((args === null || args === void 0 ? void 0 : args.qubits) || 3)];
                    case 15:
                        quantum = _c.sent();
                        return [4 /*yield*/, quantum.createSuperposition()];
                    case 16:
                        _c.sent();
                        return [4 /*yield*/, quantum.measure()];
                    case 17:
                        measurement = _c.sent();
                        return [2 /*return*/, {
                                content: [
                                    {
                                        type: 'text',
                                        text: JSON.stringify({
                                            success: true,
                                            measurement: {
                                                result: measurement,
                                                qubits: quantum.qubits,
                                                collapsedState: measurement,
                                                isInSuperposition: quantum.isInSuperposition
                                            },
                                            message: "Quantum measurement collapsed to state ".concat(measurement)
                                        }, null, 2)
                                    }
                                ]
                            }];
                    case 18: return [4 /*yield*/, StrangeLoop.createTemporalPredictor({
                            horizonNs: (args === null || args === void 0 ? void 0 : args.horizonNs) || 10000000,
                            historySize: (args === null || args === void 0 ? void 0 : args.historySize) || 500
                        })];
                    case 19:
                        predictor = _c.sent();
                        return [2 /*return*/, {
                                content: [
                                    {
                                        type: 'text',
                                        text: JSON.stringify({
                                            success: true,
                                            predictor: {
                                                horizonNs: predictor.horizonNs,
                                                historySize: predictor.historySize,
                                                currentHistory: predictor.history.length
                                            },
                                            message: "Created temporal predictor with ".concat(predictor.horizonNs, "ns horizon")
                                        }, null, 2)
                                    }
                                ]
                            }];
                    case 20: return [4 /*yield*/, StrangeLoop.createTemporalPredictor({
                            horizonNs: (args === null || args === void 0 ? void 0 : args.horizonNs) || 10000000,
                            historySize: 100
                        })];
                    case 21:
                        predictor = _c.sent();
                        currentValues = (args === null || args === void 0 ? void 0 : args.currentValues) || [1.0, 2.0, 3.0];
                        return [4 /*yield*/, predictor.predict(currentValues)];
                    case 22:
                        prediction = _c.sent();
                        return [2 /*return*/, {
                                content: [
                                    {
                                        type: 'text',
                                        text: JSON.stringify({
                                            success: true,
                                            prediction: {
                                                input: currentValues,
                                                predicted: prediction,
                                                horizonNs: predictor.horizonNs
                                            },
                                            message: "Predicted future values with ".concat(predictor.horizonNs / 1000000, "ms temporal lead")
                                        }, null, 2)
                                    }
                                ]
                            }];
                    case 23: return [4 /*yield*/, StrangeLoop.createTemporalConsciousness({
                            maxIterations: (args === null || args === void 0 ? void 0 : args.maxIterations) || 1000,
                            enableQuantum: (args === null || args === void 0 ? void 0 : args.enableQuantum) !== false
                        })];
                    case 24:
                        consciousness = _c.sent();
                        return [4 /*yield*/, consciousness.evolveStep()];
                    case 25:
                        state = _c.sent();
                        return [2 /*return*/, {
                                content: [
                                    {
                                        type: 'text',
                                        text: JSON.stringify({
                                            success: true,
                                            consciousness: {
                                                iteration: state.iteration,
                                                consciousnessIndex: state.consciousnessIndex,
                                                temporalPatterns: state.temporalPatterns,
                                                quantumInfluence: state.quantumInfluence
                                            },
                                            message: "Consciousness evolved to iteration ".concat(state.iteration, " with index ").concat(state.consciousnessIndex.toFixed(3))
                                        }, null, 2)
                                    }
                                ]
                            }];
                    case 26: return [4 /*yield*/, StrangeLoop.getSystemInfo()];
                    case 27:
                        info = _c.sent();
                        return [2 /*return*/, {
                                content: [
                                    {
                                        type: 'text',
                                        text: JSON.stringify({
                                            success: true,
                                            system: info,
                                            message: 'Strange Loops system information retrieved'
                                        }, null, 2)
                                    }
                                ]
                            }];
                    case 28: return [4 /*yield*/, StrangeLoop.runBenchmark({
                            agentCount: (args === null || args === void 0 ? void 0 : args.agentCount) || 1000,
                            duration: (args === null || args === void 0 ? void 0 : args.durationMs) || 5000
                        })];
                    case 29:
                        results = _c.sent();
                        return [2 /*return*/, {
                                content: [
                                    {
                                        type: 'text',
                                        text: JSON.stringify({
                                            success: true,
                                            benchmark: {
                                                totalTicks: results.totalTicks,
                                                agentCount: results.agentCount,
                                                runtimeNs: results.runtimeNs,
                                                ticksPerSecond: Math.round(results.ticksPerSecond),
                                                budgetViolations: results.budgetViolations,
                                                performanceRating: results.ticksPerSecond > 500000 ? 'Excellent' :
                                                    results.ticksPerSecond > 250000 ? 'Good' : 'Fair'
                                            },
                                            message: "Benchmark completed: ".concat(Math.round(results.ticksPerSecond), " ticks/sec")
                                        }, null, 2)
                                    }
                                ]
                            }];
                    case 30: return [2 /*return*/, {
                            content: [
                                {
                                    type: 'text',
                                    text: JSON.stringify({
                                        success: false,
                                        error: "Unknown tool: ".concat(name),
                                        availableTools: [
                                            'nano_swarm_create', 'nano_swarm_run', 'quantum_container_create',
                                            'quantum_superposition', 'quantum_measure', 'temporal_predictor_create',
                                            'temporal_predict', 'consciousness_evolve', 'system_info', 'benchmark_run'
                                        ]
                                    }, null, 2)
                                }
                            ]
                        }];
                    case 31: return [3 /*break*/, 33];
                    case 32:
                        error_1 = _c.sent();
                        return [2 /*return*/, {
                                content: [
                                    {
                                        type: 'text',
                                        text: JSON.stringify({
                                            success: false,
                                            error: error_1 instanceof Error ? error_1.message : 'Unknown error',
                                            tool: name,
                                            arguments: args
                                        }, null, 2)
                                    }
                                ]
                            }];
                    case 33: return [2 /*return*/];
                }
            });
        }); });
    };
    StrangeLoopsMCPServer.prototype.start = function () {
        return __awaiter(this, void 0, void 0, function () {
            var transport;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        transport = new stdio_js_1.StdioServerTransport();
                        return [4 /*yield*/, this.server.connect(transport)];
                    case 1:
                        _a.sent();
                        console.error('Strange Loops MCP Server started');
                        return [2 /*return*/];
                }
            });
        });
    };
    return StrangeLoopsMCPServer;
}());
// Start the server
var server = new StrangeLoopsMCPServer();
server.start().catch(function (error) {
    console.error('Failed to start Strange Loops MCP Server:', error);
    process.exit(1);
});
