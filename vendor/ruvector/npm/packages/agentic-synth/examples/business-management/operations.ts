/**
 * Business Operations Management Data Generation
 * Simulates project management, vendor management, contract lifecycle, and approval workflows
 */

import { createSynth } from '../../src/index.js';

// Project Management Schema (Jira/Asana/MS Project style)
const projectManagementSchema = {
  projectId: { type: 'string', required: true },
  projectName: { type: 'string', required: true },
  projectCode: { type: 'string', required: true },
  description: { type: 'string', required: true },
  projectType: { type: 'string', required: true },
  status: { type: 'string', required: true },
  priority: { type: 'string', required: true },
  businessUnit: { type: 'string', required: true },
  department: { type: 'string', required: true },
  timeline: { type: 'object', required: true, properties: {
    plannedStartDate: { type: 'string' },
    plannedEndDate: { type: 'string' },
    actualStartDate: { type: 'string' },
    actualEndDate: { type: 'string' },
    duration: { type: 'number' },
    percentComplete: { type: 'number' }
  }},
  team: { type: 'object', required: true, properties: {
    projectManager: { type: 'object', properties: {
      employeeId: { type: 'string' },
      name: { type: 'string' },
      email: { type: 'string' }
    }},
    sponsor: { type: 'object', properties: {
      employeeId: { type: 'string' },
      name: { type: 'string' },
      department: { type: 'string' }
    }},
    teamMembers: { type: 'array', items: {
      employeeId: { type: 'string' },
      name: { type: 'string' },
      role: { type: 'string' },
      allocation: { type: 'number' }
    }},
    stakeholders: { type: 'array' }
  }},
  budget: { type: 'object', required: true, properties: {
    plannedBudget: { type: 'number' },
    actualCost: { type: 'number' },
    committedCost: { type: 'number' },
    remainingBudget: { type: 'number' },
    variance: { type: 'number' },
    variancePercent: { type: 'number' },
    currency: { type: 'string' }
  }},
  phases: { type: 'array', required: true, items: {
    phaseId: { type: 'string' },
    phaseName: { type: 'string' },
    startDate: { type: 'string' },
    endDate: { type: 'string' },
    status: { type: 'string' },
    deliverables: { type: 'array' }
  }},
  tasks: { type: 'array', required: true, items: {
    taskId: { type: 'string' },
    taskName: { type: 'string' },
    description: { type: 'string' },
    assignee: { type: 'string' },
    status: { type: 'string' },
    priority: { type: 'string' },
    startDate: { type: 'string' },
    dueDate: { type: 'string' },
    completedDate: { type: 'string' },
    estimatedHours: { type: 'number' },
    actualHours: { type: 'number' },
    dependencies: { type: 'array' }
  }},
  risks: { type: 'array', required: false, items: {
    riskId: { type: 'string' },
    description: { type: 'string' },
    probability: { type: 'string' },
    impact: { type: 'string' },
    mitigation: { type: 'string' },
    owner: { type: 'string' },
    status: { type: 'string' }
  }},
  issues: { type: 'array', required: false, items: {
    issueId: { type: 'string' },
    description: { type: 'string' },
    severity: { type: 'string' },
    reportedBy: { type: 'string' },
    assignedTo: { type: 'string' },
    status: { type: 'string' },
    resolution: { type: 'string' }
  }},
  metrics: { type: 'object', required: true, properties: {
    schedulePerformanceIndex: { type: 'number' },
    costPerformanceIndex: { type: 'number' },
    earnedValue: { type: 'number' },
    plannedValue: { type: 'number' },
    actualCost: { type: 'number' },
    estimateAtCompletion: { type: 'number' }
  }}
};

