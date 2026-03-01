# Business Management Simulation Examples

Comprehensive enterprise business management data generation examples using agentic-synth for ERP, CRM, HR, Financial, and Operations systems.

## Overview

This directory contains production-ready examples for generating synthetic data that simulates real enterprise systems including SAP, Salesforce, Microsoft Dynamics, Oracle, and other major business platforms.

## Files

### 1. ERP Data (`erp-data.ts`)
Enterprise Resource Planning data generation including:
- **Material Management** - SAP MM material master records
- **Purchase Orders** - Complete PO workflows with line items
- **Supply Chain Events** - Oracle-style supply chain event tracking
- **Manufacturing Orders** - Microsoft Dynamics 365 production orders
- **Warehouse Inventory** - Multi-location warehouse management
- **Financial Transactions** - SAP FI/CO transaction documents

**Use Cases:**
- SAP S/4HANA system testing
- Oracle ERP Cloud integration testing
- Microsoft Dynamics 365 data migration
- Supply chain analytics development
- Inventory management system testing

### 2. CRM Simulation (`crm-simulation.ts`)
Customer Relationship Management data including:
- **Lead Generation** - Salesforce lead qualification pipeline
- **Sales Pipeline** - Opportunity management with forecasting
- **Contact Interactions** - HubSpot-style engagement tracking
- **Account Management** - Microsoft Dynamics 365 account hierarchies
- **Support Tickets** - Service Cloud case management
- **Customer LTV** - Lifetime value analysis and churn prediction

**Use Cases:**
- Salesforce development and testing
- Sales analytics dashboard development
- Customer journey mapping
- Marketing automation testing
- Support team training data

### 3. HR Management (`hr-management.ts`)
Human Resources data generation including:
- **Employee Profiles** - Workday-style employee master data
- **Recruitment Pipeline** - SAP SuccessFactors applicant tracking
- **Performance Reviews** - Oracle HCM performance management
- **Payroll Data** - Workday payroll processing records
- **Time & Attendance** - Time tracking and shift management
- **Training Records** - Learning and development tracking

**Use Cases:**
- Workday system testing
- SAP SuccessFactors integration
- Oracle HCM Cloud development
- HR analytics and reporting
- Compliance testing (GDPR, SOC 2)

### 4. Financial Planning (`financial-planning.ts`)
Financial management and FP&A data including:
- **Budget Planning** - Departmental and project budgets
- **Revenue Forecasting** - Multi-scenario revenue projections
- **Expense Tracking** - Real-time expense monitoring with variance
- **Cash Flow Projections** - Operating, investing, financing activities
- **P&L Statements** - Income statements with YoY comparisons
- **Balance Sheets** - Complete financial position statements
- **KPI Dashboards** - Real-time financial metrics and alerts

**Use Cases:**
- Financial system testing (SAP, Oracle Financials)
- FP&A tool development
- Business intelligence dashboards
- Budget vs actual analysis
- Financial modeling and forecasting

### 5. Operations (`operations.ts`)
Business operations management including:
- **Project Management** - Jira/MS Project style project tracking
- **Resource Allocation** - Team member utilization and assignment
- **Vendor Management** - Supplier performance and compliance
- **Contract Lifecycle** - Complete CLM workflows
- **Approval Workflows** - Multi-step approval processes
- **Audit Trails** - Comprehensive activity logging

**Use Cases:**
- Project management tool development
- Procurement system testing
- Contract management systems
- Workflow automation testing
- Compliance and audit reporting

## Quick Start

### Basic Usage

```typescript
import { generateMaterialData } from './erp-data.js';
import { generateLeads } from './crm-simulation.js';
import { generateEmployeeProfiles } from './hr-management.js';
import { generateBudgetPlans } from './financial-planning.js';
import { generateProjects } from './operations.js';

// Generate 100 material master records
const materials = await generateMaterialData(100);

// Generate 50 sales leads
const leads = await generateLeads(50);

// Generate 200 employee profiles
const employees = await generateEmployeeProfiles(200);

// Generate 25 budget plans
const budgets = await generateBudgetPlans(25);

// Generate 30 project records
const projects = await generateProjects(30);
```

### Complete Dataset Generation

Generate entire business system datasets in parallel:

