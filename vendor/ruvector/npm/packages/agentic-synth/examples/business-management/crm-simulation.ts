/**
 * Customer Relationship Management (CRM) Data Generation
 * Simulates Salesforce, Microsoft Dynamics CRM, and HubSpot scenarios
 */

import { createSynth } from '../../src/index.js';

// Salesforce Lead Schema
const leadSchema = {
  leadId: { type: 'string', required: true },
  firstName: { type: 'string', required: true },
  lastName: { type: 'string', required: true },
  email: { type: 'string', required: true },
  phone: { type: 'string', required: false },
  company: { type: 'string', required: true },
  title: { type: 'string', required: true },
  industry: { type: 'string', required: true },
  numberOfEmployees: { type: 'number', required: false },
  annualRevenue: { type: 'number', required: false },
  leadSource: { type: 'string', required: true },
  status: { type: 'string', required: true },
  rating: { type: 'string', required: true },
  address: { type: 'object', required: false, properties: {
    street: { type: 'string' },
    city: { type: 'string' },
    state: { type: 'string' },
    postalCode: { type: 'string' },
    country: { type: 'string' }
  }},
  description: { type: 'string', required: false },
  website: { type: 'string', required: false },
  leadScore: { type: 'number', required: true },
  conversionProbability: { type: 'number', required: true },
  ownerId: { type: 'string', required: true },
  ownerName: { type: 'string', required: true },
  createdDate: { type: 'string', required: true },
  lastActivityDate: { type: 'string', required: false },
  convertedDate: { type: 'string', required: false },
  convertedAccountId: { type: 'string', required: false },
  convertedContactId: { type: 'string', required: false },
  convertedOpportunityId: { type: 'string', required: false }
};

// Salesforce Sales Pipeline (Opportunity) Schema
const opportunitySchema = {
  opportunityId: { type: 'string', required: true },
  opportunityName: { type: 'string', required: true },
  accountId: { type: 'string', required: true },
  accountName: { type: 'string', required: true },
  type: { type: 'string', required: true },
  stage: { type: 'string', required: true },
  amount: { type: 'number', required: true },
  probability: { type: 'number', required: true },
  expectedRevenue: { type: 'number', required: true },
  closeDate: { type: 'string', required: true },
  nextStep: { type: 'string', required: false },
  leadSource: { type: 'string', required: true },
  campaignId: { type: 'string', required: false },
  ownerId: { type: 'string', required: true },
  ownerName: { type: 'string', required: true },
  createdDate: { type: 'string', required: true },
  lastModifiedDate: { type: 'string', required: true },
  products: { type: 'array', required: true, items: {
    productId: { type: 'string' },
    productName: { type: 'string' },
    quantity: { type: 'number' },
    listPrice: { type: 'number' },
    salesPrice: { type: 'number' },
    discount: { type: 'number' },
    totalPrice: { type: 'number' }
  }},
  competitors: { type: 'array', required: false },
  description: { type: 'string', required: false },
  isClosed: { type: 'boolean', required: true },
  isWon: { type: 'boolean', required: false },
  lostReason: { type: 'string', required: false },
  forecastCategory: { type: 'string', required: true }
};

// HubSpot Contact Interaction Schema
const contactInteractionSchema = {
  interactionId: { type: 'string', required: true },
  contactId: { type: 'string', required: true },
  contactEmail: { type: 'string', required: true },
  interactionType: { type: 'string', required: true },
  timestamp: { type: 'string', required: true },
  channel: { type: 'string', required: true },
  subject: { type: 'string', required: false },
  body: { type: 'string', required: false },
  duration: { type: 'number', required: false },
  outcome: { type: 'string', required: false },
  sentiment: { type: 'string', required: false },
  engagement: { type: 'object', required: true, properties: {
    opened: { type: 'boolean' },
    clicked: { type: 'boolean' },
    replied: { type: 'boolean' },
    bounced: { type: 'boolean' },
    unsubscribed: { type: 'boolean' }
  }},
  associatedDealId: { type: 'string', required: false },
  associatedTicketId: { type: 'string', required: false },
  ownerId: { type: 'string', required: true },
  properties: { type: 'object', required: false }
};