// Resource Allocation Schema
const resourceAllocationSchema = {
  allocationId: { type: 'string', required: true },
  allocationDate: { type: 'string', required: true },
  period: { type: 'object', required: true, properties: {
    startDate: { type: 'string' },
    endDate: { type: 'string' }
  }},
  resource: { type: 'object', required: true, properties: {
    resourceId: { type: 'string' },
    resourceName: { type: 'string' },
    resourceType: { type: 'string' },
    department: { type: 'string' },
    costCenter: { type: 'string' },
    skillSet: { type: 'array' },
    seniorityLevel: { type: 'string' }
  }},
  project: { type: 'object', required: true, properties: {
    projectId: { type: 'string' },
    projectName: { type: 'string' },
    projectManager: { type: 'string' }
  }},
  allocation: { type: 'object', required: true, properties: {
    allocationPercent: { type: 'number' },
    hoursPerWeek: { type: 'number' },
    totalHours: { type: 'number' },
    billableRate: { type: 'number' },
    internalRate: { type: 'number' },
    currency: { type: 'string' }
  }},
  utilization: { type: 'object', required: true, properties: {
    totalCapacity: { type: 'number' },
    allocatedHours: { type: 'number' },
    availableHours: { type: 'number' },
    utilizationRate: { type: 'number' },
    overallocationHours: { type: 'number' }
  }},
  status: { type: 'string', required: true },
  approvedBy: { type: 'string', required: false },
  approvalDate: { type: 'string', required: false }
};

// Vendor Management Schema
const vendorManagementSchema = {
  vendorId: { type: 'string', required: true },
  vendorName: { type: 'string', required: true },
  vendorType: { type: 'string', required: true },
  status: { type: 'string', required: true },
  tier: { type: 'string', required: true },
  contactInfo: { type: 'object', required: true, properties: {
    primaryContact: { type: 'object', properties: {
      name: { type: 'string' },
      title: { type: 'string' },
      email: { type: 'string' },
      phone: { type: 'string' }
    }},
    accountManager: { type: 'object', properties: {
      name: { type: 'string' },
      email: { type: 'string' }
    }},
    address: { type: 'object', properties: {
      street: { type: 'string' },
      city: { type: 'string' },
      state: { type: 'string' },
      country: { type: 'string' },
      postalCode: { type: 'string' }
    }},
    website: { type: 'string' },
    taxId: { type: 'string' }
  }},
  businessDetails: { type: 'object', required: true, properties: {
    industry: { type: 'string' },
    yearEstablished: { type: 'number' },
    numberOfEmployees: { type: 'number' },
    annualRevenue: { type: 'number' },
    certifications: { type: 'array' },
    servicesProvided: { type: 'array' }
  }},
  contractInfo: { type: 'object', required: true, properties: {
    activeContracts: { type: 'number' },
    totalContractValue: { type: 'number' },
    contractStartDate: { type: 'string' },
    contractEndDate: { type: 'string' },
    renewalDate: { type: 'string' },
    paymentTerms: { type: 'string' },
    currency: { type: 'string' }
  }},
  performance: { type: 'object', required: true, properties: {
    overallScore: { type: 'number' },
    qualityScore: { type: 'number' },
    deliveryScore: { type: 'number' },
    complianceScore: { type: 'number' },
    responsiveScore: { type: 'number' },
    lastReviewDate: { type: 'string' },
    nextReviewDate: { type: 'string' }
  }},
  riskAssessment: { type: 'object', required: true, properties: {
    riskLevel: { type: 'string' },
    financialRisk: { type: 'string' },
    operationalRisk: { type: 'string' },
    complianceRisk: { type: 'string' },
    cyberSecurityRisk: { type: 'string' },
    lastAuditDate: { type: 'string' }
  }},
  spending: { type: 'object', required: true, properties: {
    ytdSpending: { type: 'number' },
    lifetimeSpending: { type: 'number' },
    averageInvoiceAmount: { type: 'number' },
    paymentHistory: { type: 'object', properties: {
      onTimePaymentRate: { type: 'number' },
      averageDaysToPay: { type: 'number' }
    }}
  }},
  compliance: { type: 'object', required: false, properties: {
    insuranceCertificate: { type: 'boolean' },
    w9Form: { type: 'boolean' },
    nda: { type: 'boolean' },
    backgroundCheckCompleted: { type: 'boolean' },
    lastComplianceCheck: { type: 'string' }
  }},
  documents: { type: 'array', required: false }
};

