/**
 * MCP Tools for Temporal Lead Solver
 * Provides temporal computational lead calculations through MCP
 */
import { Tool } from '@modelcontextprotocol/sdk/types.js';
export declare class TemporalTools {
    private predictor;
    constructor();
    /**
     * Get all temporal lead tools
     */
    getTools(): Tool[];
    /**
     * Handle tool calls
     */
    handleToolCall(name: string, args: any): Promise<any>;
    /**
     * Predict with temporal advantage
     */
    private predictWithTemporalAdvantage;
    /**
     * Validate temporal advantage
     */
    private validateTemporalAdvantage;
    /**
     * Calculate light travel comparison
     */
    private calculateLightTravel;
    /**
     * Demonstrate temporal lead scenarios
     */
    private demonstrateTemporalLead;
    /**
     * Convert matrix to dense format
     */
    private convertToDenseMatrix;
    /**
     * Interpret validation results
     */
    private interpretResults;
    /**
     * Get practical application description
     */
    private getPracticalApplication;
}