// Microsoft Dynamics 365 Account Management Schema
const accountSchema = {
  accountId: { type: 'string', required: true },
  accountName: { type: 'string', required: true },
  accountNumber: { type: 'string', required: true },
  parentAccountId: { type: 'string', required: false },
  accountType: { type: 'string', required: true },
  industry: { type: 'string', required: true },
  subIndustry: { type: 'string', required: false },
  annualRevenue: { type: 'number', required: true },
  numberOfEmployees: { type: 'number', required: true },
  ownership: { type: 'string', required: true },
  website: { type: 'string', required: false },
  phone: { type: 'string', required: true },
  fax: { type: 'string', required: false },
  billingAddress: { type: 'object', required: true, properties: {
    street1: { type: 'string' },
    street2: { type: 'string' },
    city: { type: 'string' },
    stateProvince: { type: 'string' },
    postalCode: { type: 'string' },
    country: { type: 'string' }
  }},
  shippingAddress: { type: 'object', required: true, properties: {
    street1: { type: 'string' },
    street2: { type: 'string' },
    city: { type: 'string' },
    stateProvince: { type: 'string' },
    postalCode: { type: 'string' },
    country: { type: 'string' }
  }},
  primaryContact: { type: 'object', required: true, properties: {
    contactId: { type: 'string' },
    fullName: { type: 'string' },
    title: { type: 'string' },
    email: { type: 'string' },
    phone: { type: 'string' }
  }},
  accountRating: { type: 'string', required: true },
  creditLimit: { type: 'number', required: false },
  paymentTerms: { type: 'string', required: true },
  preferredContactMethod: { type: 'string', required: true },
  ownerId: { type: 'string', required: true },
  ownerName: { type: 'string', required: true },
  teamId: { type: 'string', required: false },
  territory: { type: 'string', required: true },
  createdOn: { type: 'string', required: true },
  modifiedOn: { type: 'string', required: true },
  lastInteractionDate: { type: 'string', required: false },
  description: { type: 'string', required: false }
};

// Salesforce Service Cloud Support Ticket Schema
const supportTicketSchema = {
  caseId: { type: 'string', required: true },
  caseNumber: { type: 'string', required: true },
  subject: { type: 'string', required: true },
  description: { type: 'string', required: true },
  status: { type: 'string', required: true },
  priority: { type: 'string', required: true },
  severity: { type: 'string', required: true },
  type: { type: 'string', required: true },
  origin: { type: 'string', required: true },
  reason: { type: 'string', required: false },
  contactId: { type: 'string', required: true },
  contactName: { type: 'string', required: true },
  contactEmail: { type: 'string', required: true },
  contactPhone: { type: 'string', required: false },
  accountId: { type: 'string', required: true },
  accountName: { type: 'string', required: true },
  productId: { type: 'string', required: false },
  productName: { type: 'string', required: false },
  ownerId: { type: 'string', required: true },
  ownerName: { type: 'string', required: true },
  createdDate: { type: 'string', required: true },
  closedDate: { type: 'string', required: false },
  firstResponseDate: { type: 'string', required: false },
  firstResponseSLA: { type: 'number', required: true },
  resolutionSLA: { type: 'number', required: true },
  escalated: { type: 'boolean', required: true },
  escalationDate: { type: 'string', required: false },
  resolution: { type: 'string', required: false },
  comments: { type: 'array', required: false, items: {
    commentId: { type: 'string' },
    author: { type: 'string' },
    timestamp: { type: 'string' },
    text: { type: 'string' },
    isPublic: { type: 'boolean' }
  }},
  satisfaction: { type: 'object', required: false, properties: {
    score: { type: 'number' },
    feedback: { type: 'string' },
    surveyDate: { type: 'string' }
  }}
};

