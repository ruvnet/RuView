# Ad ROAS (Return on Ad Spend) Tracking Examples

Comprehensive examples for generating advertising and marketing analytics data using agentic-synth. These examples demonstrate how to create realistic campaign performance data, optimization scenarios, and analytics pipelines for major advertising platforms.

## Overview

This directory contains practical examples for:

- **Campaign Performance Tracking**: Generate realistic ad campaign metrics
- **Optimization Simulations**: Test budget allocation and bidding strategies
- **Analytics Pipelines**: Build comprehensive marketing analytics systems
- **Multi-Platform Integration**: Work with Google Ads, Facebook Ads, TikTok Ads

## Files

### 1. campaign-data.ts

Generates comprehensive ad campaign performance data including:

- **Platform-Specific Campaigns**
  - Google Ads (Search, Display, Shopping)
  - Facebook/Meta Ads (Feed, Stories, Reels)
  - TikTok Ads (In-Feed, TopView, Branded Effects)

- **Multi-Channel Attribution**
  - First-touch, last-touch, linear attribution
  - Time-decay and position-based models
  - Data-driven attribution

- **Customer Journey Tracking**
  - Touchpoint analysis
  - Path to conversion
  - Device and location tracking

- **A/B Testing Results**
  - Creative variations
  - Audience testing
  - Landing page experiments

- **Cohort Analysis**
  - Retention rates
  - LTV calculations
  - Payback periods

### 2. optimization-simulator.ts

Simulates various optimization scenarios:

- **Budget Allocation**
  - Cross-platform budget distribution
  - ROI-based allocation
  - Risk-adjusted scenarios

- **Bid Strategy Testing**
  - Manual CPC vs automated bidding
  - Target CPA/ROAS strategies
  - Maximize conversions/value

- **Audience Segmentation**
  - Demographic targeting
  - Interest-based audiences
  - Lookalike/similar audiences
  - Custom and remarketing lists

- **Creative Optimization**
  - Ad format testing
  - Copy variations
  - Visual element testing

- **Advanced Optimizations**
  - Dayparting analysis
  - Geo-targeting optimization
  - Multi-variate testing

### 3. analytics-pipeline.ts

Marketing analytics and modeling examples:

- **Attribution Modeling**
  - Compare attribution models
  - Channel valuation
  - Cross-channel interactions

- **LTV (Lifetime Value) Analysis**
  - Cohort-based LTV
  - Predictive LTV models
  - LTV:CAC ratios

- **Funnel Analysis**
  - Conversion funnel stages
  - Dropout analysis
  - Bottleneck identification

- **Predictive Analytics**
  - Revenue forecasting
  - Scenario planning
  - Risk assessment

- **Marketing Mix Modeling (MMM)**
  - Channel contribution analysis
  - Saturation curves
  - Optimal budget allocation

- **Incrementality Testing**
  - Geo holdout tests
  - PSA (Public Service Announcement) tests
  - True lift measurement

## Quick Start

### Basic Usage

```typescript
import { createSynth } from 'agentic-synth';

// Initialize with your API key
const synth = createSynth({
  provider: 'gemini',
  apiKey: process.env.GEMINI_API_KEY
});

// Generate Google Ads campaign data
const campaigns = await synth.generateStructured({
  count: 100,
  schema: {
    campaignId: { type: 'string', required: true },
    impressions: { type: 'number', required: true },
    clicks: { type: 'number', required: true },
    conversions: { type: 'number', required: true },
    spend: { type: 'number', required: true },
    revenue: { type: 'number', required: true },
    roas: { type: 'number', required: true }
  },
  constraints: {
    impressions: { min: 1000, max: 100000 },
    roas: { min: 0.5, max: 8.0 }
  }
});
```

### Time-Series Campaign Data