// Contract Lifecycle Management Schema
const contractLifecycleSchema = {
  contractId: { type: 'string', required: true },
  contractNumber: { type: 'string', required: true },
  contractName: { type: 'string', required: true },
  contractType: { type: 'string', required: true },
  status: { type: 'string', required: true },
  parties: { type: 'object', required: true, properties: {
    buyer: { type: 'object', properties: {
      companyCode: { type: 'string' },
      companyName: { type: 'string' },
      legalEntity: { type: 'string' },
      signatoryName: { type: 'string' },
      signatoryTitle: { type: 'string' }
    }},
    seller: { type: 'object', properties: {
      vendorId: { type: 'string' },
      vendorName: { type: 'string' },
      legalEntity: { type: 'string' },
      signatoryName: { type: 'string' },
      signatoryTitle: { type: 'string' }
    }}
  }},
  timeline: { type: 'object', required: true, properties: {
    requestDate: { type: 'string' },
    approvalDate: { type: 'string' },
    executionDate: { type: 'string' },
    effectiveDate: { type: 'string' },
    expirationDate: { type: 'string' },
    autoRenewal: { type: 'boolean' },
    renewalNoticeDays: { type: 'number' },
    terminationNoticeDays: { type: 'number' }
  }},
  financial: { type: 'object', required: true, properties: {
    totalContractValue: { type: 'number' },
    currency: { type: 'string' },
    billingFrequency: { type: 'string' },
    paymentTerms: { type: 'string' },
    annualValue: { type: 'number' },
    invoicedToDate: { type: 'number' },
    paidToDate: { type: 'number' },
    outstandingBalance: { type: 'number' }
  }},
  terms: { type: 'object', required: true, properties: {
    scopeOfWork: { type: 'string' },
    deliverables: { type: 'array' },
    serviceLevelAgreements: { type: 'array' },
    penaltyClause: { type: 'boolean' },
    warrantyPeriod: { type: 'number' },
    liabilityLimit: { type: 'number' },
    confidentialityClause: { type: 'boolean' },
    nonCompeteClause: { type: 'boolean' }
  }},
  obligations: { type: 'array', required: true, items: {
    obligationId: { type: 'string' },
    description: { type: 'string' },
    responsibleParty: { type: 'string' },
    dueDate: { type: 'string' },
    status: { type: 'string' },
    completedDate: { type: 'string' }
  }},
  amendments: { type: 'array', required: false, items: {
    amendmentNumber: { type: 'string' },
    amendmentDate: { type: 'string' },
    description: { type: 'string' },
    financialImpact: { type: 'number' }
  }},
  owners: { type: 'object', required: true, properties: {
    contractOwner: { type: 'string' },
    businessOwner: { type: 'string' },
    legalReviewer: { type: 'string' },
    financeApprover: { type: 'string' }
  }},
  compliance: { type: 'object', required: true, properties: {
    regulatoryCompliance: { type: 'boolean' },
    dataPrivacyCompliance: { type: 'boolean' },
    lastAuditDate: { type: 'string' },
    nextReviewDate: { type: 'string' }
  }},
  risks: { type: 'array', required: false },
  documents: { type: 'array', required: false }
};

// Approval Workflow Schema
const approvalWorkflowSchema = {
  workflowId: { type: 'string', required: true },
  requestId: { type: 'string', required: true },
  requestType: { type: 'string', required: true },
  requestDate: { type: 'string', required: true },
  currentStatus: { type: 'string', required: true },
  priority: { type: 'string', required: true },
  requester: { type: 'object', required: true, properties: {
    employeeId: { type: 'string' },
    employeeName: { type: 'string' },
    department: { type: 'string' },
    email: { type: 'string' }
  }},
  requestDetails: { type: 'object', required: true, properties: {
    subject: { type: 'string' },
    description: { type: 'string' },
    category: { type: 'string' },
    subcategory: { type: 'string' },
    businessJustification: { type: 'string' },
    urgency: { type: 'string' }
  }},
  financialDetails: { type: 'object', required: false, properties: {
    amount: { type: 'number' },
    currency: { type: 'string' },
    budgetCode: { type: 'string' },
    costCenter: { type: 'string' },
    budgetAvailable: { type: 'boolean' }
  }},
  approvalChain: { type: 'array', required: true, items: {
    stepNumber: { type: 'number' },
    approverRole: { type: 'string' },
    approverId: { type: 'string' },
    approverName: { type: 'string' },
    approverEmail: { type: 'string' },
    status: { type: 'string' },
    assignedDate: { type: 'string' },
    responseDate: { type: 'string' },
    decision: { type: 'string' },
    comments: { type: 'string' },
    durationHours: { type: 'number' }
  }},
  routing: { type: 'object', required: true, properties: {
    routingType: { type: 'string' },
    parallelApprovals: { type: 'boolean' },
    escalationEnabled: { type: 'boolean' },
    escalationAfterHours: { type: 'number' },
    notificationEnabled: { type: 'boolean' }
  }},
  timeline: { type: 'object', required: true, properties: {
    submittedDate: { type: 'string' },
    firstApprovalDate: { type: 'string' },
    finalApprovalDate: { type: 'string' },
    completedDate: { type: 'string' },
    totalDurationHours: { type: 'number' },
    slaTarget: { type: 'number' },
    slaBreached: { type: 'boolean' }
  }},
  attachments: { type: 'array', required: false },
  audit: { type: 'array', required: true, items: {
    timestamp: { type: 'string' },
    action: { type: 'string' },
    performedBy: { type: 'string' },
    details: { type: 'string' }
  }}
};

