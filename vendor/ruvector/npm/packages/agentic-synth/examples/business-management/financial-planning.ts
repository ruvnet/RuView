/**
 * Financial Planning and Analysis Data Generation
 * Simulates enterprise financial systems, budgeting, forecasting, and reporting
 */

import { createSynth } from '../../src/index.js';

// Budget Planning Schema
const budgetPlanningSchema = {
  budgetId: { type: 'string', required: true },
  fiscalYear: { type: 'number', required: true },
  fiscalPeriod: { type: 'string', required: true },
  organization: { type: 'object', required: true, properties: {
    companyCode: { type: 'string' },
    businessUnit: { type: 'string' },
    department: { type: 'string' },
    costCenter: { type: 'string' },
    profitCenter: { type: 'string' }
  }},
  budgetType: { type: 'string', required: true },
  currency: { type: 'string', required: true },
  version: { type: 'string', required: true },
  status: { type: 'string', required: true },
  revenue: { type: 'object', required: true, properties: {
    productSales: { type: 'number' },
    serviceSales: { type: 'number' },
    subscriptionRevenue: { type: 'number' },
    otherRevenue: { type: 'number' },
    totalRevenue: { type: 'number' }
  }},
  costOfGoodsSold: { type: 'object', required: true, properties: {
    materials: { type: 'number' },
    labor: { type: 'number' },
    overhead: { type: 'number' },
    totalCOGS: { type: 'number' }
  }},
  operatingExpenses: { type: 'object', required: true, properties: {
    salaries: { type: 'number' },
    benefits: { type: 'number' },
    rent: { type: 'number' },
    utilities: { type: 'number' },
    marketing: { type: 'number' },
    travelExpenses: { type: 'number' },
    professionalFees: { type: 'number' },
    technology: { type: 'number' },
    depreciation: { type: 'number' },
    other: { type: 'number' },
    totalOpEx: { type: 'number' }
  }},
  capitalExpenditure: { type: 'object', required: false, properties: {
    equipment: { type: 'number' },
    infrastructure: { type: 'number' },
    technology: { type: 'number' },
    totalCapEx: { type: 'number' }
  }},
  calculations: { type: 'object', required: true, properties: {
    grossProfit: { type: 'number' },
    grossMargin: { type: 'number' },
    operatingIncome: { type: 'number' },
    operatingMargin: { type: 'number' },
    ebitda: { type: 'number' },
    netIncome: { type: 'number' },
    netMargin: { type: 'number' }
  }},
  owners: { type: 'object', required: true, properties: {
    preparedBy: { type: 'string' },
    reviewedBy: { type: 'string' },
    approvedBy: { type: 'string' }
  }},
  createdDate: { type: 'string', required: true },
  lastModifiedDate: { type: 'string', required: true }
};

// Revenue Forecasting Schema
const revenueForecastSchema = {
  forecastId: { type: 'string', required: true },
  forecastDate: { type: 'string', required: true },
  forecastPeriod: { type: 'object', required: true, properties: {
    startDate: { type: 'string' },
    endDate: { type: 'string' },
    periodType: { type: 'string' }
  }},
  businessUnit: { type: 'string', required: true },
  region: { type: 'string', required: true },
  currency: { type: 'string', required: true },
  forecastType: { type: 'string', required: true },
  methodology: { type: 'string', required: true },
  confidence: { type: 'number', required: true },
  revenueStreams: { type: 'array', required: true, items: {
    streamId: { type: 'string' },
    streamName: { type: 'string' },
    category: { type: 'string' },
    forecast: { type: 'object', properties: {
      conservative: { type: 'number' },
      expected: { type: 'number' },
      optimistic: { type: 'number' }
    }},
    assumptions: { type: 'array' },
    drivers: { type: 'array' },
    risks: { type: 'array' }
  }},
  totals: { type: 'object', required: true, properties: {
    conservativeTotal: { type: 'number' },
    expectedTotal: { type: 'number' },
    optimisticTotal: { type: 'number' }
  }},
  comparisonMetrics: { type: 'object', required: true, properties: {
    priorYearActual: { type: 'number' },
    yoyGrowth: { type: 'number' },
    budgetVariance: { type: 'number' },
    lastForecastVariance: { type: 'number' }
  }},
  modelInputs: { type: 'object', required: false, properties: {
    marketGrowthRate: { type: 'number' },
    pricingAssumptions: { type: 'number' },
    volumeAssumptions: { type: 'number' },
    marketShareTarget: { type: 'number' },
    newCustomerAcquisition: { type: 'number' },
    churnRate: { type: 'number' }
  }},
  preparedBy: { type: 'string', required: true },
  approvedBy: { type: 'string', required: false },
  lastUpdated: { type: 'string', required: true }
};

