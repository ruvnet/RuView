/**
 * Enterprise Resource Planning (ERP) Data Generation
 * Simulates SAP, Oracle ERP, and Microsoft Dynamics integration scenarios
 */

import { createSynth } from '../../src/index.js';

// SAP S/4HANA Material Management Schema
const materialSchema = {
  materialNumber: { type: 'string', required: true },
  description: { type: 'string', required: true },
  materialType: { type: 'string', required: true },
  baseUnitOfMeasure: { type: 'string', required: true },
  materialGroup: { type: 'string', required: true },
  grossWeight: { type: 'number', required: true },
  netWeight: { type: 'number', required: true },
  weightUnit: { type: 'string', required: true },
  division: { type: 'string', required: false },
  plant: { type: 'string', required: true },
  storageLocation: { type: 'string', required: true },
  stockQuantity: { type: 'number', required: true },
  reservedQuantity: { type: 'number', required: true },
  availableQuantity: { type: 'number', required: true },
  valuationClass: { type: 'string', required: true },
  priceControl: { type: 'string', required: true },
  standardPrice: { type: 'number', required: true },
  movingAveragePrice: { type: 'number', required: true },
  priceUnit: { type: 'number', required: true },
  currency: { type: 'string', required: true }
};

// SAP Purchase Order Schema
const purchaseOrderSchema = {
  poNumber: { type: 'string', required: true },
  poDate: { type: 'string', required: true },
  vendor: { type: 'object', required: true, properties: {
    vendorId: { type: 'string' },
    vendorName: { type: 'string' },
    country: { type: 'string' },
    paymentTerms: { type: 'string' }
  }},
  companyCode: { type: 'string', required: true },
  purchasingOrg: { type: 'string', required: true },
  purchasingGroup: { type: 'string', required: true },
  documentType: { type: 'string', required: true },
  currency: { type: 'string', required: true },
  exchangeRate: { type: 'number', required: true },
  items: { type: 'array', required: true, items: {
    itemNumber: { type: 'string' },
    materialNumber: { type: 'string' },
    shortText: { type: 'string' },
    quantity: { type: 'number' },
    unit: { type: 'string' },
    netPrice: { type: 'number' },
    priceUnit: { type: 'number' },
    netValue: { type: 'number' },
    taxCode: { type: 'string' },
    plant: { type: 'string' },
    storageLocation: { type: 'string' },
    deliveryDate: { type: 'string' },
    accountAssignment: { type: 'string' },
    costCenter: { type: 'string' },
    glAccount: { type: 'string' }
  }},
  totalAmount: { type: 'number', required: true },
  taxAmount: { type: 'number', required: true },
  status: { type: 'string', required: true },
  createdBy: { type: 'string', required: true },
  changedBy: { type: 'string', required: false }
};

// Oracle ERP Supply Chain Event Schema
const supplyChainEventSchema = {
  eventId: { type: 'string', required: true },
  eventType: { type: 'string', required: true },
  timestamp: { type: 'string', required: true },
  organizationId: { type: 'string', required: true },
  location: { type: 'object', required: true, properties: {
    locationId: { type: 'string' },
    locationName: { type: 'string' },
    locationType: { type: 'string' },
    address: { type: 'string' },
    city: { type: 'string' },
    state: { type: 'string' },
    country: { type: 'string' },
    postalCode: { type: 'string' }
  }},
  shipment: { type: 'object', required: false, properties: {
    shipmentNumber: { type: 'string' },
    carrier: { type: 'string' },
    trackingNumber: { type: 'string' },
    expectedDelivery: { type: 'string' },
    actualDelivery: { type: 'string' },
    status: { type: 'string' }
  }},
  inventory: { type: 'object', required: false, properties: {
    itemId: { type: 'string' },
    itemDescription: { type: 'string' },
    quantity: { type: 'number' },
    uom: { type: 'string' },
    lotNumber: { type: 'string' },
    serialNumbers: { type: 'array' }
  }},
  impact: { type: 'string', required: true },
  severity: { type: 'string', required: true },
  resolution: { type: 'string', required: false }
};

// Microsoft Dynamics 365 Manufacturing Process Schema
const manufacturingProcessSchema = {
  productionOrderId: { type: 'string', required: true },
  orderType: { type: 'string', required: true },
  status: { type: 'string', required: true },
  priority: { type: 'number', required: true },
  plannedStartDate: { type: 'string', required: true },
  plannedEndDate: { type: 'string', required: true },
  actualStartDate: { type: 'string', required: false },
  actualEndDate: { type: 'string', required: false },
  product: { type: 'object', required: true, properties: {
    itemNumber: { type: 'string' },
    productName: { type: 'string' },
    configurationId: { type: 'string' },
    bom: { type: 'string' },
    routingNumber: { type: 'string' }
  }},
  quantity: { type: 'object', required: true, properties: {
    ordered: { type: 'number' },
    started: { type: 'number' },
    completed: { type: 'number' },
    scrapped: { type: 'number' },
    remaining: { type: 'number' },
    unit: { type: 'string' }
  }},
  warehouse: { type: 'string', required: true },
  site: { type: 'string', required: true },
  resourceGroup: { type: 'string', required: true },
  costingLotSize: { type: 'number', required: true },
  operations: { type: 'array', required: true, items: {
    operationNumber: { type: 'string' },
    operationName: { type: 'string' },
    workCenter: { type: 'string' },
    setupTime: { type: 'number' },
    processTime: { type: 'number' },
    queueTime: { type: 'number' },
    laborCost: { type: 'number' },
    machineCost: { type: 'number' },
    status: { type: 'string' }
  }},
  materials: { type: 'array', required: true, items: {
    lineNumber: { type: 'string' },
    itemNumber: { type: 'string' },
    itemName: { type: 'string' },
    quantity: { type: 'number' },
    consumed: { type: 'number' },
    unit: { type: 'string' },
    warehouse: { type: 'string' },
    batchNumber: { type: 'string' }
  }}
};

