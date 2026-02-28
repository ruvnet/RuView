/**
 * Ad Optimization Simulator
 *
 * Generates optimization scenario data including:
 * - Budget allocation simulations
 * - Bid strategy testing data
 * - Audience segmentation data
 * - Creative performance variations
 * - ROAS optimization scenarios
 */

import { AgenticSynth, createSynth } from '../../src/index.js';

// Example 1: Budget allocation simulation
async function simulateBudgetAllocation() {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  const budgetSchema = {
    scenarioId: { type: 'string', required: true },
    scenarioName: { type: 'string', required: true },
    totalBudget: { type: 'number', required: true },
    timeframe: { type: 'string', required: true },
    allocation: {
      type: 'object',
      required: true,
      properties: {
        googleAds: {
          type: 'object',
          properties: {
            budget: { type: 'number' },
            percentage: { type: 'number' },
            expectedImpressions: { type: 'number' },
            expectedClicks: { type: 'number' },
            expectedConversions: { type: 'number' },
            expectedRevenue: { type: 'number' },
            expectedROAS: { type: 'number' }
          }
        },
        facebookAds: {
          type: 'object',
          properties: {
            budget: { type: 'number' },
            percentage: { type: 'number' },
            expectedImpressions: { type: 'number' },
            expectedClicks: { type: 'number' },
            expectedConversions: { type: 'number' },
            expectedRevenue: { type: 'number' },
            expectedROAS: { type: 'number' }
          }
        },
        tiktokAds: {
          type: 'object',
          properties: {
            budget: { type: 'number' },
            percentage: { type: 'number' },
            expectedImpressions: { type: 'number' },
            expectedClicks: { type: 'number' },
            expectedConversions: { type: 'number' },
            expectedRevenue: { type: 'number' },
            expectedROAS: { type: 'number' }
          }
        }
      }
    },
    projectedROAS: { type: 'number', required: true },
    projectedRevenue: { type: 'number', required: true },
    riskScore: { type: 'number', required: true },
    confidenceInterval: { type: 'object', required: true }
  };

  const result = await synth.generateStructured({
    count: 50,
    schema: budgetSchema,
    constraints: {
      totalBudget: { min: 10000, max: 500000 },
      timeframe: ['daily', 'weekly', 'monthly', 'quarterly'],
      projectedROAS: { min: 1.0, max: 10.0 },
      riskScore: { min: 0.1, max: 0.9 }
    }
  });

  console.log('Budget Allocation Simulations:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 2: Bid strategy testing
async function simulateBidStrategies() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const bidStrategySchema = {
    strategyId: { type: 'string', required: true },
    strategyName: { type: 'string', required: true },
    platform: { type: 'string', required: true },
    strategyType: { type: 'string', required: true },
    configuration: {
      type: 'object',
      required: true,
      properties: {
        targetCPA: { type: 'number' },
        targetROAS: { type: 'number' },
        maxCPC: { type: 'number' },
        bidAdjustments: { type: 'object' }
      }
    },
    historicalPerformance: {
      type: 'object',
      required: true,
      properties: {
        avgCPC: { type: 'number' },
        avgCPA: { type: 'number' },
        avgROAS: { type: 'number' },
        conversionRate: { type: 'number' },
        impressionShare: { type: 'number' }
      }
    },
    simulatedResults: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          scenario: { type: 'string' },
          budget: { type: 'number' },
          impressions: { type: 'number' },
          clicks: { type: 'number' },
          conversions: { type: 'number' },
          cost: { type: 'number' },
          revenue: { type: 'number' },
          cpc: { type: 'number' },
          cpa: { type: 'number' },
          roas: { type: 'number' }
        }
      }
    },
    recommendedBid: { type: 'number', required: true },
    expectedImprovement: { type: 'number', required: true }
  };

  const result = await synth.generateStructured({
    count: 40,
    schema: bidStrategySchema,
    constraints: {
      platform: ['Google Ads', 'Facebook Ads', 'TikTok Ads'],
      strategyType: [
        'manual_cpc',
        'enhanced_cpc',
        'target_cpa',
        'target_roas',
        'maximize_conversions',
        'maximize_conversion_value'
      ],
      simulatedResults: { minLength: 3, maxLength: 5 },
      expectedImprovement: { min: -0.2, max: 0.5 }
    }
  });

  console.log('Bid Strategy Simulations:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 3: Audience segmentation testing
async function simulateAudienceSegmentation() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const audienceSchema = {
    segmentId: { type: 'string', required: true },
    segmentName: { type: 'string', required: true },
    platform: { type: 'string', required: true },
    segmentType: { type: 'string', required: true },
    demographics: {
      type: 'object',
      required: true,
      properties: {
        ageRange: { type: 'string' },
        gender: { type: 'string' },
        location: { type: 'array' },
        income: { type: 'string' },
        education: { type: 'string' }
      }
    },
    interests: { type: 'array', required: true },
    behaviors: { type: 'array', required: true },
    size: { type: 'number', required: true },
    performance: {
      type: 'object',
      required: true,
      properties: {
        impressions: { type: 'number' },
        clicks: { type: 'number' },
        conversions: { type: 'number' },
        spend: { type: 'number' },
        revenue: { type: 'number' },
        ctr: { type: 'number' },
        cvr: { type: 'number' },
        cpa: { type: 'number' },
        roas: { type: 'number' },
        ltv: { type: 'number' }
      }
    },
    optimization: {
      type: 'object',
      required: true,
      properties: {
        recommendedBudget: { type: 'number' },
        recommendedBid: { type: 'number' },
        expectedROAS: { type: 'number' },
        scalingPotential: { type: 'string' }
      }
    }
  };

  const result = await synth.generateStructured({
    count: 60,
    schema: audienceSchema,
    constraints: {
      platform: ['Google Ads', 'Facebook Ads', 'TikTok Ads'],
      segmentType: [
        'lookalike',
        'custom',
        'remarketing',
        'interest_based',
        'behavioral',
        'demographic'
      ],
      size: { min: 10000, max: 10000000 },
      scalingPotential: ['low', 'medium', 'high']
    }
  });

  console.log('Audience Segmentation Data:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 4: Creative performance variations
async function simulateCreativePerformance() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const creativeSchema = {
    creativeId: { type: 'string', required: true },
    creativeName: { type: 'string', required: true },
    platform: { type: 'string', required: true },
    format: { type: 'string', required: true },
    elements: {
      type: 'object',
      required: true,
      properties: {
        headline: { type: 'string' },
        description: { type: 'string' },
        cta: { type: 'string' },
        imageUrl: { type: 'string' },
        videoUrl: { type: 'string' },
        videoDuration: { type: 'number' }
      }
    },
    variations: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          variationId: { type: 'string' },
          variationName: { type: 'string' },
          changeDescription: { type: 'string' },
          impressions: { type: 'number' },
          clicks: { type: 'number' },
          conversions: { type: 'number' },
          spend: { type: 'number' },
          revenue: { type: 'number' },
          ctr: { type: 'number' },
          cvr: { type: 'number' },
          cpa: { type: 'number' },
          roas: { type: 'number' },
          engagementRate: { type: 'number' }
        }
      }
    },
    bestPerforming: { type: 'string', required: true },
    performanceLift: { type: 'number', required: true },
    recommendation: { type: 'string', required: true }
  };

  const result = await synth.generateStructured({
    count: 50,
    schema: creativeSchema,
    constraints: {
      platform: ['Google Ads', 'Facebook Ads', 'TikTok Ads', 'Instagram Ads'],
      format: [
        'image_ad',
        'video_ad',
        'carousel_ad',
        'collection_ad',
        'story_ad',
        'responsive_display'
      ],
      variations: { minLength: 2, maxLength: 5 },
      performanceLift: { min: -0.3, max: 2.0 }
    }
  });

  console.log('Creative Performance Variations:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 5: ROAS optimization scenarios
async function simulateROASOptimization() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const roasSchema = {
    optimizationId: { type: 'string', required: true },
    optimizationName: { type: 'string', required: true },
    currentState: {
      type: 'object',
      required: true,
      properties: {
        totalSpend: { type: 'number' },
        totalRevenue: { type: 'number' },
        currentROAS: { type: 'number' },
        campaignCount: { type: 'number' },
        activeChannels: { type: 'array' }
      }
    },
    optimizationScenarios: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          scenarioId: { type: 'string' },
          scenarioName: { type: 'string' },
          changes: { type: 'array' },
          projectedSpend: { type: 'number' },
          projectedRevenue: { type: 'number' },
          projectedROAS: { type: 'number' },
          roasImprovement: { type: 'number' },
          implementationDifficulty: { type: 'string' },
          estimatedTimeframe: { type: 'string' },
          riskLevel: { type: 'string' }
        }
      }
    },
    recommendations: {
      type: 'object',
      required: true,
      properties: {
        primaryRecommendation: { type: 'string' },
        quickWins: { type: 'array' },
        longTermStrategies: { type: 'array' },
        budgetReallocation: { type: 'object' }
      }
    },
    expectedOutcome: {
      type: 'object',
      required: true,
      properties: {
        targetROAS: { type: 'number' },
        targetRevenue: { type: 'number' },
        timeToTarget: { type: 'string' },
        confidenceLevel: { type: 'number' }
      }
    }
  };

  const result = await synth.generateStructured({
    count: 30,
    schema: roasSchema,
    constraints: {
      'currentState.currentROAS': { min: 0.5, max: 5.0 },
      optimizationScenarios: { minLength: 3, maxLength: 6 },
      'expectedOutcome.targetROAS': { min: 2.0, max: 10.0 },
      'expectedOutcome.confidenceLevel': { min: 0.6, max: 0.95 }
    }
  });

  console.log('ROAS Optimization Scenarios:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 6: Time-series optimization impact
async function simulateOptimizationImpact() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateTimeSeries({
    count: 90,
    startDate: new Date(Date.now() - 90 * 24 * 60 * 60 * 1000),
    endDate: new Date(),
    interval: '1d',
    metrics: [
      'baseline_roas',
      'optimized_roas',
      'baseline_revenue',
      'optimized_revenue',
      'baseline_cpa',
      'optimized_cpa',
      'improvement_percentage'
    ],
    trend: 'up',
    seasonality: true,
    noise: 0.1,
    constraints: {
      baseline_roas: { min: 2.0, max: 4.0 },
      optimized_roas: { min: 2.5, max: 8.0 },
      baseline_revenue: { min: 5000, max: 50000 },
      optimized_revenue: { min: 6000, max: 80000 },
      improvement_percentage: { min: 0, max: 100 }
    }
  });

  console.log('Optimization Impact Time-Series:');
  console.log(result.data.slice(0, 7));

  return result;
}

