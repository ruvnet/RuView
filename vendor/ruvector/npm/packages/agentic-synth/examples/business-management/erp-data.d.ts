/**
 * Enterprise Resource Planning (ERP) Data Generation
 * Simulates SAP, Oracle ERP, and Microsoft Dynamics integration scenarios
 */
/**
 * Generate SAP Material Management data
 */
export declare function generateMaterialData(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate SAP Purchase Orders
 */
export declare function generatePurchaseOrders(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Oracle Supply Chain Events (time-series)
 */
export declare function generateSupplyChainEvents(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Microsoft Dynamics 365 Manufacturing Orders
 */
export declare function generateManufacturingOrders(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate multi-location warehouse inventory snapshots
 */
export declare function generateWarehouseInventory(warehouseCount?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate SAP Financial Transactions (FI/CO)
 */
export declare function generateFinancialTransactions(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate complete ERP dataset in parallel
 */
export declare function generateCompleteERPDataset(): Promise<{
    materials: unknown[];
    purchaseOrders: unknown[];
    supplyChainEvents: unknown[];
    manufacturingOrders: unknown[];
    warehouseInventory: unknown[];
    financialTransactions: unknown[];
    metadata: {
        totalRecords: number;
        generatedAt: string;
    };
}>;
/**
 * Stream ERP data generation for large datasets
 */
export declare function streamERPData(type: 'material' | 'po' | 'transaction', count?: number): Promise<void>;
declare const _default: {
    generateMaterialData: typeof generateMaterialData;
    generatePurchaseOrders: typeof generatePurchaseOrders;
    generateSupplyChainEvents: typeof generateSupplyChainEvents;
    generateManufacturingOrders: typeof generateManufacturingOrders;
    generateWarehouseInventory: typeof generateWarehouseInventory;
    generateFinancialTransactions: typeof generateFinancialTransactions;
    generateCompleteERPDataset: typeof generateCompleteERPDataset;
    streamERPData: typeof streamERPData;
};
export default _default;
//# sourceMappingURL=erp-data.d.ts.map