// Multi-location Warehouse Management Schema
const warehouseInventorySchema = {
  inventoryId: { type: 'string', required: true },
  timestamp: { type: 'string', required: true },
  warehouse: { type: 'object', required: true, properties: {
    warehouseId: { type: 'string' },
    warehouseName: { type: 'string' },
    type: { type: 'string' },
    capacity: { type: 'number' },
    utilization: { type: 'number' },
    address: { type: 'object', properties: {
      street: { type: 'string' },
      city: { type: 'string' },
      state: { type: 'string' },
      country: { type: 'string' },
      postalCode: { type: 'string' }
    }}
  }},
  zones: { type: 'array', required: true, items: {
    zoneId: { type: 'string' },
    zoneName: { type: 'string' },
    zoneType: { type: 'string' },
    temperature: { type: 'number' },
    humidity: { type: 'number' },
    items: { type: 'array', items: {
      sku: { type: 'string' },
      description: { type: 'string' },
      quantity: { type: 'number' },
      unit: { type: 'string' },
      location: { type: 'string' },
      lotNumber: { type: 'string' },
      expiryDate: { type: 'string' },
      value: { type: 'number' }
    }}
  }},
  movements: { type: 'array', required: true, items: {
    movementId: { type: 'string' },
    timestamp: { type: 'string' },
    type: { type: 'string' },
    fromLocation: { type: 'string' },
    toLocation: { type: 'string' },
    sku: { type: 'string' },
    quantity: { type: 'number' },
    operator: { type: 'string' },
    reason: { type: 'string' }
  }},
  metrics: { type: 'object', required: true, properties: {
    totalItems: { type: 'number' },
    totalValue: { type: 'number' },
    turnoverRate: { type: 'number' },
    fillRate: { type: 'number' },
    accuracyRate: { type: 'number' }
  }}
};

// Financial Transaction Schema (SAP FI/CO)
const financialTransactionSchema = {
  documentNumber: { type: 'string', required: true },
  fiscalYear: { type: 'string', required: true },
  companyCode: { type: 'string', required: true },
  documentType: { type: 'string', required: true },
  documentDate: { type: 'string', required: true },
  postingDate: { type: 'string', required: true },
  period: { type: 'number', required: true },
  currency: { type: 'string', required: true },
  exchangeRate: { type: 'number', required: true },
  reference: { type: 'string', required: false },
  headerText: { type: 'string', required: false },
  lineItems: { type: 'array', required: true, items: {
    lineNumber: { type: 'string' },
    glAccount: { type: 'string' },
    accountDescription: { type: 'string' },
    debitCredit: { type: 'string' },
    amount: { type: 'number' },
    taxCode: { type: 'string' },
    taxAmount: { type: 'number' },
    costCenter: { type: 'string' },
    profitCenter: { type: 'string' },
    segment: { type: 'string' },
    assignment: { type: 'string' },
    text: { type: 'string' },
    businessArea: { type: 'string' }
  }},
  totalDebit: { type: 'number', required: true },
  totalCredit: { type: 'number', required: true },
  status: { type: 'string', required: true },
  parkedBy: { type: 'string', required: false },
  postedBy: { type: 'string', required: false },
  reversalDocument: { type: 'string', required: false }
};

/**
 * Generate SAP Material Management data
 */
export async function generateMaterialData(count: number = 100) {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  console.log(`Generating ${count} SAP material master records...`);

  const result = await synth.generateStructured({
    count,
    schema: materialSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} materials in ${result.metadata.duration}ms`);
  console.log('Sample material:', result.data[0]);

  return result;
}

/**
 * Generate SAP Purchase Orders
 */
export async function generatePurchaseOrders(count: number = 50) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} SAP purchase orders...`);

  const result = await synth.generateStructured({
    count,
    schema: purchaseOrderSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} POs in ${result.metadata.duration}ms`);
  console.log('Sample PO:', result.data[0]);

  return result;
}

/**
 * Generate Oracle Supply Chain Events (time-series)
 */
export async function generateSupplyChainEvents(count: number = 200) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} supply chain events...`);

  const result = await synth.generateEvents({
    count,
    eventTypes: ['shipment_departure', 'shipment_arrival', 'inventory_adjustment',
                'quality_check', 'customs_clearance', 'delivery_exception'],
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000), // 30 days ago
      end: new Date()
    }
  });

  console.log(`Generated ${result.data.length} events in ${result.metadata.duration}ms`);
  console.log('Sample event:', result.data[0]);

  return result;
}