// Expense Tracking Schema
const expenseTrackingSchema = {
  expenseId: { type: 'string', required: true },
  transactionDate: { type: 'string', required: true },
  postingDate: { type: 'string', required: true },
  fiscalPeriod: { type: 'string', required: true },
  organization: { type: 'object', required: true, properties: {
    companyCode: { type: 'string' },
    businessUnit: { type: 'string' },
    department: { type: 'string' },
    costCenter: { type: 'string' }
  }},
  expenseCategory: { type: 'string', required: true },
  expenseType: { type: 'string', required: true },
  glAccount: { type: 'string', required: true },
  accountDescription: { type: 'string', required: true },
  amount: { type: 'number', required: true },
  currency: { type: 'string', required: true },
  vendor: { type: 'object', required: false, properties: {
    vendorId: { type: 'string' },
    vendorName: { type: 'string' }
  }},
  budgetInfo: { type: 'object', required: true, properties: {
    budgetedAmount: { type: 'number' },
    spentToDate: { type: 'number' },
    remainingBudget: { type: 'number' },
    variance: { type: 'number' },
    variancePercent: { type: 'number' }
  }},
  approval: { type: 'object', required: true, properties: {
    requestedBy: { type: 'string' },
    approvedBy: { type: 'string' },
    approvalDate: { type: 'string' },
    status: { type: 'string' }
  }},
  project: { type: 'object', required: false, properties: {
    projectId: { type: 'string' },
    projectName: { type: 'string' },
    workPackage: { type: 'string' }
  }},
  description: { type: 'string', required: true },
  reference: { type: 'string', required: false },
  tags: { type: 'array', required: false }
};

// Cash Flow Projection Schema
const cashFlowProjectionSchema = {
  projectionId: { type: 'string', required: true },
  projectionDate: { type: 'string', required: true },
  period: { type: 'object', required: true, properties: {
    startDate: { type: 'string' },
    endDate: { type: 'string' },
    frequency: { type: 'string' }
  }},
  currency: { type: 'string', required: true },
  openingBalance: { type: 'number', required: true },
  operatingActivities: { type: 'object', required: true, properties: {
    cashFromCustomers: { type: 'number' },
    cashToSuppliers: { type: 'number' },
    cashToEmployees: { type: 'number' },
    operatingExpenses: { type: 'number' },
    interestPaid: { type: 'number' },
    taxesPaid: { type: 'number' },
    netOperatingCashFlow: { type: 'number' }
  }},
  investingActivities: { type: 'object', required: true, properties: {
    capitalExpenditures: { type: 'number' },
    assetPurchases: { type: 'number' },
    assetSales: { type: 'number' },
    investments: { type: 'number' },
    netInvestingCashFlow: { type: 'number' }
  }},
  financingActivities: { type: 'object', required: true, properties: {
    debtProceeds: { type: 'number' },
    debtRepayments: { type: 'number' },
    equityIssuance: { type: 'number' },
    dividendsPaid: { type: 'number' },
    netFinancingCashFlow: { type: 'number' }
  }},
  netCashFlow: { type: 'number', required: true },
  closingBalance: { type: 'number', required: true },
  metrics: { type: 'object', required: true, properties: {
    cashConversionCycle: { type: 'number' },
    daysReceivablesOutstanding: { type: 'number' },
    daysPayablesOutstanding: { type: 'number' },
    daysInventoryOutstanding: { type: 'number' },
    operatingCashFlowRatio: { type: 'number' }
  }},
  scenarios: { type: 'object', required: false, properties: {
    baseline: { type: 'number' },
    bestCase: { type: 'number' },
    worstCase: { type: 'number' }
  }},
  assumptions: { type: 'array', required: false },
  risks: { type: 'array', required: false }
};