// Example 7: Multi-variate testing simulation
async function simulateMultiVariateTesting() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const mvtSchema = {
    testId: { type: 'string', required: true },
    testName: { type: 'string', required: true },
    platform: { type: 'string', required: true },
    startDate: { type: 'string', required: true },
    endDate: { type: 'string', required: true },
    testFactors: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          factor: { type: 'string' },
          variations: { type: 'array' }
        }
      }
    },
    combinations: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          combinationId: { type: 'string' },
          factors: { type: 'object' },
          impressions: { type: 'number' },
          clicks: { type: 'number' },
          conversions: { type: 'number' },
          spend: { type: 'number' },
          revenue: { type: 'number' },
          ctr: { type: 'number' },
          cvr: { type: 'number' },
          cpa: { type: 'number' },
          roas: { type: 'number' },
          score: { type: 'number' }
        }
      }
    },
    winningCombination: { type: 'string', required: true },
    keyInsights: { type: 'array', required: true },
    implementationPlan: { type: 'string', required: true }
  };

  const result = await synth.generateStructured({
    count: 25,
    schema: mvtSchema,
    constraints: {
      platform: ['Google Ads', 'Facebook Ads', 'TikTok Ads'],
      testFactors: { minLength: 2, maxLength: 4 },
      combinations: { minLength: 4, maxLength: 16 }
    }
  });

  console.log('Multi-Variate Testing Results:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 8: Dayparting optimization
async function simulateDaypartingOptimization() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const daypartingSchema = {
    analysisId: { type: 'string', required: true },
    campaign: { type: 'string', required: true },
    platform: { type: 'string', required: true },
    timezone: { type: 'string', required: true },
    hourlyPerformance: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          hour: { type: 'number' },
          dayOfWeek: { type: 'string' },
          impressions: { type: 'number' },
          clicks: { type: 'number' },
          conversions: { type: 'number' },
          spend: { type: 'number' },
          revenue: { type: 'number' },
          ctr: { type: 'number' },
          cvr: { type: 'number' },
          cpa: { type: 'number' },
          roas: { type: 'number' },
          competitionLevel: { type: 'string' }
        }
      }
    },
    recommendations: {
      type: 'object',
      required: true,
      properties: {
        peakHours: { type: 'array' },
        bidAdjustments: { type: 'object' },
        budgetAllocation: { type: 'object' },
        expectedImprovement: { type: 'number' }
      }
    }
  };

  const result = await synth.generateStructured({
    count: 20,
    schema: daypartingSchema,
    constraints: {
      platform: ['Google Ads', 'Facebook Ads', 'TikTok Ads'],
      hourlyPerformance: { minLength: 168, maxLength: 168 }, // 24 hours x 7 days
      'recommendations.expectedImprovement': { min: 0.05, max: 0.5 }
    }
  });

  console.log('Dayparting Optimization Data:');
  console.log(result.data.slice(0, 1));

  return result;
}

