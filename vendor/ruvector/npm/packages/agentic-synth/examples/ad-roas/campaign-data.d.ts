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
declare function generateGoogleAdsCampaign(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function generateFacebookAdsCampaign(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function generateTikTokAdsCampaign(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function generateAttributionData(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function generateCustomerJourneys(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function generateABTestResults(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function generateCohortAnalysis(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function generateTimeSeriesCampaignData(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function streamCampaignData(): Promise<void>;
declare function generateMultiPlatformBatch(): Promise<import("../../src/types.js").GenerationResult<unknown>[]>;
export declare function runCampaignDataExamples(): Promise<void>;
export { generateGoogleAdsCampaign, generateFacebookAdsCampaign, generateTikTokAdsCampaign, generateAttributionData, generateCustomerJourneys, generateABTestResults, generateCohortAnalysis, generateTimeSeriesCampaignData, streamCampaignData, generateMultiPlatformBatch };
//# sourceMappingURL=campaign-data.d.ts.map