```typescript
// Generate daily campaign metrics for 90 days
const timeSeries = await synth.generateTimeSeries({
  count: 90,
  interval: '1d',
  metrics: ['impressions', 'clicks', 'conversions', 'spend', 'revenue', 'roas'],
  trend: 'up',
  seasonality: true,
  constraints: {
    roas: { min: 1.0, max: 10.0 }
  }
});
```

### Multi-Platform Batch Generation

```typescript
// Generate data for multiple platforms in parallel
const platforms = [
  { count: 50, constraints: { platform: 'Google Ads' } },
  { count: 50, constraints: { platform: 'Facebook Ads' } },
  { count: 50, constraints: { platform: 'TikTok Ads' } }
];

const results = await synth.generateBatch('structured', platforms, 3);
```

## Real-World Use Cases

### 1. Performance Dashboard Testing

Generate realistic data for testing marketing dashboards:

```typescript
import { generateTimeSeriesCampaignData } from './campaign-data.js';

// Generate 6 months of daily metrics
const dashboardData = await generateTimeSeriesCampaignData();

// Use for:
// - Frontend dashboard development
// - Chart/visualization testing
// - Performance optimization
// - Demo presentations
```

### 2. Attribution Model Comparison

Compare different attribution models:

```typescript
import { generateAttributionModels } from './analytics-pipeline.js';

// Generate attribution data for analysis
const attribution = await generateAttributionModels();

// Compare:
// - First-touch vs last-touch
// - Linear vs time-decay
// - Position-based vs data-driven
```

### 3. Budget Optimization Simulation

Test budget allocation strategies:

```typescript
import { simulateBudgetAllocation } from './optimization-simulator.js';

// Generate optimization scenarios
const scenarios = await simulateBudgetAllocation();

// Analyze:
// - Risk-adjusted returns
// - Diversification benefits
// - Scaling opportunities
```

### 4. A/B Test Planning

Plan and simulate A/B tests:

```typescript
import { generateABTestResults } from './campaign-data.js';

// Generate A/B test data
const tests = await generateABTestResults();

// Use for:
// - Sample size calculations
// - Statistical significance testing
// - Test design validation
```

### 5. LTV Analysis & Forecasting

Analyze customer lifetime value:

```typescript
import { generateLTVAnalysis } from './analytics-pipeline.js';

// Generate cohort LTV data
const ltvData = await generateLTVAnalysis();

// Calculate:
// - Payback periods
// - LTV:CAC ratios
// - Retention curves
```

## Platform-Specific Examples

### Google Ads

```typescript
// Search campaign with quality score
const googleAds = await synth.generateStructured({
  count: 100,
  schema: {
    keyword: { type: 'string' },
    matchType: { type: 'string' },
    qualityScore: { type: 'number' },
    avgPosition: { type: 'number' },
    impressionShare: { type: 'number' },
    cpc: { type: 'number' },
    roas: { type: 'number' }
  },
  constraints: {
    matchType: ['exact', 'phrase', 'broad'],
    qualityScore: { min: 1, max: 10 }
  }
});
```

### Facebook/Meta Ads

```typescript
// Facebook campaign with engagement metrics
const facebookAds = await synth.generateStructured({
  count: 100,
  schema: {
    objective: { type: 'string' },
    placement: { type: 'string' },
    reach: { type: 'number' },
    frequency: { type: 'number' },
    engagement: { type: 'number' },
    relevanceScore: { type: 'number' },
    cpm: { type: 'number' },
    roas: { type: 'number' }
  },
  constraints: {
    objective: ['conversions', 'traffic', 'engagement'],
    placement: ['feed', 'stories', 'reels', 'marketplace']
  }
});
```

### TikTok Ads

```typescript
// TikTok campaign with video metrics
const tiktokAds = await synth.generateStructured({
  count: 100,
  schema: {
    objective: { type: 'string' },
    videoViews: { type: 'number' },
    videoCompletionRate: { type: 'number' },
    engagement: { type: 'number' },
    shares: { type: 'number' },
    follows: { type: 'number' },
    roas: { type: 'number' }
  },
  constraints: {
    objective: ['conversions', 'app_install', 'video_views'],
    videoCompletionRate: { min: 0.1, max: 0.8 }
  }
});
```

