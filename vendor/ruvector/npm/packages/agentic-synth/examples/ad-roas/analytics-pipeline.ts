/**
 * Marketing Analytics Pipeline Examples
 *
 * Generates analytics data including:
 * - Attribution modeling data
 * - LTV (Lifetime Value) calculation datasets
 * - Funnel analysis data
 * - Seasonal trend simulation
 */

import { AgenticSynth, createSynth } from '../../src/index.js';

// Example 1: Attribution modeling data
async function generateAttributionModels() {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  const attributionSchema = {
    modelId: { type: 'string', required: true },
    modelType: { type: 'string', required: true },
    analysisDate: { type: 'string', required: true },
    timeWindow: { type: 'string', required: true },
    totalConversions: { type: 'number', required: true },
    totalRevenue: { type: 'number', required: true },
    channelAttribution: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          channel: { type: 'string' },
          touchpoints: { type: 'number' },
          firstTouchConversions: { type: 'number' },
          lastTouchConversions: { type: 'number' },
          linearConversions: { type: 'number' },
          timeDecayConversions: { type: 'number' },
          positionBasedConversions: { type: 'number' },
          algorithmicConversions: { type: 'number' },
          attributedRevenue: { type: 'number' },
          attributedSpend: { type: 'number' },
          roas: { type: 'number' },
          efficiency: { type: 'number' }
        }
      }
    },
    crossChannelInteractions: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          path: { type: 'array' },
          conversions: { type: 'number' },
          revenue: { type: 'number' },
          avgPathLength: { type: 'number' },
          avgTimeToConversion: { type: 'number' }
        }
      }
    },
    insights: {
      type: 'object',
      required: true,
      properties: {
        topPerformingChannels: { type: 'array' },
        undervaluedChannels: { type: 'array' },
        overvaluedChannels: { type: 'array' },
        recommendedBudgetShift: { type: 'object' }
      }
    }
  };

  const result = await synth.generateStructured({
    count: 30,
    schema: attributionSchema,
    constraints: {
      modelType: [
        'first_touch',
        'last_touch',
        'linear',
        'time_decay',
        'position_based',
        'data_driven'
      ],
      timeWindow: ['7_days', '14_days', '30_days', '60_days', '90_days'],
      totalConversions: { min: 100, max: 10000 },
      totalRevenue: { min: 10000, max: 5000000 },
      channelAttribution: { minLength: 4, maxLength: 10 }
    }
  });

  console.log('Attribution Model Data:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 2: LTV (Lifetime Value) calculations
async function generateLTVAnalysis() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const ltvSchema = {
    cohortId: { type: 'string', required: true },
    cohortName: { type: 'string', required: true },
    acquisitionChannel: { type: 'string', required: true },
    acquisitionDate: { type: 'string', required: true },
    cohortSize: { type: 'number', required: true },
    metrics: {
      type: 'object',
      required: true,
      properties: {
        avgFirstPurchase: { type: 'number' },
        avgOrderValue: { type: 'number' },
        purchaseFrequency: { type: 'number' },
        customerLifespan: { type: 'number' },
        retentionRate: { type: 'number' },
        churnRate: { type: 'number' },
        marginPerCustomer: { type: 'number' }
      }
    },
    ltvCalculations: {
      type: 'object',
      required: true,
      properties: {
        historicLTV: { type: 'number' },
        predictiveLTV: { type: 'number' },
        ltv30Days: { type: 'number' },
        ltv90Days: { type: 'number' },
        ltv180Days: { type: 'number' },
        ltv365Days: { type: 'number' },
        ltv3Years: { type: 'number' }
      }
    },
    acquisition: {
      type: 'object',
      required: true,
      properties: {
        cac: { type: 'number' },
        ltvCacRatio: { type: 'number' },
        paybackPeriod: { type: 'number' },
        roi: { type: 'number' }
      }
    },
    revenueByPeriod: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          period: { type: 'number' },
          activeCustomers: { type: 'number' },
          purchases: { type: 'number' },
          revenue: { type: 'number' },
          cumulativeRevenue: { type: 'number' },
          cumulativeLTV: { type: 'number' }
        }
      }
    },
    segments: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          segmentName: { type: 'string' },
          percentage: { type: 'number' },
          avgLTV: { type: 'number' },
          characteristics: { type: 'array' }
        }
      }
    }
  };

  const result = await synth.generateStructured({
    count: 40,
    schema: ltvSchema,
    constraints: {
      acquisitionChannel: [
        'google_ads',
        'facebook_ads',
        'tiktok_ads',
        'organic_search',
        'email',
        'referral',
        'direct'
      ],
      cohortSize: { min: 100, max: 50000 },
      'metrics.customerLifespan': { min: 3, max: 60 },
      'acquisition.ltvCacRatio': { min: 0.5, max: 15.0 },
      revenueByPeriod: { minLength: 12, maxLength: 36 }
    }
  });

  console.log('LTV Analysis Data:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 3: Marketing funnel analysis
async function generateFunnelAnalysis() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const funnelSchema = {
    funnelId: { type: 'string', required: true },
    funnelName: { type: 'string', required: true },
    channel: { type: 'string', required: true },
    campaign: { type: 'string', required: true },
    dateRange: {
      type: 'object',
      required: true,
      properties: {
        start: { type: 'string' },
        end: { type: 'string' }
      }
    },
    stages: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          stageName: { type: 'string' },
          stageOrder: { type: 'number' },
          users: { type: 'number' },
          conversions: { type: 'number' },
          conversionRate: { type: 'number' },
          dropoffRate: { type: 'number' },
          avgTimeInStage: { type: 'number' },
          revenue: { type: 'number' },
          cost: { type: 'number' }
        }
      }
    },
    overallMetrics: {
      type: 'object',
      required: true,
      properties: {
        totalUsers: { type: 'number' },
        totalConversions: { type: 'number' },
        overallConversionRate: { type: 'number' },
        totalRevenue: { type: 'number' },
        totalCost: { type: 'number' },
        roas: { type: 'number' },
        avgTimeToConversion: { type: 'number' }
      }
    },
    dropoffAnalysis: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          fromStage: { type: 'string' },
          toStage: { type: 'string' },
          dropoffCount: { type: 'number' },
          dropoffRate: { type: 'number' },
          reasons: { type: 'array' },
          recoveryOpportunities: { type: 'array' }
        }
      }
    },
    optimization: {
      type: 'object',
      required: true,
      properties: {
        bottlenecks: { type: 'array' },
        recommendations: { type: 'array' },
        expectedImprovement: { type: 'number' },
        priorityActions: { type: 'array' }
      }
    }
  };

  const result = await synth.generateStructured({
    count: 35,
    schema: funnelSchema,
    constraints: {
      channel: ['google_ads', 'facebook_ads', 'tiktok_ads', 'email', 'organic'],
      stages: { minLength: 4, maxLength: 8 },
      'overallMetrics.overallConversionRate': { min: 0.01, max: 0.25 },
      'overallMetrics.roas': { min: 0.5, max: 10.0 }
    }
  });

  console.log('Funnel Analysis Data:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 4: Seasonal trend analysis
async function generateSeasonalTrends() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateTimeSeries({
    count: 365,
    startDate: new Date(Date.now() - 365 * 24 * 60 * 60 * 1000),
    endDate: new Date(),
    interval: '1d',
    metrics: [
      'impressions',
      'clicks',
      'conversions',
      'spend',
      'revenue',
      'roas',
      'ctr',
      'cvr',
      'cpa',
      'seasonality_index',
      'trend_index',
      'day_of_week_effect'
    ],
    trend: 'up',
    seasonality: true,
    noise: 0.12,
    constraints: {
      impressions: { min: 50000, max: 500000 },
      clicks: { min: 500, max: 10000 },
      conversions: { min: 50, max: 1000 },
      spend: { min: 500, max: 20000 },
      revenue: { min: 1000, max: 100000 },
      roas: { min: 1.0, max: 12.0 },
      seasonality_index: { min: 0.5, max: 2.0 }
    }
  });

  console.log('Seasonal Trend Data (daily for 1 year):');
  console.log(result.data.slice(0, 7));
  console.log('Metadata:', result.metadata);

  return result;
}

