/**
 * Customer Relationship Management (CRM) Data Generation
 * Simulates Salesforce, Microsoft Dynamics CRM, and HubSpot scenarios
 */
/**
 * Generate Salesforce Leads
 */
export declare function generateLeads(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Sales Pipeline (Opportunities)
 */
export declare function generateOpportunities(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate HubSpot Contact Interactions (time-series)
 */
export declare function generateContactInteractions(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Microsoft Dynamics 365 Accounts
 */
export declare function generateAccounts(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Salesforce Service Cloud Support Tickets
 */
export declare function generateSupportTickets(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Customer Lifetime Value Analysis
 */
export declare function generateCustomerLTV(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Simulate complete sales funnel with conversion metrics
 */
export declare function simulateSalesFunnel(): Promise<{
    leads: unknown[];
    opportunities: unknown[];
    accounts: unknown[];
    metrics: {
        leads: number;
        qualifiedLeads: number;
        opportunities: number;
        wonDeals: number;
        accounts: number;
        conversionRates: {
            leadToQualified: string;
            qualifiedToOpportunity: string;
            opportunityToWon: string;
            leadToCustomer: string;
        };
        totalPipelineValue: number;
        averageDealSize: number;
    };
}>;
/**
 * Generate complete CRM dataset in parallel
 */
export declare function generateCompleteCRMDataset(): Promise<{
    leads: unknown[];
    opportunities: unknown[];
    interactions: unknown[];
    accounts: unknown[];
    supportTickets: unknown[];
    customerLTV: unknown[];
    metadata: {
        totalRecords: number;
        generatedAt: string;
    };
}>;
/**
 * Stream CRM interactions for real-time analysis
 */
export declare function streamCRMInteractions(duration?: number): Promise<void>;
declare const _default: {
    generateLeads: typeof generateLeads;
    generateOpportunities: typeof generateOpportunities;
    generateContactInteractions: typeof generateContactInteractions;
    generateAccounts: typeof generateAccounts;
    generateSupportTickets: typeof generateSupportTickets;
    generateCustomerLTV: typeof generateCustomerLTV;
    simulateSalesFunnel: typeof simulateSalesFunnel;
    generateCompleteCRMDataset: typeof generateCompleteCRMDataset;
    streamCRMInteractions: typeof streamCRMInteractions;
};
export default _default;
//# sourceMappingURL=crm-simulation.d.ts.map