// Example 9: Geo-targeting optimization
async function simulateGeoTargetingOptimization() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const geoSchema = {
    analysisId: { type: 'string', required: true },
    campaign: { type: 'string', required: true },
    platform: { type: 'string', required: true },
    locationPerformance: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          locationId: { type: 'string' },
          locationName: { type: 'string' },
          locationType: { type: 'string' },
          population: { type: 'number' },
          impressions: { type: 'number' },
          clicks: { type: 'number' },
          conversions: { type: 'number' },
          spend: { type: 'number' },
          revenue: { type: 'number' },
          ctr: { type: 'number' },
          cvr: { type: 'number' },
          cpa: { type: 'number' },
          roas: { type: 'number' },
          marketPotential: { type: 'string' }
        }
      }
    },
    optimization: {
      type: 'object',
      required: true,
      properties: {
        topPerformingLocations: { type: 'array' },
        underperformingLocations: { type: 'array' },
        expansionOpportunities: { type: 'array' },
        bidAdjustments: { type: 'object' },
        expectedROASImprovement: { type: 'number' }
      }
    }
  };

  const result = await synth.generateStructured({
    count: 15,
    schema: geoSchema,
    constraints: {
      platform: ['Google Ads', 'Facebook Ads', 'TikTok Ads'],
      locationPerformance: { minLength: 10, maxLength: 50 },
      'optimization.expectedROASImprovement': { min: 0.1, max: 1.0 }
    }
  });

  console.log('Geo-Targeting Optimization Data:');
  console.log(result.data.slice(0, 1));

  return result;
}

