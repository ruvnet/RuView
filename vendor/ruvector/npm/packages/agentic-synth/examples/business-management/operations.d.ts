/**
 * Business Operations Management Data Generation
 * Simulates project management, vendor management, contract lifecycle, and approval workflows
 */
/**
 * Generate Project Management Data
 */
export declare function generateProjects(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Resource Allocation Data
 */
export declare function generateResourceAllocations(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Vendor Management Data
 */
export declare function generateVendors(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Contract Lifecycle Data
 */
export declare function generateContracts(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Approval Workflow Data
 */
export declare function generateApprovalWorkflows(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Audit Trail Data (time-series)
 */
export declare function generateAuditTrail(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate complete operations dataset in parallel
 */
export declare function generateCompleteOperationsDataset(): Promise<{
    projects: unknown[];
    resourceAllocations: unknown[];
    vendors: unknown[];
    contracts: unknown[];
    approvalWorkflows: unknown[];
    auditTrail: unknown[];
    metadata: {
        totalRecords: number;
        generatedAt: string;
    };
}>;
/**
 * Simulate end-to-end procurement workflow
 */
export declare function simulateProcurementWorkflow(): Promise<{
    vendors: unknown[];
    contracts: unknown[];
    approvals: unknown[];
    auditTrail: unknown[];
    summary: {
        vendorsOnboarded: number;
        contractsCreated: number;
        approvalsProcessed: number;
        auditEvents: number;
    };
}>;
declare const _default: {
    generateProjects: typeof generateProjects;
    generateResourceAllocations: typeof generateResourceAllocations;
    generateVendors: typeof generateVendors;
    generateContracts: typeof generateContracts;
    generateApprovalWorkflows: typeof generateApprovalWorkflows;
    generateAuditTrail: typeof generateAuditTrail;
    generateCompleteOperationsDataset: typeof generateCompleteOperationsDataset;
    simulateProcurementWorkflow: typeof simulateProcurementWorkflow;
};
export default _default;
//# sourceMappingURL=operations.d.ts.map