// Audit Trail Schema
const auditTrailSchema = {
  auditId: { type: 'string', required: true },
  timestamp: { type: 'string', required: true },
  eventType: { type: 'string', required: true },
  entity: { type: 'object', required: true, properties: {
    entityType: { type: 'string' },
    entityId: { type: 'string' },
    entityName: { type: 'string' }
  }},
  action: { type: 'string', required: true },
  actor: { type: 'object', required: true, properties: {
    userId: { type: 'string' },
    userName: { type: 'string' },
    userRole: { type: 'string' },
    department: { type: 'string' },
    ipAddress: { type: 'string' },
    sessionId: { type: 'string' }
  }},
  changes: { type: 'array', required: false, items: {
    fieldName: { type: 'string' },
    oldValue: { type: 'string' },
    newValue: { type: 'string' },
    dataType: { type: 'string' }
  }},
  metadata: { type: 'object', required: true, properties: {
    source: { type: 'string' },
    application: { type: 'string' },
    module: { type: 'string' },
    transactionId: { type: 'string' },
    severity: { type: 'string' }
  }},
  compliance: { type: 'object', required: false, properties: {
    regulationApplicable: { type: 'array' },
    retentionYears: { type: 'number' },
    classification: { type: 'string' }
  }},
  result: { type: 'object', required: true, properties: {
    status: { type: 'string' },
    errorCode: { type: 'string' },
    errorMessage: { type: 'string' }
  }}
};

/**
 * Generate Project Management Data
 */
export async function generateProjects(count: number = 50) {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  console.log(`Generating ${count} project records...`);

  const result = await synth.generateStructured({
    count,
    schema: projectManagementSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} projects in ${result.metadata.duration}ms`);
  console.log('Sample project:', result.data[0]);

  return result;
}

/**
 * Generate Resource Allocation Data
 */
export async function generateResourceAllocations(count: number = 200) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} resource allocations...`);

  const result = await synth.generateStructured({
    count,
    schema: resourceAllocationSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} allocations in ${result.metadata.duration}ms`);
  console.log('Sample allocation:', result.data[0]);

  return result;
}

/**
 * Generate Vendor Management Data
 */
export async function generateVendors(count: number = 75) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} vendor records...`);

  const result = await synth.generateStructured({
    count,
    schema: vendorManagementSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} vendors in ${result.metadata.duration}ms`);
  console.log('Sample vendor:', result.data[0]);

  return result;
}

/**
 * Generate Contract Lifecycle Data
 */
export async function generateContracts(count: number = 100) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} contracts...`);

  const result = await synth.generateStructured({
    count,
    schema: contractLifecycleSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} contracts in ${result.metadata.duration}ms`);
  console.log('Sample contract:', result.data[0]);

  return result;
}

/**
 * Generate Approval Workflow Data
 */
export async function generateApprovalWorkflows(count: number = 300) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} approval workflows...`);

  const result = await synth.generateStructured({
    count,
    schema: approvalWorkflowSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} workflows in ${result.metadata.duration}ms`);
  console.log('Sample workflow:', result.data[0]);

  return result;
}

/**
 * Generate Audit Trail Data (time-series)
 */
export async function generateAuditTrail(count: number = 1000) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} audit trail entries...`);

  const result = await synth.generateEvents({
    count,
    eventTypes: ['create', 'read', 'update', 'delete', 'approve', 'reject', 'login', 'logout'],
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000), // 30 days ago
      end: new Date()
    }
  });

  console.log(`Generated ${result.data.length} audit entries in ${result.metadata.duration}ms`);
  console.log('Sample audit entry:', result.data[0]);

  return result;
}