```typescript
import { generateCompleteERPDataset } from './erp-data.js';
import { generateCompleteCRMDataset } from './crm-simulation.js';
import { generateCompleteHRDataset } from './hr-management.js';
import { generateCompleteFinancialDataset } from './financial-planning.js';
import { generateCompleteOperationsDataset } from './operations.js';

// Generate all datasets concurrently
const [erp, crm, hr, financial, operations] = await Promise.all([
  generateCompleteERPDataset(),
  generateCompleteCRMDataset(),
  generateCompleteHRDataset(),
  generateCompleteFinancialDataset(),
  generateCompleteOperationsDataset()
]);

console.log('Total records:',
  erp.metadata.totalRecords +
  crm.metadata.totalRecords +
  hr.metadata.totalRecords +
  financial.metadata.totalRecords +
  operations.metadata.totalRecords
);
```

### Streaming Large Datasets

For generating millions of records efficiently:

```typescript
import { streamERPData } from './erp-data.js';
import { streamCRMInteractions } from './crm-simulation.js';

// Stream 1 million material records
await streamERPData('material', 1000000);

// Stream CRM interactions for 24 hours
await streamCRMInteractions(86400); // 24 hours in seconds
```

## Enterprise System Integrations

### SAP Integration

**SAP S/4HANA:**
```typescript
import { generateMaterialData, generatePurchaseOrders, generateFinancialTransactions } from './erp-data.js';

// Generate SAP MM data
const materials = await generateMaterialData(1000);

// Generate SAP PO data
const pos = await generatePurchaseOrders(500);

// Generate SAP FI/CO transactions
const transactions = await generateFinancialTransactions(5000);

// Export to SAP IDoc format
const idocs = materials.data.map(material => ({
  IDOC_TYPE: 'MATMAS',
  MATERIAL: material.materialNumber,
  // ... map to SAP structure
}));
```

**SAP SuccessFactors:**
```typescript
import { generateEmployeeProfiles, generatePerformanceReviews } from './hr-management.js';

// Generate employee data for SuccessFactors
const employees = await generateEmployeeProfiles(500);

// Generate performance review data
const reviews = await generatePerformanceReviews(500);

// Export to SuccessFactors OData format
const odataEmployees = employees.data.map(emp => ({
  userId: emp.employeeId,
  firstName: emp.firstName,
  // ... map to SuccessFactors structure
}));
```

### Salesforce Integration

**Salesforce Sales Cloud:**
```typescript
import { generateLeads, generateOpportunities, generateAccounts } from './crm-simulation.js';

// Generate Salesforce data
const leads = await generateLeads(1000);
const opportunities = await generateOpportunities(500);
const accounts = await generateAccounts(200);

// Export to Salesforce bulk API format
const sfLeads = leads.data.map(lead => ({
  FirstName: lead.firstName,
  LastName: lead.lastName,
  Company: lead.company,
  Email: lead.email,
  LeadSource: lead.leadSource,
  Status: lead.status,
  Rating: lead.rating
}));

// Use Salesforce Bulk API
// await salesforce.bulk.load('Lead', 'insert', sfLeads);
```

**Salesforce Service Cloud:**
```typescript
import { generateSupportTickets } from './crm-simulation.js';

// Generate Service Cloud cases
const tickets = await generateSupportTickets(1000);

// Export to Salesforce Case format
const sfCases = tickets.data.map(ticket => ({
  Subject: ticket.subject,
  Description: ticket.description,
  Status: ticket.status,
  Priority: ticket.priority,
  Origin: ticket.origin
}));
```

### Microsoft Dynamics Integration

**Dynamics 365 Finance & Operations:**
```typescript
import { generateManufacturingOrders } from './erp-data.js';
import { generateBudgetPlans, generateProfitLossStatements } from './financial-planning.ts';

// Generate manufacturing data
const prodOrders = await generateManufacturingOrders(200);

// Generate financial data
const budgets = await generateBudgetPlans(50);
const financials = await generateProfitLossStatements(12);

// Export to Dynamics 365 data entities
const d365ProdOrders = prodOrders.data.map(order => ({
  ProductionOrderNumber: order.productionOrderId,
  ItemNumber: order.product.itemNumber,
  OrderedQuantity: order.quantity.ordered,
  // ... map to Dynamics structure
}));
```

**Dynamics 365 CRM:**
```typescript
import { generateAccounts, generateOpportunities } from './crm-simulation.js';

// Generate CRM data
const accounts = await generateAccounts(500);
const opportunities = await generateOpportunities(300);

// Export to Dynamics 365 format
const d365Accounts = accounts.data.map(account => ({
  name: account.accountName,
  accountnumber: account.accountNumber,
  industrycode: account.industry,
  revenue: account.annualRevenue,
  // ... map to Dynamics structure
}));
```

### Oracle Integration