// Profit & Loss Statement Schema
const profitLossSchema = {
  statementId: { type: 'string', required: true },
  statementDate: { type: 'string', required: true },
  period: { type: 'object', required: true, properties: {
    startDate: { type: 'string' },
    endDate: { type: 'string' },
    fiscalYear: { type: 'number' },
    fiscalQuarter: { type: 'string' },
    fiscalMonth: { type: 'string' }
  }},
  organization: { type: 'object', required: true, properties: {
    companyCode: { type: 'string' },
    companyName: { type: 'string' },
    businessUnit: { type: 'string' },
    segment: { type: 'string' }
  }},
  currency: { type: 'string', required: true },
  revenue: { type: 'object', required: true, properties: {
    productRevenue: { type: 'number' },
    serviceRevenue: { type: 'number' },
    otherRevenue: { type: 'number' },
    totalRevenue: { type: 'number' }
  }},
  costOfRevenue: { type: 'object', required: true, properties: {
    directMaterials: { type: 'number' },
    directLabor: { type: 'number' },
    manufacturingOverhead: { type: 'number' },
    totalCostOfRevenue: { type: 'number' }
  }},
  grossProfit: { type: 'number', required: true },
  grossMargin: { type: 'number', required: true },
  operatingExpenses: { type: 'object', required: true, properties: {
    salesAndMarketing: { type: 'number' },
    researchAndDevelopment: { type: 'number' },
    generalAndAdministrative: { type: 'number' },
    totalOperatingExpenses: { type: 'number' }
  }},
  operatingIncome: { type: 'number', required: true },
  operatingMargin: { type: 'number', required: true },
  nonOperating: { type: 'object', required: false, properties: {
    interestIncome: { type: 'number' },
    interestExpense: { type: 'number' },
    otherIncome: { type: 'number' },
    otherExpenses: { type: 'number' },
    netNonOperating: { type: 'number' }
  }},
  incomeBeforeTax: { type: 'number', required: true },
  incomeTaxExpense: { type: 'number', required: true },
  effectiveTaxRate: { type: 'number', required: true },
  netIncome: { type: 'number', required: true },
  netMargin: { type: 'number', required: true },
  earningsPerShare: { type: 'object', required: false, properties: {
    basic: { type: 'number' },
    diluted: { type: 'number' }
  }},
  comparisonPeriod: { type: 'object', required: false, properties: {
    priorPeriodRevenue: { type: 'number' },
    priorPeriodNetIncome: { type: 'number' },
    revenueGrowth: { type: 'number' },
    incomeGrowth: { type: 'number' }
  }}
};

// Balance Sheet Schema
const balanceSheetSchema = {
  statementId: { type: 'string', required: true },
  asOfDate: { type: 'string', required: true },
  fiscalPeriod: { type: 'string', required: true },
  organization: { type: 'object', required: true, properties: {
    companyCode: { type: 'string' },
    companyName: { type: 'string' }
  }},
  currency: { type: 'string', required: true },
  assets: { type: 'object', required: true, properties: {
    currentAssets: { type: 'object', properties: {
      cashAndEquivalents: { type: 'number' },
      shortTermInvestments: { type: 'number' },
      accountsReceivable: { type: 'number' },
      inventory: { type: 'number' },
      prepaidExpenses: { type: 'number' },
      otherCurrentAssets: { type: 'number' },
      totalCurrentAssets: { type: 'number' }
    }},
    nonCurrentAssets: { type: 'object', properties: {
      propertyPlantEquipment: { type: 'number' },
      accumulatedDepreciation: { type: 'number' },
      netPPE: { type: 'number' },
      intangibleAssets: { type: 'number' },
      goodwill: { type: 'number' },
      longTermInvestments: { type: 'number' },
      otherNonCurrentAssets: { type: 'number' },
      totalNonCurrentAssets: { type: 'number' }
    }},
    totalAssets: { type: 'number' }
  }},
  liabilities: { type: 'object', required: true, properties: {
    currentLiabilities: { type: 'object', properties: {
      accountsPayable: { type: 'number' },
      accruedExpenses: { type: 'number' },
      shortTermDebt: { type: 'number' },
      currentPortionLongTermDebt: { type: 'number' },
      deferredRevenue: { type: 'number' },
      otherCurrentLiabilities: { type: 'number' },
      totalCurrentLiabilities: { type: 'number' }
    }},
    nonCurrentLiabilities: { type: 'object', properties: {
      longTermDebt: { type: 'number' },
      deferredTaxLiabilities: { type: 'number' },
      pensionObligations: { type: 'number' },
      otherNonCurrentLiabilities: { type: 'number' },
      totalNonCurrentLiabilities: { type: 'number' }
    }},
    totalLiabilities: { type: 'number' }
  }},
  equity: { type: 'object', required: true, properties: {
    commonStock: { type: 'number' },
    preferredStock: { type: 'number' },
    additionalPaidInCapital: { type: 'number' },
    retainedEarnings: { type: 'number' },
    treasuryStock: { type: 'number' },
    accumulatedOtherComprehensiveIncome: { type: 'number' },
    totalEquity: { type: 'number' }
  }},
  totalLiabilitiesAndEquity: { type: 'number', required: true },
  ratios: { type: 'object', required: true, properties: {
    currentRatio: { type: 'number' },
    quickRatio: { type: 'number' },
    debtToEquity: { type: 'number' },
    workingCapital: { type: 'number' },
    returnOnAssets: { type: 'number' },
    returnOnEquity: { type: 'number' }
  }}
};