// Example 5: Predictive analytics
async function generatePredictiveAnalytics() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const predictiveSchema = {
    predictionId: { type: 'string', required: true },
    predictionDate: { type: 'string', required: true },
    predictionHorizon: { type: 'string', required: true },
    model: { type: 'string', required: true },
    historicalPeriod: { type: 'string', required: true },
    predictions: {
      type: 'object',
      required: true,
      properties: {
        expectedSpend: { type: 'number' },
        expectedRevenue: { type: 'number' },
        expectedConversions: { type: 'number' },
        expectedROAS: { type: 'number' },
        expectedCAC: { type: 'number' },
        expectedLTV: { type: 'number' }
      }
    },
    confidenceIntervals: {
      type: 'object',
      required: true,
      properties: {
        spend: { type: 'object' },
        revenue: { type: 'object' },
        conversions: { type: 'object' },
        roas: { type: 'object' }
      }
    },
    scenarios: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          scenarioName: { type: 'string' },
          probability: { type: 'number' },
          predictedROAS: { type: 'number' },
          predictedRevenue: { type: 'number' },
          factors: { type: 'array' }
        }
      }
    },
    riskFactors: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          factor: { type: 'string' },
          impact: { type: 'string' },
          probability: { type: 'number' },
          mitigation: { type: 'string' }
        }
      }
    },
    recommendations: { type: 'array', required: true }
  };

  const result = await synth.generateStructured({
    count: 25,
    schema: predictiveSchema,
    constraints: {
      predictionHorizon: ['7_days', '30_days', '90_days', '180_days', '365_days'],
      model: ['arima', 'prophet', 'lstm', 'random_forest', 'xgboost', 'ensemble'],
      scenarios: { minLength: 3, maxLength: 5 },
      'predictions.expectedROAS': { min: 1.0, max: 15.0 }
    }
  });

  console.log('Predictive Analytics Data:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 6: Channel performance comparison
async function generateChannelComparison() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const comparisonSchema = {
    reportId: { type: 'string', required: true },
    reportDate: { type: 'string', required: true },
    dateRange: {
      type: 'object',
      required: true,
      properties: {
        start: { type: 'string' },
        end: { type: 'string' }
      }
    },
    channels: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          channel: { type: 'string' },
          platform: { type: 'string' },
          campaigns: { type: 'number' },
          impressions: { type: 'number' },
          clicks: { type: 'number' },
          conversions: { type: 'number' },
          spend: { type: 'number' },
          revenue: { type: 'number' },
          ctr: { type: 'number' },
          cvr: { type: 'number' },
          cpc: { type: 'number' },
          cpa: { type: 'number' },
          roas: { type: 'number' },
          marketShare: { type: 'number' },
          efficiency: { type: 'number' },
          scalability: { type: 'string' }
        }
      }
    },
    crossChannelMetrics: {
      type: 'object',
      required: true,
      properties: {
        totalSpend: { type: 'number' },
        totalRevenue: { type: 'number' },
        overallROAS: { type: 'number' },
        channelDiversity: { type: 'number' },
        portfolioRisk: { type: 'number' }
      }
    },
    recommendations: {
      type: 'object',
      required: true,
      properties: {
        scaleUp: { type: 'array' },
        maintain: { type: 'array' },
        optimize: { type: 'array' },
        scaleDown: { type: 'array' },
        budgetReallocation: { type: 'object' }
      }
    }
  };

  const result = await synth.generateStructured({
    count: 30,
    schema: comparisonSchema,
    constraints: {
      channels: { minLength: 4, maxLength: 10 },
      'crossChannelMetrics.overallROAS': { min: 2.0, max: 8.0 }
    }
  });

  console.log('Channel Comparison Data:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 7: Incrementality testing
async function generateIncrementalityTests() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const incrementalitySchema = {
    testId: { type: 'string', required: true },
    testName: { type: 'string', required: true },
    channel: { type: 'string', required: true },
    testType: { type: 'string', required: true },
    startDate: { type: 'string', required: true },
    endDate: { type: 'string', required: true },
    methodology: { type: 'string', required: true },
    testGroup: {
      type: 'object',
      required: true,
      properties: {
        size: { type: 'number' },
        spend: { type: 'number' },
        conversions: { type: 'number' },
        revenue: { type: 'number' }
      }
    },
    controlGroup: {
      type: 'object',
      required: true,
      properties: {
        size: { type: 'number' },
        spend: { type: 'number' },
        conversions: { type: 'number' },
        revenue: { type: 'number' }
      }
    },
    results: {
      type: 'object',
      required: true,
      properties: {
        incrementalConversions: { type: 'number' },
        incrementalRevenue: { type: 'number' },
        incrementalityRate: { type: 'number' },
        trueROAS: { type: 'number' },
        reportedROAS: { type: 'number' },
        overestimation: { type: 'number' },
        statisticalSignificance: { type: 'boolean' },
        confidenceLevel: { type: 'number' }
      }
    },
    insights: {
      type: 'object',
      required: true,
      properties: {
        cannibalizedRevenue: { type: 'number' },
        brandLiftEffect: { type: 'number' },
        spilloverEffect: { type: 'number' },
        recommendedAction: { type: 'string' }
      }
    }
  };

  const result = await synth.generateStructured({
    count: 20,
    schema: incrementalitySchema,
    constraints: {
      channel: ['google_ads', 'facebook_ads', 'tiktok_ads', 'display', 'video'],
      testType: ['geo_holdout', 'user_holdout', 'time_based', 'psm'],
      methodology: ['randomized_control', 'quasi_experimental', 'synthetic_control'],
      'results.incrementalityRate': { min: 0.1, max: 1.0 }
    }
  });

  console.log('Incrementality Test Data:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 8: Marketing mix modeling
async function generateMarketingMixModel() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const mmmSchema = {
    modelId: { type: 'string', required: true },
    modelDate: { type: 'string', required: true },
    timeRange: {
      type: 'object',
      required: true,
      properties: {
        start: { type: 'string' },
        end: { type: 'string' }
      }
    },
    modelMetrics: {
      type: 'object',
      required: true,
      properties: {
        rSquared: { type: 'number' },
        mape: { type: 'number' },
        rmse: { type: 'number' },
        decomposition: { type: 'object' }
      }
    },
    channelContributions: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          channel: { type: 'string' },
          spend: { type: 'number' },
          contribution: { type: 'number' },
          contributionPercent: { type: 'number' },
          roi: { type: 'number' },
          saturationLevel: { type: 'number' },
          carryoverEffect: { type: 'number' },
          elasticity: { type: 'number' }
        }
      }
    },
    optimization: {
      type: 'object',
      required: true,
      properties: {
        currentROI: { type: 'number' },
        optimizedROI: { type: 'number' },
        improvementPotential: { type: 'number' },
        optimalAllocation: { type: 'object' },
        scenarioAnalysis: { type: 'array' }
      }
    },
    externalFactors: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          factor: { type: 'string' },
          impact: { type: 'number' },
          significance: { type: 'string' }
        }
      }
    }
  };

  const result = await synth.generateStructured({
    count: 15,
    schema: mmmSchema,
    constraints: {
      'modelMetrics.rSquared': { min: 0.7, max: 0.95 },
      channelContributions: { minLength: 5, maxLength: 12 },
      'optimization.improvementPotential': { min: 0.05, max: 0.5 }
    }
  });

  console.log('Marketing Mix Model Data:');
  console.log(result.data.slice(0, 1));

  return result;
}