**Oracle ERP Cloud:**
```typescript
import { generateSupplyChainEvents, generatePurchaseOrders } from './erp-data.js';

// Generate Oracle ERP data
const scEvents = await generateSupplyChainEvents(1000);
const pos = await generatePurchaseOrders(500);

// Export to Oracle REST API format
const oracleEvents = scEvents.data.map(event => ({
  EventId: event.eventId,
  EventType: event.eventType,
  EventTimestamp: event.timestamp,
  // ... map to Oracle structure
}));
```

**Oracle HCM Cloud:**
```typescript
import { generateEmployeeProfiles, generatePerformanceReviews } from './hr-management.js';

// Generate Oracle HCM data
const employees = await generateEmployeeProfiles(1000);
const reviews = await generatePerformanceReviews(800);

// Export to Oracle HCM REST API format
const oracleWorkers = employees.data.map(emp => ({
  PersonNumber: emp.employeeNumber,
  FirstName: emp.firstName,
  LastName: emp.lastName,
  // ... map to Oracle structure
}));
```

### Workday Integration

```typescript
import { generateEmployeeProfiles, generatePayrollData } from './hr-management.js';

// Generate Workday data
const employees = await generateEmployeeProfiles(500);
const payroll = await generatePayrollData(2000);

// Export to Workday Web Services format
const workdayWorkers = employees.data.map(emp => ({
  Worker_Reference: {
    ID: {
      _: emp.employeeId,
      type: 'Employee_ID'
    }
  },
  Personal_Data: {
    Name_Data: {
      Legal_Name: {
        First_Name: emp.firstName,
        Last_Name: emp.lastName
      }
    }
  }
  // ... map to Workday XML structure
}));
```

## Advanced Usage

### Custom Schema Extension

Extend existing schemas with custom fields:

```typescript
import { createSynth } from '../../src/index.js';

// Custom extended employee schema
const customEmployeeSchema = {
  ...employeeProfileSchema,
  customFields: {
    type: 'object',
    required: false,
    properties: {
      securityClearance: { type: 'string' },
      badgeNumber: { type: 'string' },
      parkingSpot: { type: 'string' }
    }
  }
};

const synth = createSynth();
const result = await synth.generateStructured({
  count: 100,
  schema: customEmployeeSchema,
  format: 'json'
});
```

### Multi-Tenant Data Generation

Generate data for multiple organizations:

```typescript
const organizations = ['org1', 'org2', 'org3'];

const allData = await Promise.all(
  organizations.map(async (org) => {
    const [erp, crm, hr] = await Promise.all([
      generateCompleteERPDataset(),
      generateCompleteCRMDataset(),
      generateCompleteHRDataset()
    ]);

    return {
      organizationId: org,
      data: { erp, crm, hr }
    };
  })
);
```

### Real-Time Simulation

Simulate real-time business operations:

```typescript
import { generateContactInteractions } from './crm-simulation.js';
import { generateAuditTrail } from './operations.js';

// Simulate 24/7 operations
async function simulateRealTime() {
  while (true) {
    // Generate interactions every 5 seconds
    const interactions = await generateContactInteractions(10);
    console.log(`Generated ${interactions.data.length} interactions`);

    // Generate audit events
    const audit = await generateAuditTrail(20);
    console.log(`Logged ${audit.data.length} audit events`);

    // Wait 5 seconds
    await new Promise(resolve => setTimeout(resolve, 5000));
  }
}
```

### Data Validation

Validate generated data against business rules:

```typescript
import { generatePurchaseOrders } from './erp-data.js';

const pos = await generatePurchaseOrders(100);

// Validate PO data
const validPOs = pos.data.filter(po => {
  // Check totals match
  const itemsTotal = po.items.reduce((sum, item) => sum + item.netValue, 0);
  const totalMatch = Math.abs(itemsTotal - po.totalAmount) < 0.01;

  // Check dates are logical
  const dateValid = new Date(po.poDate) <= new Date();

  return totalMatch && dateValid;
});

console.log(`Valid POs: ${validPOs.length}/${pos.data.length}`);
```

## Performance Considerations

### Batch Generation

For large datasets, use batch generation:

```typescript
import { createSynth } from '../../src/index.js';

const synth = createSynth({
  cacheStrategy: 'memory',
  cacheTTL: 3600
});

// Generate in batches of 1000
const batchSize = 1000;
const totalRecords = 100000;
const batches = Math.ceil(totalRecords / batchSize);

for (let i = 0; i < batches; i++) {
  const batch = await synth.generateStructured({
    count: batchSize,
    schema: materialSchema,
    format: 'json'
  });

  console.log(`Batch ${i + 1}/${batches} complete`);

  // Process or save batch
  // await saveToDB(batch.data);
}
```