/**
 * Generate Microsoft Dynamics 365 Manufacturing Orders
 */
export async function generateManufacturingOrders(count: number = 75) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} manufacturing orders...`);

  const result = await synth.generateStructured({
    count,
    schema: manufacturingProcessSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} orders in ${result.metadata.duration}ms`);
  console.log('Sample order:', result.data[0]);

  return result;
}

/**
 * Generate multi-location warehouse inventory snapshots
 */
export async function generateWarehouseInventory(warehouseCount: number = 5) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating inventory for ${warehouseCount} warehouses...`);

  const result = await synth.generateStructured({
    count: warehouseCount,
    schema: warehouseInventorySchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} warehouse snapshots in ${result.metadata.duration}ms`);
  console.log('Sample warehouse:', result.data[0]);

  return result;
}

/**
 * Generate SAP Financial Transactions (FI/CO)
 */
export async function generateFinancialTransactions(count: number = 500) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} financial transactions...`);

  const result = await synth.generateStructured({
    count,
    schema: financialTransactionSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} transactions in ${result.metadata.duration}ms`);
  console.log('Sample transaction:', result.data[0]);

  return result;
}

/**
 * Generate complete ERP dataset in parallel
 */
export async function generateCompleteERPDataset() {
  const synth = createSynth({
    provider: 'gemini',
    cacheStrategy: 'memory'
  });

  console.log('Generating complete ERP dataset in parallel...');
  console.time('Total ERP generation');

  const [materials, purchaseOrders, supplyChain, manufacturing, warehouses, financial] =
    await Promise.all([
      generateMaterialData(50),
      generatePurchaseOrders(25),
      generateSupplyChainEvents(100),
      generateManufacturingOrders(30),
      generateWarehouseInventory(3),
      generateFinancialTransactions(200)
    ]);

  console.timeEnd('Total ERP generation');

  return {
    materials: materials.data,
    purchaseOrders: purchaseOrders.data,
    supplyChainEvents: supplyChain.data,
    manufacturingOrders: manufacturing.data,
    warehouseInventory: warehouses.data,
    financialTransactions: financial.data,
    metadata: {
      totalRecords: materials.data.length + purchaseOrders.data.length +
                   supplyChain.data.length + manufacturing.data.length +
                   warehouses.data.length + financial.data.length,
      generatedAt: new Date().toISOString()
    }
  };
}

/**
 * Stream ERP data generation for large datasets
 */
export async function streamERPData(type: 'material' | 'po' | 'transaction', count: number = 1000) {
  const synth = createSynth({
    provider: 'gemini',
    streaming: true
  });

  const schemaMap = {
    material: materialSchema,
    po: purchaseOrderSchema,
    transaction: financialTransactionSchema
  };

  console.log(`Streaming ${count} ${type} records...`);

  let recordCount = 0;
  for await (const record of synth.generateStream('structured', {
    count,
    schema: schemaMap[type],
    format: 'json'
  })) {
    recordCount++;
    if (recordCount % 100 === 0) {
      console.log(`Streamed ${recordCount} records...`);
    }
  }

  console.log(`Completed streaming ${recordCount} ${type} records`);
}

// Example usage
async function runERPExamples() {
  console.log('=== ERP Data Generation Examples ===\n');

  // Example 1: Material Master Data
  console.log('1. Material Master Data (SAP MM)');
  await generateMaterialData(10);

  // Example 2: Purchase Orders
  console.log('\n2. Purchase Orders (SAP MM)');
  await generatePurchaseOrders(5);

  // Example 3: Supply Chain Events
  console.log('\n3. Supply Chain Events (Oracle)');
  await generateSupplyChainEvents(20);

  // Example 4: Manufacturing Orders
  console.log('\n4. Manufacturing Orders (Dynamics 365)');
  await generateManufacturingOrders(10);

  // Example 5: Warehouse Inventory
  console.log('\n5. Multi-location Warehouse Inventory');
  await generateWarehouseInventory(2);

  // Example 6: Financial Transactions
  console.log('\n6. Financial Transactions (SAP FI/CO)');
  await generateFinancialTransactions(25);

  // Example 7: Complete dataset in parallel
  console.log('\n7. Complete ERP Dataset (Parallel)');
  const completeDataset = await generateCompleteERPDataset();
  console.log('Total records generated:', completeDataset.metadata.totalRecords);
}

// Uncomment to run
// runERPExamples().catch(console.error);

export default {
  generateMaterialData,
  generatePurchaseOrders,
  generateSupplyChainEvents,
  generateManufacturingOrders,
  generateWarehouseInventory,
  generateFinancialTransactions,
  generateCompleteERPDataset,
  streamERPData
};
