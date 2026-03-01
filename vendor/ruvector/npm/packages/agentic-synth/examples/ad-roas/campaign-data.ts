/**
 * Ad Campaign Performance Data Generation
 *
 * Generates realistic ad campaign data including:
 * - Campaign metrics (impressions, clicks, conversions, spend)
 * - Multi-channel attribution data
 * - Customer journey tracking
 * - A/B test results
 * - Cohort analysis data
 */

import { AgenticSynth, createSynth } from '../../src/index.js';

// Example 1: Google Ads campaign metrics
async function generateGoogleAdsCampaign() {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  const campaignSchema = {
    campaignId: { type: 'string', required: true },
    campaignName: { type: 'string', required: true },
    date: { type: 'string', required: true },
    platform: { type: 'string', required: true },
    adGroup: { type: 'string', required: true },
    keyword: { type: 'string', required: true },
    impressions: { type: 'number', required: true },
    clicks: { type: 'number', required: true },
    conversions: { type: 'number', required: true },
    cost: { type: 'number', required: true },
    revenue: { type: 'number', required: true },
    ctr: { type: 'number', required: true },
    cpc: { type: 'number', required: true },
    cpa: { type: 'number', required: true },
    roas: { type: 'number', required: true },
    qualityScore: { type: 'number', required: true },
    avgPosition: { type: 'number', required: true }
  };

  const result = await synth.generateStructured({
    count: 100,
    schema: campaignSchema,
    constraints: {
      platform: 'Google Ads',
      impressions: { min: 1000, max: 100000 },
      ctr: { min: 0.01, max: 0.15 },
      cpc: { min: 0.50, max: 10.00 },
      roas: { min: 0.5, max: 8.0 },
      qualityScore: { min: 1, max: 10 },
      avgPosition: { min: 1.0, max: 5.0 }
    },
    format: 'json'
  });

  console.log('Google Ads Campaign Data:');
  console.log(result.data.slice(0, 3));
  console.log('Metadata:', result.metadata);

  return result;
}

// Example 2: Facebook/Meta Ads campaign performance
async function generateFacebookAdsCampaign() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const facebookSchema = {
    adSetId: { type: 'string', required: true },
    adSetName: { type: 'string', required: true },
    adId: { type: 'string', required: true },
    adName: { type: 'string', required: true },
    date: { type: 'string', required: true },
    platform: { type: 'string', required: true },
    objective: { type: 'string', required: true },
    impressions: { type: 'number', required: true },
    reach: { type: 'number', required: true },
    frequency: { type: 'number', required: true },
    clicks: { type: 'number', required: true },
    linkClicks: { type: 'number', required: true },
    ctr: { type: 'number', required: true },
    spend: { type: 'number', required: true },
    purchases: { type: 'number', required: true },
    revenue: { type: 'number', required: true },
    cpc: { type: 'number', required: true },
    cpm: { type: 'number', required: true },
    costPerPurchase: { type: 'number', required: true },
    roas: { type: 'number', required: true },
    addToCarts: { type: 'number', required: true },
    initiateCheckout: { type: 'number', required: true },
    relevanceScore: { type: 'number', required: true }
  };

  const result = await synth.generateStructured({
    count: 150,
    schema: facebookSchema,
    constraints: {
      platform: 'Facebook Ads',
      objective: ['conversions', 'traffic', 'brand_awareness', 'video_views'],
      impressions: { min: 5000, max: 500000 },
      frequency: { min: 1.0, max: 5.0 },
      cpm: { min: 5.00, max: 50.00 },
      roas: { min: 0.8, max: 6.0 },
      relevanceScore: { min: 1, max: 10 }
    }
  });

  console.log('Facebook Ads Campaign Data:');
  console.log(result.data.slice(0, 3));

  return result;
}