// Example 9: Real-time streaming analytics
async function streamAnalyticsData() {
  const synth = createSynth({
    provider: 'gemini',
    streaming: true
  });

  console.log('Streaming real-time analytics:');

  let count = 0;
  for await (const metric of synth.generateStream('structured', {
    count: 15,
    schema: {
      timestamp: { type: 'string', required: true },
      channel: { type: 'string', required: true },
      impressions: { type: 'number', required: true },
      clicks: { type: 'number', required: true },
      conversions: { type: 'number', required: true },
      spend: { type: 'number', required: true },
      revenue: { type: 'number', required: true },
      roas: { type: 'number', required: true },
      alert: { type: 'string', required: false }
    }
  })) {
    count++;
    console.log(`[${count}] Metric received:`, metric);
  }
}

// Example 10: Comprehensive analytics batch
async function generateAnalyticsBatch() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const analyticsTypes = [
    {
      count: 20,
      schema: {
        type: { type: 'string' },
        metric: { type: 'string' },
        value: { type: 'number' },
        change: { type: 'number' }
      },
      constraints: { type: 'attribution' }
    },
    {
      count: 20,
      schema: {
        type: { type: 'string' },
        metric: { type: 'string' },
        value: { type: 'number' },
        change: { type: 'number' }
      },
      constraints: { type: 'ltv' }
    },
    {
      count: 20,
      schema: {
        type: { type: 'string' },
        metric: { type: 'string' },
        value: { type: 'number' },
        change: { type: 'number' }
      },
      constraints: { type: 'funnel' }
    }
  ];

  const results = await synth.generateBatch('structured', analyticsTypes, 3);

  console.log('Analytics Batch Results:');
  results.forEach((result, i) => {
    const types = ['Attribution', 'LTV', 'Funnel'];
    console.log(`${types[i]}: ${result.metadata.count} metrics in ${result.metadata.duration}ms`);
  });

  return results;
}

