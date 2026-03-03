export declare class EmergenceTools {
    private emergenceSystem;
    constructor();
    getTools(): ({
        name: string;
        description: string;
        inputSchema: {
            type: string;
            properties: {
                input: {
                    description: string;
                };
                tools: {
                    type: string;
                    description: string;
                    items: {
                        type: string;
                    };
                };
                cursor: {
                    type: string;
                    description: string;
                };
                pageSize: {
                    type: string;
                    description: string;
                    minimum: number;
                    maximum: number;
                };
                count?: undefined;
                targetCapability?: undefined;
                component?: undefined;
                scenarios?: undefined;
                matrixOperations?: undefined;
                maxDepth?: undefined;
                wasmAcceleration?: undefined;
                emergenceMode?: undefined;
            };
            required: string[];
        };
    } | {
        name: string;
        description: string;
        inputSchema: {
            type: string;
            properties: {
                input: {
                    description: string;
                };
                count: {
                    type: string;
                    description: string;
                    minimum: number;
                    maximum: number;
                };
                tools: {
                    type: string;
                    description: string;
                    items: {
                        type: string;
                    };
                };
                cursor?: undefined;
                pageSize?: undefined;
                targetCapability?: undefined;
                component?: undefined;
                scenarios?: undefined;
                matrixOperations?: undefined;
                maxDepth?: undefined;
                wasmAcceleration?: undefined;
                emergenceMode?: undefined;
            };
            required: string[];
        };
    } | {
        name: string;
        description: string;
        inputSchema: {
            type: string;
            properties: {
                input?: undefined;
                tools?: undefined;
                cursor?: undefined;
                pageSize?: undefined;
                count?: undefined;
                targetCapability?: undefined;
                component?: undefined;
                scenarios?: undefined;
                matrixOperations?: undefined;
                maxDepth?: undefined;
                wasmAcceleration?: undefined;
                emergenceMode?: undefined;
            };
            required?: undefined;
        };
    } | {
        name: string;
        description: string;
        inputSchema: {
            type: string;
            properties: {
                targetCapability: {
                    type: string;
                    description: string;
                };
                input?: undefined;
                tools?: undefined;
                cursor?: undefined;
                pageSize?: undefined;
                count?: undefined;
                component?: undefined;
                scenarios?: undefined;
                matrixOperations?: undefined;
                maxDepth?: undefined;
                wasmAcceleration?: undefined;
                emergenceMode?: undefined;
            };
            required: string[];
        };
    } | {
        name: string;
        description: string;
        inputSchema: {
            type: string;
            properties: {
                component: {
                    type: string;
                    description: string;
                    enum: string[];
                };
                input?: undefined;
                tools?: undefined;
                cursor?: undefined;
                pageSize?: undefined;
                count?: undefined;
                targetCapability?: undefined;
                scenarios?: undefined;
                matrixOperations?: undefined;
                maxDepth?: undefined;
                wasmAcceleration?: undefined;
                emergenceMode?: undefined;
            };
            required?: undefined;
        };
    } | {
        name: string;
        description: string;
        inputSchema: {
            type: string;
            properties: {
                scenarios: {
                    type: string;
                    description: string;
                    items: {
                        type: string;
                        enum: string[];
                    };
                };
                input?: undefined;
                tools?: undefined;
                cursor?: undefined;
                pageSize?: undefined;
                count?: undefined;
                targetCapability?: undefined;
                component?: undefined;
                matrixOperations?: undefined;
                maxDepth?: undefined;
                wasmAcceleration?: undefined;
                emergenceMode?: undefined;
            };
            required: string[];
        };
    } | {
        name: string;
        description: string;
        inputSchema: {
            type: string;
            properties: {
                input: {
                    description: string;
                };
                matrixOperations: {
                    type: string;
                    description: string;
                    items: {
                        type: string;
                        enum: string[];
                    };
                };
                maxDepth: {
                    type: string;
                    description: string;
                    minimum: number;
                    maximum: number;
                    default: number;
                };
                wasmAcceleration: {
                    type: string;
                    description: string;
                    default: boolean;
                };
                emergenceMode: {
                    type: string;
                    description: string;
                    enum: string[];
                    default: string;
                };
                tools?: undefined;
                cursor?: undefined;
                pageSize?: undefined;
                count?: undefined;
                targetCapability?: undefined;
                component?: undefined;
                scenarios?: undefined;
            };
            required: string[];
        };
    })[];
    handleToolCall(name: string, args: any): Promise<any>;
    private processWithTimeout;
    /**
     * Process emergence with pagination support for large tool arrays
     */
    private processWithPagination;
    /**
     * Matrix-focused emergence with WASM acceleration and controlled recursion
     */
    private processMatrixEmergence;
    /**
     * Create controlled matrix tools environment with WASM acceleration
     */
    private createMatrixToolsEnvironment;
    /**
     * Run matrix emergence with controlled mathematical recursion
     */
    private runMatrixEmergence;
    /**
     * Explore numerical emergence patterns with WASM-accelerated computations
     */
    private exploreNumericalEmergence;
    /**
     * Execute controlled mathematical operation with WASM acceleration
     */
    private executeControlledMathOperation;
    private generateMockSolutionVector;
    private generateMockRankVector;
    private calculateOperationEmergence;
    private extractEmergentProperties;
    private synthesizeMultiLevelEmergence;
    private calculateMatrixEmergenceLevel;
    private assessMathComplexity;
    private identifyMatrixPatterns;
    private exploreAlgebraicEmergence;
    private exploreTemporalEmergence;
    private exploreGraphEmergence;
    /**
     * Fixed version of runTestScenarios that doesn't hang
     */
    private runTestScenariosFixed;
    /**
     * Fixed version that doesn't call processWithEmergence for problematic scenarios
     */
    private runSingleTestScenarioFixed;
    private testSelfModificationFixed;
    private testPersistentLearningFixed;
    private testStochasticExplorationFixed;
    private testCrossToolSharingFixed;
    private testFeedbackLoopsFixed;
    private testEmergentCapabilitiesFixed;
}