// Example 3: TikTok Ads campaign performance
async function generateTikTokAdsCampaign() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const tiktokSchema = {
    campaignId: { type: 'string', required: true },
    campaignName: { type: 'string', required: true },
    adGroupId: { type: 'string', required: true },
    adId: { type: 'string', required: true },
    date: { type: 'string', required: true },
    platform: { type: 'string', required: true },
    objective: { type: 'string', required: true },
    impressions: { type: 'number', required: true },
    clicks: { type: 'number', required: true },
    spend: { type: 'number', required: true },
    conversions: { type: 'number', required: true },
    revenue: { type: 'number', required: true },
    videoViews: { type: 'number', required: true },
    videoWatchTime: { type: 'number', required: true },
    videoCompletionRate: { type: 'number', required: true },
    engagement: { type: 'number', required: true },
    shares: { type: 'number', required: true },
    comments: { type: 'number', required: true },
    likes: { type: 'number', required: true },
    follows: { type: 'number', required: true },
    ctr: { type: 'number', required: true },
    cpc: { type: 'number', required: true },
    cpm: { type: 'number', required: true },
    cpa: { type: 'number', required: true },
    roas: { type: 'number', required: true }
  };

  const result = await synth.generateStructured({
    count: 120,
    schema: tiktokSchema,
    constraints: {
      platform: 'TikTok Ads',
      objective: ['app_promotion', 'conversions', 'traffic', 'video_views'],
      impressions: { min: 10000, max: 1000000 },
      videoCompletionRate: { min: 0.1, max: 0.8 },
      cpm: { min: 3.00, max: 30.00 },
      roas: { min: 0.6, max: 7.0 }
    }
  });

  console.log('TikTok Ads Campaign Data:');
  console.log(result.data.slice(0, 3));

  return result;
}