### Memory Management

For very large datasets, use streaming:

```typescript
import { streamERPData } from './erp-data.js';
import fs from 'fs';

// Stream to file
const writeStream = fs.createWriteStream('materials.jsonl');

let recordCount = 0;
for await (const record of streamERPData('material', 1000000)) {
  writeStream.write(JSON.stringify(record) + '\n');
  recordCount++;

  if (recordCount % 10000 === 0) {
    console.log(`Processed ${recordCount} records`);
  }
}

writeStream.end();
```

### Parallel Processing

Maximize throughput with parallel generation:

```typescript
import pLimit from 'p-limit';

// Limit to 5 concurrent generations
const limit = pLimit(5);

const tasks = [
  () => generateMaterialData(1000),
  () => generatePurchaseOrders(500),
  () => generateLeads(1000),
  () => generateEmployeeProfiles(500),
  () => generateProjects(200)
];

const results = await Promise.all(
  tasks.map(task => limit(task))
);

console.log('All generations complete');
```

## Testing & Validation

### Unit Testing

```typescript
import { describe, it, expect } from 'vitest';
import { generateLeads } from './crm-simulation.js';

describe('CRM Lead Generation', () => {
  it('should generate specified number of leads', async () => {
    const result = await generateLeads(50);
    expect(result.data).toHaveLength(50);
  });

  it('should have valid email addresses', async () => {
    const result = await generateLeads(10);
    result.data.forEach(lead => {
      expect(lead.email).toMatch(/^[^\s@]+@[^\s@]+\.[^\s@]+$/);
    });
  });

  it('should have lead scores between 0-100', async () => {
    const result = await generateLeads(10);
    result.data.forEach(lead => {
      expect(lead.leadScore).toBeGreaterThanOrEqual(0);
      expect(lead.leadScore).toBeLessThanOrEqual(100);
    });
  });
});
```

### Integration Testing

```typescript
import { generateCompleteERPDataset } from './erp-data.js';

describe('ERP Dataset Integration', () => {
  it('should generate complete linked dataset', async () => {
    const dataset = await generateCompleteERPDataset();

    // Verify data relationships
    expect(dataset.materials.length).toBeGreaterThan(0);
    expect(dataset.purchaseOrders.length).toBeGreaterThan(0);

    // Verify total count
    expect(dataset.metadata.totalRecords).toBeGreaterThan(0);
  });
});
```

## Configuration

### Environment Variables

```bash
# API Keys
GEMINI_API_KEY=your_gemini_key
OPENROUTER_API_KEY=your_openrouter_key

# Cache Configuration
CACHE_STRATEGY=memory
CACHE_TTL=3600

# Generation Settings
DEFAULT_PROVIDER=gemini
DEFAULT_MODEL=gemini-2.0-flash-exp
STREAMING_ENABLED=false
```

### Custom Configuration

```typescript
import { createSynth } from '../../src/index.js';

const synth = createSynth({
  provider: 'gemini',
  apiKey: process.env.GEMINI_API_KEY,
  model: 'gemini-2.0-flash-exp',
  cacheStrategy: 'memory',
  cacheTTL: 3600,
  maxRetries: 3,
  timeout: 30000,
  streaming: false
});
```

## Best Practices

1. **Start Small**: Generate small datasets first to validate schemas
2. **Use Caching**: Enable caching for repeated operations
3. **Batch Processing**: Use batches for large datasets
4. **Validate Data**: Implement validation rules for business logic
5. **Error Handling**: Wrap generations in try-catch blocks
6. **Monitor Performance**: Track generation times and optimize
7. **Version Control**: Track schema changes and data versions
8. **Document Assumptions**: Document business rules and assumptions

## Troubleshooting

### Common Issues

**Issue**: Generation is slow
- **Solution**: Enable caching, use batch processing, or parallel generation

**Issue**: Out of memory errors
- **Solution**: Use streaming for large datasets, reduce batch sizes

**Issue**: Data doesn't match expected format
- **Solution**: Validate schemas, check type definitions

**Issue**: API rate limits
- **Solution**: Implement retry logic, use multiple API keys

## Support

For issues, questions, or contributions:
- GitHub Issues: https://github.com/ruvnet/agentic-synth/issues
- Documentation: https://github.com/ruvnet/agentic-synth/docs
- Examples: https://github.com/ruvnet/agentic-synth/examples

## License

MIT License - see LICENSE file for details
