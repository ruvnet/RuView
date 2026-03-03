/**
 * MCP Server for Sublinear-Time Solver
 * Provides MCP interface to the core solver algorithms
 */
export declare class SublinearSolverMCPServer {
    private server;
    private solvers;
    private temporalTools;
    private psychoSymbolicTools;
    private dynamicPsychoSymbolicTools;
    private domainManagementTools;
    private domainValidationTools;
    private consciousnessTools;
    private emergenceTools;
    private schedulerTools;
    private wasmSolver;
    private trueSublinearSolver;
    constructor();
    private setupToolHandlers;
    private setupErrorHandling;
    private handleSolve;
    private handleEstimateEntry;
    private handleAnalyzeMatrix;
    private handlePageRank;
    private handleSolveTrueSublinear;
    private handleAnalyzeTrueSublinearMatrix;
    private handleGenerateTestVector;
    private handleSaveVectorToFile;
    private loadVectorFromFile;
    private saveVectorToFile;
    private getFileFormat;
    private generateRecommendations;
    run(): Promise<void>;
}