// Customer Lifetime Value Schema
const customerLifetimeValueSchema = {
  customerId: { type: 'string', required: true },
  customerName: { type: 'string', required: true },
  segment: { type: 'string', required: true },
  acquisitionDate: { type: 'string', required: true },
  acquisitionChannel: { type: 'string', required: true },
  acquisitionCost: { type: 'number', required: true },
  metrics: { type: 'object', required: true, properties: {
    totalRevenue: { type: 'number' },
    totalOrders: { type: 'number' },
    averageOrderValue: { type: 'number' },
    totalProfit: { type: 'number' },
    profitMargin: { type: 'number' },
    retentionRate: { type: 'number' },
    churnProbability: { type: 'number' }
  }},
  ltv: { type: 'object', required: true, properties: {
    currentLTV: { type: 'number' },
    predictedLTV: { type: 'number' },
    ltvCACRatio: { type: 'number' },
    paybackPeriod: { type: 'number' },
    timeHorizon: { type: 'string' }
  }},
  engagement: { type: 'object', required: true, properties: {
    lastPurchaseDate: { type: 'string' },
    daysSinceLastPurchase: { type: 'number' },
    averageDaysBetweenPurchases: { type: 'number' },
    emailOpenRate: { type: 'number' },
    emailClickRate: { type: 'number' },
    websiteVisits: { type: 'number' },
    supportTickets: { type: 'number' },
    npsScore: { type: 'number' }
  }},
  crossSell: { type: 'array', required: false, items: {
    productCategory: { type: 'string' },
    probability: { type: 'number' },
    potentialRevenue: { type: 'number' }
  }},
  churnRisk: { type: 'object', required: true, properties: {
    score: { type: 'number' },
    factors: { type: 'array' },
    mitigationActions: { type: 'array' }
  }}
};

/**
 * Generate Salesforce Leads
 */
export async function generateLeads(count: number = 100) {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  console.log(`Generating ${count} Salesforce leads...`);

  const result = await synth.generateStructured({
    count,
    schema: leadSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} leads in ${result.metadata.duration}ms`);
  console.log('Sample lead:', result.data[0]);

  return result;
}

/**
 * Generate Sales Pipeline (Opportunities)
 */
export async function generateOpportunities(count: number = 75) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} sales opportunities...`);

  const result = await synth.generateStructured({
    count,
    schema: opportunitySchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} opportunities in ${result.metadata.duration}ms`);
  console.log('Sample opportunity:', result.data[0]);

  return result;
}

/**
 * Generate HubSpot Contact Interactions (time-series)
 */
export async function generateContactInteractions(count: number = 500) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} contact interactions...`);

  const result = await synth.generateEvents({
    count,
    eventTypes: ['email', 'call', 'meeting', 'chat', 'website_visit', 'form_submission', 'social_media'],
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 90 * 24 * 60 * 60 * 1000), // 90 days ago
      end: new Date()
    }
  });

  console.log(`Generated ${result.data.length} interactions in ${result.metadata.duration}ms`);
  console.log('Sample interaction:', result.data[0]);

  return result;
}

/**
 * Generate Microsoft Dynamics 365 Accounts
 */
export async function generateAccounts(count: number = 50) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} CRM accounts...`);

  const result = await synth.generateStructured({
    count,
    schema: accountSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} accounts in ${result.metadata.duration}ms`);
  console.log('Sample account:', result.data[0]);

  return result;
}

/**
 * Generate Salesforce Service Cloud Support Tickets
 */
export async function generateSupportTickets(count: number = 200) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} support tickets...`);

  const result = await synth.generateStructured({
    count,
    schema: supportTicketSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} tickets in ${result.metadata.duration}ms`);
  console.log('Sample ticket:', result.data[0]);

  return result;
}

/**
 * Generate Customer Lifetime Value Analysis
 */
export async function generateCustomerLTV(count: number = 100) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} customer LTV records...`);

  const result = await synth.generateStructured({
    count,
    schema: customerLifetimeValueSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} LTV records in ${result.metadata.duration}ms`);
  console.log('Sample LTV:', result.data[0]);

  return result;
}

/**
 * Simulate complete sales funnel with conversion metrics
 */
export async function simulateSalesFunnel() {
  const synth = createSynth({
    provider: 'gemini',
    cacheStrategy: 'memory'
  });

  console.log('Simulating complete sales funnel...');
  console.time('Sales funnel simulation');

  // Generate funnel stages in sequence to maintain conversion logic
  const leads = await generateLeads(1000);
  const qualifiedLeadCount = Math.floor(leads.data.length * 0.4); // 40% qualification rate

  const opportunities = await generateOpportunities(qualifiedLeadCount);
  const wonOpportunityCount = Math.floor(opportunities.data.length * 0.25); // 25% win rate

  const accounts = await generateAccounts(wonOpportunityCount);

  console.timeEnd('Sales funnel simulation');

  const metrics = {
    leads: leads.data.length,
    qualifiedLeads: qualifiedLeadCount,
    opportunities: opportunities.data.length,
    wonDeals: wonOpportunityCount,
    accounts: accounts.data.length,
    conversionRates: {
      leadToQualified: (qualifiedLeadCount / leads.data.length * 100).toFixed(2) + '%',
      qualifiedToOpportunity: '100%', // By design
      opportunityToWon: (wonOpportunityCount / opportunities.data.length * 100).toFixed(2) + '%',
      leadToCustomer: (accounts.data.length / leads.data.length * 100).toFixed(2) + '%'
    },
    totalPipelineValue: opportunities.data.reduce((sum: number, opp: any) => sum + (opp.amount || 0), 0),
    averageDealSize: opportunities.data.reduce((sum: number, opp: any) => sum + (opp.amount || 0), 0) / opportunities.data.length
  };

  console.log('Sales Funnel Metrics:', metrics);

  return {
    leads: leads.data,
    opportunities: opportunities.data,
    accounts: accounts.data,
    metrics
  };
}