// KPI Dashboard Data Schema
const kpiDashboardSchema = {
  dashboardId: { type: 'string', required: true },
  timestamp: { type: 'string', required: true },
  period: { type: 'string', required: true },
  businessUnit: { type: 'string', required: true },
  financialKPIs: { type: 'object', required: true, properties: {
    revenue: { type: 'object', properties: {
      value: { type: 'number' },
      target: { type: 'number' },
      variance: { type: 'number' },
      trend: { type: 'string' }
    }},
    profitMargin: { type: 'object', properties: {
      value: { type: 'number' },
      target: { type: 'number' },
      variance: { type: 'number' },
      trend: { type: 'string' }
    }},
    ebitdaMargin: { type: 'object', properties: {
      value: { type: 'number' },
      target: { type: 'number' },
      variance: { type: 'number' },
      trend: { type: 'string' }
    }},
    returnOnInvestment: { type: 'object', properties: {
      value: { type: 'number' },
      target: { type: 'number' },
      variance: { type: 'number' },
      trend: { type: 'string' }
    }},
    cashFlowFromOperations: { type: 'object', properties: {
      value: { type: 'number' },
      target: { type: 'number' },
      variance: { type: 'number' },
      trend: { type: 'string' }
    }}
  }},
  operationalKPIs: { type: 'object', required: true, properties: {
    revenuePerEmployee: { type: 'number' },
    operatingExpenseRatio: { type: 'number' },
    inventoryTurnover: { type: 'number' },
    daysInventoryOutstanding: { type: 'number' },
    assetTurnover: { type: 'number' }
  }},
  liquidityKPIs: { type: 'object', required: true, properties: {
    currentRatio: { type: 'number' },
    quickRatio: { type: 'number' },
    cashRatio: { type: 'number' },
    workingCapital: { type: 'number' },
    daysWorkingCapital: { type: 'number' }
  }},
  leverageKPIs: { type: 'object', required: true, properties: {
    debtToEquity: { type: 'number' },
    debtToAssets: { type: 'number' },
    interestCoverageRatio: { type: 'number' },
    debtServiceCoverageRatio: { type: 'number' }
  }},
  efficiencyKPIs: { type: 'object', required: true, properties: {
    daysReceivablesOutstanding: { type: 'number' },
    daysPayablesOutstanding: { type: 'number' },
    cashConversionCycle: { type: 'number' },
    burnRate: { type: 'number' },
    runwayMonths: { type: 'number' }
  }},
  alerts: { type: 'array', required: false, items: {
    kpiName: { type: 'string' },
    severity: { type: 'string' },
    message: { type: 'string' },
    threshold: { type: 'number' },
    actualValue: { type: 'number' }
  }}
};

/**
 * Generate Budget Planning Data
 */