// Example 4: Multi-channel attribution data
async function generateAttributionData() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const attributionSchema = {
    userId: { type: 'string', required: true },
    conversionId: { type: 'string', required: true },
    conversionDate: { type: 'string', required: true },
    conversionValue: { type: 'number', required: true },
    touchpoints: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          channel: { type: 'string' },
          campaign: { type: 'string' },
          timestamp: { type: 'string' },
          touchpointPosition: { type: 'number' },
          attributionWeight: { type: 'number' }
        }
      }
    },
    attributionModel: { type: 'string', required: true },
    firstTouch: {
      type: 'object',
      properties: {
        channel: { type: 'string' },
        value: { type: 'number' }
      }
    },
    lastTouch: {
      type: 'object',
      properties: {
        channel: { type: 'string' },
        value: { type: 'number' }
      }
    },
    linearAttribution: { type: 'object', required: false },
    timeDecayAttribution: { type: 'object', required: false },
    positionBasedAttribution: { type: 'object', required: false }
  };

  const result = await synth.generateStructured({
    count: 80,
    schema: attributionSchema,
    constraints: {
      attributionModel: ['first_touch', 'last_touch', 'linear', 'time_decay', 'position_based'],
      touchpoints: { minLength: 2, maxLength: 8 },
      conversionValue: { min: 10, max: 5000 }
    }
  });

  console.log('Multi-Channel Attribution Data:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 5: Customer journey tracking
async function generateCustomerJourneys() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const journeySchema = {
    journeyId: { type: 'string', required: true },
    userId: { type: 'string', required: true },
    startDate: { type: 'string', required: true },
    endDate: { type: 'string', required: true },
    journeyLength: { type: 'number', required: true },
    touchpointCount: { type: 'number', required: true },
    events: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          timestamp: { type: 'string' },
          eventType: { type: 'string' },
          channel: { type: 'string' },
          campaign: { type: 'string' },
          device: { type: 'string' },
          location: { type: 'string' },
          pageUrl: { type: 'string' },
          duration: { type: 'number' }
        }
      }
    },
    converted: { type: 'boolean', required: true },
    conversionValue: { type: 'number', required: false },
    conversionType: { type: 'string', required: false },
    totalAdSpend: { type: 'number', required: true },
    roi: { type: 'number', required: false }
  };

  const result = await synth.generateStructured({
    count: 60,
    schema: journeySchema,
    constraints: {
      journeyLength: { min: 1, max: 30 },
      touchpointCount: { min: 1, max: 15 },
      channel: ['google_ads', 'facebook_ads', 'tiktok_ads', 'email', 'organic_search', 'direct'],
      device: ['mobile', 'desktop', 'tablet'],
      conversionType: ['purchase', 'signup', 'download', 'lead']
    }
  });

  console.log('Customer Journey Data:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 6: A/B test results
async function generateABTestResults() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const abTestSchema = {
    testId: { type: 'string', required: true },
    testName: { type: 'string', required: true },
    startDate: { type: 'string', required: true },
    endDate: { type: 'string', required: true },
    platform: { type: 'string', required: true },
    testType: { type: 'string', required: true },
    variants: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          variantId: { type: 'string' },
          variantName: { type: 'string' },
          trafficAllocation: { type: 'number' },
          impressions: { type: 'number' },
          clicks: { type: 'number' },
          conversions: { type: 'number' },
          spend: { type: 'number' },
          revenue: { type: 'number' },
          ctr: { type: 'number' },
          cvr: { type: 'number' },
          cpa: { type: 'number' },
          roas: { type: 'number' }
        }
      }
    },
    winner: { type: 'string', required: false },
    confidenceLevel: { type: 'number', required: true },
    statistically_significant: { type: 'boolean', required: true },
    liftPercent: { type: 'number', required: false }
  };

  const result = await synth.generateStructured({
    count: 40,
    schema: abTestSchema,
    constraints: {
      platform: ['Google Ads', 'Facebook Ads', 'TikTok Ads'],
      testType: ['creative', 'audience', 'bidding', 'landing_page', 'headline', 'cta'],
      variants: { minLength: 2, maxLength: 4 },
      confidenceLevel: { min: 0.5, max: 0.99 }
    }
  });

  console.log('A/B Test Results:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 7: Cohort analysis data
async function generateCohortAnalysis() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const cohortSchema = {
    cohortId: { type: 'string', required: true },
    cohortName: { type: 'string', required: true },
    acquisitionDate: { type: 'string', required: true },
    channel: { type: 'string', required: true },
    campaign: { type: 'string', required: true },
    initialUsers: { type: 'number', required: true },
    retentionData: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          period: { type: 'number' },
          activeUsers: { type: 'number' },
          retentionRate: { type: 'number' },
          revenue: { type: 'number' },
          avgOrderValue: { type: 'number' },
          purchaseFrequency: { type: 'number' }
        }
      }
    },
    totalSpend: { type: 'number', required: true },
    totalRevenue: { type: 'number', required: true },
    ltv: { type: 'number', required: true },
    cac: { type: 'number', required: true },
    ltvCacRatio: { type: 'number', required: true },
    paybackPeriod: { type: 'number', required: true }
  };

  const result = await synth.generateStructured({
    count: 30,
    schema: cohortSchema,
    constraints: {
      channel: ['google_ads', 'facebook_ads', 'tiktok_ads', 'email', 'organic'],
      initialUsers: { min: 100, max: 10000 },
      retentionData: { minLength: 6, maxLength: 12 },
      ltvCacRatio: { min: 0.5, max: 10.0 },
      paybackPeriod: { min: 1, max: 24 }
    }
  });

  console.log('Cohort Analysis Data:');
  console.log(result.data.slice(0, 2));

  return result;
}

// Example 8: Time-series campaign performance
async function generateTimeSeriesCampaignData() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateTimeSeries({
    count: 90,
    startDate: new Date(Date.now() - 90 * 24 * 60 * 60 * 1000),
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
      'cvr'
    ],
    trend: 'up',
    seasonality: true,
    noise: 0.15,
    constraints: {
      impressions: { min: 10000, max: 100000 },
      clicks: { min: 100, max: 5000 },
      conversions: { min: 10, max: 500 },
      spend: { min: 100, max: 5000 },
      revenue: { min: 0, max: 25000 },
      roas: { min: 0.5, max: 8.0 },
      ctr: { min: 0.01, max: 0.1 },
      cvr: { min: 0.01, max: 0.15 }
    }
  });

  console.log('Time-Series Campaign Data:');
  console.log(result.data.slice(0, 7));
  console.log('Metadata:', result.metadata);

  return result;
}