## Advanced Features

### Streaming Real-Time Data

```typescript
// Stream campaign metrics in real-time
const synth = createSynth({ streaming: true });

for await (const metric of synth.generateStream('structured', {
  count: 100,
  schema: {
    timestamp: { type: 'string' },
    roas: { type: 'number' },
    alert: { type: 'string' }
  }
})) {
  console.log('Real-time metric:', metric);

  // Trigger alerts based on ROAS
  if (metric.roas < 1.0) {
    console.log('⚠️ ROAS below target!');
  }
}
```

### Caching for Performance

```typescript
// Use caching for repeated queries
const synth = createSynth({
  cacheStrategy: 'memory',
  cacheTTL: 600 // 10 minutes
});

// First call generates data
const data1 = await synth.generateStructured({ count: 100, schema });

// Second call uses cache (much faster)
const data2 = await synth.generateStructured({ count: 100, schema });
```

### Custom Constraints

```typescript
// Apply realistic business constraints
const campaigns = await synth.generateStructured({
  count: 50,
  schema: campaignSchema,
  constraints: {
    // Budget constraints
    spend: { min: 1000, max: 50000 },

    // Performance constraints
    roas: { min: 2.0, max: 10.0 },
    cpa: { max: 50.0 },

    // Volume constraints
    impressions: { min: 10000 },
    clicks: { min: 100 },
    conversions: { min: 10 },

    // Platform-specific
    platform: ['Google Ads', 'Facebook Ads'],
    status: ['active', 'paused']
  }
});
```

## Integration Examples

### Data Warehouse Pipeline

```typescript
import { generateTimeSeriesCampaignData } from './campaign-data.js';

async function loadToWarehouse() {
  const campaigns = await generateTimeSeriesCampaignData();

  // Transform to warehouse schema
  const rows = campaigns.data.map(campaign => ({
    date: campaign.timestamp,
    platform: campaign.platform,
    metrics: {
      impressions: campaign.impressions,
      clicks: campaign.clicks,
      spend: campaign.spend,
      revenue: campaign.revenue,
      roas: campaign.roas
    }
  }));

  // Load to BigQuery, Snowflake, Redshift, etc.
  await warehouse.bulkInsert('campaigns', rows);
}
```

### BI Tool Testing

```typescript
import { generateChannelComparison } from './analytics-pipeline.js';

async function generateBIReport() {
  const comparison = await generateChannelComparison();

  // Export for Tableau, Looker, Power BI
  const csv = convertToCSV(comparison.data);
  await fs.writeFile('channel_performance.csv', csv);
}
```

### ML Model Training

```typescript
import { generateLTVAnalysis } from './analytics-pipeline.js';

async function trainPredictiveModel() {
  // Generate training data
  const ltvData = await generateLTVAnalysis();

  // Features for ML model
  const features = ltvData.data.map(cohort => ({
    acquisitionChannel: cohort.acquisitionChannel,
    firstPurchase: cohort.metrics.avgFirstPurchase,
    frequency: cohort.metrics.purchaseFrequency,
    retention: cohort.metrics.retentionRate,
    // Target variable
    ltv: cohort.ltvCalculations.predictiveLTV
  }));

  // Train with TensorFlow, scikit-learn, etc.
  await model.train(features);
}
```

## Best Practices

### 1. Use Realistic Constraints

```typescript
// ✅ Good: Realistic business constraints
const campaigns = await synth.generateStructured({
  constraints: {
    roas: { min: 0.5, max: 15.0 },  // Typical range
    ctr: { min: 0.01, max: 0.15 },   // 1-15%
    cvr: { min: 0.01, max: 0.20 }    // 1-20%
  }
});

// ❌ Bad: Unrealistic values
const bad = await synth.generateStructured({
  constraints: {
    roas: { min: 50.0 },  // Too high
    ctr: { min: 0.5 }     // 50% CTR unrealistic
  }
});
```