export async function generateBudgetPlans(count: number = 50) {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  console.log(`Generating ${count} budget plans...`);

  const result = await synth.generateStructured({
    count,
    schema: budgetPlanningSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} budgets in ${result.metadata.duration}ms`);
  console.log('Sample budget:', result.data[0]);

  return result;
}

/**
 * Generate Revenue Forecasts
 */
export async function generateRevenueForecasts(count: number = 25) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} revenue forecasts...`);

  const result = await synth.generateStructured({
    count,
    schema: revenueForecastSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} forecasts in ${result.metadata.duration}ms`);
  console.log('Sample forecast:', result.data[0]);

  return result;
}

/**
 * Generate Expense Tracking Data (time-series)
 */
export async function generateExpenseTracking(count: number = 500) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} expense records...`);

  const result = await synth.generateStructured({
    count,
    schema: expenseTrackingSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} expenses in ${result.metadata.duration}ms`);
  console.log('Sample expense:', result.data[0]);

  return result;
}

/**
 * Generate Cash Flow Projections
 */
export async function generateCashFlowProjections(count: number = 12) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} cash flow projections...`);

  const result = await synth.generateStructured({
    count,
    schema: cashFlowProjectionSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} projections in ${result.metadata.duration}ms`);
  console.log('Sample projection:', result.data[0]);

  return result;
}

/**
 * Generate P&L Statements
 */
export async function generateProfitLossStatements(count: number = 12) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} P&L statements...`);

  const result = await synth.generateStructured({
    count,
    schema: profitLossSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} statements in ${result.metadata.duration}ms`);
  console.log('Sample P&L:', result.data[0]);

  return result;
}

/**
 * Generate Balance Sheets
 */
export async function generateBalanceSheets(count: number = 12) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} balance sheets...`);

  const result = await synth.generateStructured({
    count,
    schema: balanceSheetSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} balance sheets in ${result.metadata.duration}ms`);
  console.log('Sample balance sheet:', result.data[0]);

  return result;
}

/**
 * Generate KPI Dashboard Data (time-series)
 */
export async function generateKPIDashboards(count: number = 365) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} KPI dashboard snapshots...`);

  const result = await synth.generateTimeSeries({
    count,
    interval: '1d',
    metrics: ['revenue', 'expenses', 'profitMargin', 'cashFlow'],
    trend: 'up',
    seasonality: true
  });

  console.log(`Generated ${result.data.length} KPI snapshots in ${result.metadata.duration}ms`);
  console.log('Sample KPI:', result.data[0]);

  return result;
}

/**
 * Generate complete financial dataset in parallel
 */
export async function generateCompleteFinancialDataset() {
  const synth = createSynth({
    provider: 'gemini',
    cacheStrategy: 'memory'
  });

  console.log('Generating complete financial dataset in parallel...');
  console.time('Total financial generation');

  const [budgets, forecasts, expenses, cashFlow, profitLoss, balanceSheets, kpis] =
    await Promise.all([
      generateBudgetPlans(20),
      generateRevenueForecasts(12),
      generateExpenseTracking(200),
      generateCashFlowProjections(12),
      generateProfitLossStatements(12),
      generateBalanceSheets(12),
      generateKPIDashboards(90)
    ]);

  console.timeEnd('Total financial generation');

  return {
    budgets: budgets.data,
    revenueForecasts: forecasts.data,
    expenses: expenses.data,
    cashFlowProjections: cashFlow.data,
    profitLossStatements: profitLoss.data,
    balanceSheets: balanceSheets.data,
    kpiDashboards: kpis.data,
    metadata: {
      totalRecords: budgets.data.length + forecasts.data.length +
                   expenses.data.length + cashFlow.data.length +
                   profitLoss.data.length + balanceSheets.data.length +
                   kpis.data.length,
      generatedAt: new Date().toISOString()
    }
  };
}

// Example usage
async function runFinancialExamples() {
  console.log('=== Financial Planning Data Generation Examples ===\n');

  // Example 1: Budget Planning
  console.log('1. Budget Planning');
  await generateBudgetPlans(5);

  // Example 2: Revenue Forecasting
  console.log('\n2. Revenue Forecasting');
  await generateRevenueForecasts(5);

  // Example 3: Expense Tracking
  console.log('\n3. Expense Tracking');
  await generateExpenseTracking(25);

  // Example 4: Cash Flow Projections
  console.log('\n4. Cash Flow Projections');
  await generateCashFlowProjections(12);

  // Example 5: P&L Statements
  console.log('\n5. Profit & Loss Statements');
  await generateProfitLossStatements(4);

  // Example 6: Balance Sheets
  console.log('\n6. Balance Sheets');
  await generateBalanceSheets(4);

  // Example 7: KPI Dashboards
  console.log('\n7. KPI Dashboards');
  await generateKPIDashboards(30);

  // Example 8: Complete financial dataset
  console.log('\n8. Complete Financial Dataset (Parallel)');
  const completeDataset = await generateCompleteFinancialDataset();
  console.log('Total records generated:', completeDataset.metadata.totalRecords);
}

// Uncomment to run
// runFinancialExamples().catch(console.error);

export default {
  generateBudgetPlans,
  generateRevenueForecasts,
  generateExpenseTracking,
  generateCashFlowProjections,
  generateProfitLossStatements,
  generateBalanceSheets,
  generateKPIDashboards,
  generateCompleteFinancialDataset
};