// Run all examples
export async function runAnalyticsExamples() {
  console.log('=== Example 1: Attribution Models ===');
  await generateAttributionModels();

  console.log('\n=== Example 2: LTV Analysis ===');
  await generateLTVAnalysis();

  console.log('\n=== Example 3: Funnel Analysis ===');
  await generateFunnelAnalysis();

  console.log('\n=== Example 4: Seasonal Trends ===');
  await generateSeasonalTrends();

  console.log('\n=== Example 5: Predictive Analytics ===');
  await generatePredictiveAnalytics();

  console.log('\n=== Example 6: Channel Comparison ===');
  await generateChannelComparison();

  console.log('\n=== Example 7: Incrementality Tests ===');
  await generateIncrementalityTests();

  console.log('\n=== Example 8: Marketing Mix Model ===');
  await generateMarketingMixModel();

  console.log('\n=== Example 10: Analytics Batch ===');
  await generateAnalyticsBatch();
}

// Export individual functions
export {
  generateAttributionModels,
  generateLTVAnalysis,
  generateFunnelAnalysis,
  generateSeasonalTrends,
  generatePredictiveAnalytics,
  generateChannelComparison,
  generateIncrementalityTests,
  generateMarketingMixModel,
  streamAnalyticsData,
  generateAnalyticsBatch
};

// Uncomment to run
// runAnalyticsExamples().catch(console.error);