### 2. Match Platform Characteristics

```typescript
// Different platforms have different metrics
const googleAds = {
  qualityScore: { min: 1, max: 10 },
  avgPosition: { min: 1.0, max: 5.0 }
};

const facebookAds = {
  relevanceScore: { min: 1, max: 10 },
  frequency: { min: 1.0, max: 5.0 }
};

const tiktokAds = {
  videoCompletionRate: { min: 0.1, max: 0.8 },
  engagement: { min: 0.02, max: 0.15 }
};
```

### 3. Consider Seasonality

```typescript
// Include seasonal patterns for realistic data
const seasonal = await synth.generateTimeSeries({
  count: 365,
  interval: '1d',
  seasonality: true,  // Includes weekly/monthly patterns
  trend: 'up',        // Long-term growth
  noise: 0.15         // 15% random variation
});
```

### 4. Use Batch Processing

```typescript
// Generate large datasets efficiently
const batches = Array.from({ length: 10 }, (_, i) => ({
  count: 1000,
  schema: campaignSchema
}));

const results = await synth.generateBatch('structured', batches, 5);
// Processes 10,000 records in parallel
```

## Performance Tips

1. **Enable Caching**: Reuse generated data for similar queries
2. **Batch Operations**: Generate multiple datasets in parallel
3. **Streaming**: Use for real-time or large datasets
4. **Constraints**: Be specific to reduce generation time
5. **Schema Design**: Simpler schemas generate faster

## Testing Scenarios

### Unit Testing

```typescript
import { generateGoogleAdsCampaign } from './campaign-data.js';

describe('Campaign Data Generator', () => {
  it('should generate valid ROAS values', async () => {
    const result = await generateGoogleAdsCampaign();

    result.data.forEach(campaign => {
      expect(campaign.roas).toBeGreaterThanOrEqual(0.5);
      expect(campaign.roas).toBeLessThanOrEqual(8.0);
    });
  });
});
```

### Integration Testing

```typescript
import { runAnalyticsExamples } from './analytics-pipeline.js';

async function testAnalyticsPipeline() {
  // Generate test data
  await runAnalyticsExamples();

  // Verify pipeline processes data correctly
  const processed = await pipeline.run();

  expect(processed.success).toBe(true);
}
```

## Troubleshooting

### API Key Issues

```typescript
// Ensure API key is set
if (!process.env.GEMINI_API_KEY) {
  throw new Error('GEMINI_API_KEY not found');
}

const synth = createSynth({
  provider: 'gemini',
  apiKey: process.env.GEMINI_API_KEY
});
```

### Rate Limiting

```typescript
// Use retry logic for rate limits
const synth = createSynth({
  maxRetries: 5,
  timeout: 60000  // 60 seconds
});
```

### Memory Management

```typescript
// Use streaming for large datasets
const synth = createSynth({ streaming: true });

for await (const chunk of synth.generateStream('structured', {
  count: 100000,
  schema: simpleSchema
})) {
  await processChunk(chunk);
  // Process in batches to avoid memory issues
}
```

## Additional Resources

- [agentic-synth Documentation](../../README.md)
- [API Reference](../../docs/API.md)
- [Examples Directory](../)
- [Google Ads API](https://developers.google.com/google-ads/api)
- [Facebook Marketing API](https://developers.facebook.com/docs/marketing-apis)
- [TikTok for Business](https://ads.tiktok.com/marketing_api/docs)

## License

MIT

## Contributing

Contributions welcome! Please see the main repository for guidelines.

## Support

For issues or questions:
- Open an issue on GitHub
- Check existing examples
- Review documentation

## Changelog

### v0.1.0 (2025-11-22)
- Initial release
- Campaign data generation
- Optimization simulators
- Analytics pipelines
- Multi-platform support