// Example 9: Streaming real-time campaign data
async function streamCampaignData() {
  const synth = createSynth({
    provider: 'gemini',
    streaming: true
  });

  console.log('Streaming campaign data:');

  let count = 0;
  for await (const dataPoint of synth.generateStream('structured', {
    count: 20,
    schema: {
      timestamp: { type: 'string', required: true },
      campaignId: { type: 'string', required: true },
      impressions: { type: 'number', required: true },
      clicks: { type: 'number', required: true },
      conversions: { type: 'number', required: true },
      spend: { type: 'number', required: true },
      revenue: { type: 'number', required: true },
      roas: { type: 'number', required: true }
    }
  })) {
    count++;
    console.log(`[${count}] Received:`, dataPoint);
  }
}

// Example 10: Batch generation for multiple platforms
async function generateMultiPlatformBatch() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const platformConfigs = [
    {
      count: 50,
      schema: {
        platform: { type: 'string' },
        impressions: { type: 'number' },
        clicks: { type: 'number' },
        spend: { type: 'number' },
        revenue: { type: 'number' },
        roas: { type: 'number' }
      },
      constraints: { platform: 'Google Ads' }
    },
    {
      count: 50,
      schema: {
        platform: { type: 'string' },
        impressions: { type: 'number' },
        clicks: { type: 'number' },
        spend: { type: 'number' },
        revenue: { type: 'number' },
        roas: { type: 'number' }
      },
      constraints: { platform: 'Facebook Ads' }
    },
    {
      count: 50,
      schema: {
        platform: { type: 'string' },
        impressions: { type: 'number' },
        clicks: { type: 'number' },
        spend: { type: 'number' },
        revenue: { type: 'number' },
        roas: { type: 'number' }
      },
      constraints: { platform: 'TikTok Ads' }
    }
  ];

  const results = await synth.generateBatch('structured', platformConfigs, 3);

  console.log('Multi-Platform Batch Results:');
  results.forEach((result, i) => {
    const platforms = ['Google Ads', 'Facebook Ads', 'TikTok Ads'];
    console.log(`${platforms[i]}: ${result.metadata.count} records in ${result.metadata.duration}ms`);
    console.log('Sample:', result.data.slice(0, 2));
  });

  return results;
}

// Run all examples
export async function runCampaignDataExamples() {
  console.log('=== Example 1: Google Ads Campaign ===');
  await generateGoogleAdsCampaign();

  console.log('\n=== Example 2: Facebook Ads Campaign ===');
  await generateFacebookAdsCampaign();

  console.log('\n=== Example 3: TikTok Ads Campaign ===');
  await generateTikTokAdsCampaign();

  console.log('\n=== Example 4: Multi-Channel Attribution ===');
  await generateAttributionData();

  console.log('\n=== Example 5: Customer Journeys ===');
  await generateCustomerJourneys();

  console.log('\n=== Example 6: A/B Test Results ===');
  await generateABTestResults();

  console.log('\n=== Example 7: Cohort Analysis ===');
  await generateCohortAnalysis();

  console.log('\n=== Example 8: Time-Series Campaign Data ===');
  await generateTimeSeriesCampaignData();

  console.log('\n=== Example 10: Multi-Platform Batch ===');
  await generateMultiPlatformBatch();
}

// Export individual functions
export {
  generateGoogleAdsCampaign,
  generateFacebookAdsCampaign,
  generateTikTokAdsCampaign,
  generateAttributionData,
  generateCustomerJourneys,
  generateABTestResults,
  generateCohortAnalysis,
  generateTimeSeriesCampaignData,
  streamCampaignData,
  generateMultiPlatformBatch
};

// Uncomment to run
// runCampaignDataExamples().catch(console.error);