/**
 * Generate complete operations dataset in parallel
 */
export async function generateCompleteOperationsDataset() {
  const synth = createSynth({
    provider: 'gemini',
    cacheStrategy: 'memory'
  });

  console.log('Generating complete operations dataset in parallel...');
  console.time('Total operations generation');

  const [projects, resources, vendors, contracts, workflows, audit] =
    await Promise.all([
      generateProjects(30),
      generateResourceAllocations(100),
      generateVendors(50),
      generateContracts(60),
      generateApprovalWorkflows(150),
      generateAuditTrail(500)
    ]);

  console.timeEnd('Total operations generation');

  return {
    projects: projects.data,
    resourceAllocations: resources.data,
    vendors: vendors.data,
    contracts: contracts.data,
    approvalWorkflows: workflows.data,
    auditTrail: audit.data,
    metadata: {
      totalRecords: projects.data.length + resources.data.length +
                   vendors.data.length + contracts.data.length +
                   workflows.data.length + audit.data.length,
      generatedAt: new Date().toISOString()
    }
  };
}

/**
 * Simulate end-to-end procurement workflow
 */
export async function simulateProcurementWorkflow() {
  console.log('Simulating complete procurement workflow...');
  console.time('Procurement workflow');

  // Step 1: Vendor onboarding
  const vendors = await generateVendors(5);
  console.log(`✓ Onboarded ${vendors.data.length} vendors`);

  // Step 2: Contract creation
  const contracts = await generateContracts(5);
  console.log(`✓ Created ${contracts.data.length} contracts`);

  // Step 3: Approval workflows for contracts
  const approvals = await generateApprovalWorkflows(10);
  console.log(`✓ Processed ${approvals.data.length} approval workflows`);

  // Step 4: Audit trail
  const audit = await generateAuditTrail(50);
  console.log(`✓ Logged ${audit.data.length} audit events`);

  console.timeEnd('Procurement workflow');

  return {
    vendors: vendors.data,
    contracts: contracts.data,
    approvals: approvals.data,
    auditTrail: audit.data,
    summary: {
      vendorsOnboarded: vendors.data.length,
      contractsCreated: contracts.data.length,
      approvalsProcessed: approvals.data.length,
      auditEvents: audit.data.length
    }
  };
}

// Example usage
async function runOperationsExamples() {
  console.log('=== Business Operations Data Generation Examples ===\n');

  // Example 1: Project Management
  console.log('1. Project Management');
  await generateProjects(5);

  // Example 2: Resource Allocation
  console.log('\n2. Resource Allocation');
  await generateResourceAllocations(20);

  // Example 3: Vendor Management
  console.log('\n3. Vendor Management');
  await generateVendors(10);

  // Example 4: Contract Lifecycle
  console.log('\n4. Contract Lifecycle Management');
  await generateContracts(10);

  // Example 5: Approval Workflows
  console.log('\n5. Approval Workflows');
  await generateApprovalWorkflows(30);

  // Example 6: Audit Trail
  console.log('\n6. Audit Trail');
  await generateAuditTrail(100);

  // Example 7: Procurement Workflow Simulation
  console.log('\n7. Procurement Workflow Simulation');
  await simulateProcurementWorkflow();

  // Example 8: Complete operations dataset
  console.log('\n8. Complete Operations Dataset (Parallel)');
  const completeDataset = await generateCompleteOperationsDataset();
  console.log('Total records generated:', completeDataset.metadata.totalRecords);
}

// Uncomment to run
// runOperationsExamples().catch(console.error);

export default {
  generateProjects,
  generateResourceAllocations,
  generateVendors,
  generateContracts,
  generateApprovalWorkflows,
  generateAuditTrail,
  generateCompleteOperationsDataset,
  simulateProcurementWorkflow
};