/**
 * Generate complete CRM dataset in parallel
 */
export async function generateCompleteCRMDataset() {
  const synth = createSynth({
    provider: 'gemini',
    cacheStrategy: 'memory'
  });

  console.log('Generating complete CRM dataset in parallel...');
  console.time('Total CRM generation');

  const [leads, opportunities, interactions, accounts, tickets, ltv] =
    await Promise.all([
      generateLeads(100),
      generateOpportunities(50),
      generateContactInteractions(300),
      generateAccounts(30),
      generateSupportTickets(100),
      generateCustomerLTV(50)
    ]);

  console.timeEnd('Total CRM generation');

  return {
    leads: leads.data,
    opportunities: opportunities.data,
    interactions: interactions.data,
    accounts: accounts.data,
    supportTickets: tickets.data,
    customerLTV: ltv.data,
    metadata: {
      totalRecords: leads.data.length + opportunities.data.length +
                   interactions.data.length + accounts.data.length +
                   tickets.data.length + ltv.data.length,
      generatedAt: new Date().toISOString()
    }
  };
}

/**
 * Stream CRM interactions for real-time analysis
 */
export async function streamCRMInteractions(duration: number = 3600) {
  const synth = createSynth({
    provider: 'gemini',
    streaming: true
  });

  console.log(`Streaming CRM interactions for ${duration} seconds...`);

  const endTime = Date.now() + (duration * 1000);
  let interactionCount = 0;

  while (Date.now() < endTime) {
    for await (const interaction of synth.generateStream('events', {
      count: 10,
      eventTypes: ['email', 'call', 'meeting', 'chat'],
      distribution: 'poisson'
    })) {
      interactionCount++;
      console.log(`[${new Date().toISOString()}] Interaction ${interactionCount}:`, interaction);

      // Simulate real-time processing delay
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
  }

  console.log(`Completed streaming ${interactionCount} interactions`);
}

// Example usage
async function runCRMExamples() {
  console.log('=== CRM Data Generation Examples ===\n');

  // Example 1: Lead Generation
  console.log('1. Lead Generation (Salesforce)');
  await generateLeads(10);

  // Example 2: Sales Pipeline
  console.log('\n2. Sales Pipeline (Opportunities)');
  await generateOpportunities(10);

  // Example 3: Contact Interactions
  console.log('\n3. Contact Interactions (HubSpot)');
  await generateContactInteractions(50);

  // Example 4: Account Management
  console.log('\n4. Account Management (Dynamics 365)');
  await generateAccounts(5);

  // Example 5: Support Tickets
  console.log('\n5. Support Tickets (Service Cloud)');
  await generateSupportTickets(20);

  // Example 6: Customer LTV
  console.log('\n6. Customer Lifetime Value');
  await generateCustomerLTV(10);

  // Example 7: Sales Funnel Simulation
  console.log('\n7. Complete Sales Funnel Simulation');
  await simulateSalesFunnel();

  // Example 8: Complete CRM dataset
  console.log('\n8. Complete CRM Dataset (Parallel)');
  const completeDataset = await generateCompleteCRMDataset();
  console.log('Total records generated:', completeDataset.metadata.totalRecords);
}

// Uncomment to run
// runCRMExamples().catch(console.error);

export default {
  generateLeads,
  generateOpportunities,
  generateContactInteractions,
  generateAccounts,
  generateSupportTickets,
  generateCustomerLTV,
  simulateSalesFunnel,
  generateCompleteCRMDataset,
  streamCRMInteractions
};