// Example 10: Batch optimization simulation
async function simulateBatchOptimization() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const scenarios = [
    {
      count: 20,
      schema: {
        scenarioType: { type: 'string' },
        currentROAS: { type: 'number' },
        optimizedROAS: { type: 'number' },
        improvement: { type: 'number' }
      },
      constraints: { scenarioType: 'budget_allocation' }
    },
    {
      count: 20,
      schema: {
        scenarioType: { type: 'string' },
        currentROAS: { type: 'number' },
        optimizedROAS: { type: 'number' },
        improvement: { type: 'number' }
      },
      constraints: { scenarioType: 'bid_strategy' }
    },
    {
      count: 20,
      schema: {
        scenarioType: { type: 'string' },
        currentROAS: { type: 'number' },
        optimizedROAS: { type: 'number' },
        improvement: { type: 'number' }
      },
      constraints: { scenarioType: 'audience_targeting' }
    }
  ];

  const results = await synth.generateBatch('structured', scenarios, 3);

  console.log('Batch Optimization Results:');
  results.forEach((result, i) => {
    const types = ['Budget Allocation', 'Bid Strategy', 'Audience Targeting'];
    console.log(`${types[i]}: ${result.metadata.count} scenarios in ${result.metadata.duration}ms`);
    console.log('Sample:', result.data.slice(0, 2));
  });

  return results;
}

// Run all examples
export async function runOptimizationExamples() {
  console.log('=== Example 1: Budget Allocation ===');
  await simulateBudgetAllocation();

  console.log('\n=== Example 2: Bid Strategies ===');
  await simulateBidStrategies();

  console.log('\n=== Example 3: Audience Segmentation ===');
  await simulateAudienceSegmentation();

  console.log('\n=== Example 4: Creative Performance ===');
  await simulateCreativePerformance();

  console.log('\n=== Example 5: ROAS Optimization ===');
  await simulateROASOptimization();

  console.log('\n=== Example 6: Optimization Impact ===');
  await simulateOptimizationImpact();

  console.log('\n=== Example 7: Multi-Variate Testing ===');
  await simulateMultiVariateTesting();

  console.log('\n=== Example 8: Dayparting Optimization ===');
  await simulateDaypartingOptimization();

  console.log('\n=== Example 9: Geo-Targeting Optimization ===');
  await simulateGeoTargetingOptimization();

  console.log('\n=== Example 10: Batch Optimization ===');
  await simulateBatchOptimization();
}

// Export individual functions
export {
  simulateBudgetAllocation,
  simulateBidStrategies,
  simulateAudienceSegmentation,
  simulateCreativePerformance,
  simulateROASOptimization,
  simulateOptimizationImpact,
  simulateMultiVariateTesting,
  simulateDaypartingOptimization,
  simulateGeoTargetingOptimization,
  simulateBatchOptimization
};

// Uncomment to run
// runOptimizationExamples().catch(console.error);
