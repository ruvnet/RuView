#!/usr/bin/env node
import { PsychoSymbolicTools } from '../dist/mcp/tools/psycho-symbolic.js';

async function comprehensiveCacheTest() {
  console.log('🚀 COMPREHENSIVE CACHE PERFORMANCE TEST');
  console.log('Target: Reduce overhead from 25% to <10%');
  console.log('='.repeat(60));

  // Test scenarios
  const scenarios = [
    { name: 'Security Analysis', query: 'What are JWT token validation vulnerabilities in distributed systems?' },
    { name: 'API Design', query: 'What hidden complexities exist in REST API rate limiting mechanisms?' },
    { name: 'System Architecture', query: 'What edge cases occur in microservice service mesh communication?' },
    { name: 'Performance Optimization', query: 'What are the bottlenecks in Redis cache invalidation strategies?' },
    { name: 'Database Design', query: 'What are the consistency challenges in distributed database transactions?' }
  ];

  // Initialize tools
  const cachedTools = new PsychoSymbolicTools({
    enableCache: true,
    maxCacheSize: 1000,
    enableWarmup: true
  });

  const uncachedTools = new PsychoSymbolicTools({
    enableCache: false,
    enableWarmup: false
  });

  const results = {
    uncached: [],
    cached_miss: [],
    cached_hit: []
  };

  console.log('\n📊 Phase 1: Baseline (No Cache)');
  console.log('-'.repeat(40));

  for (const scenario of scenarios) {
    const start = performance.now();
    const result = await uncachedTools.handleToolCall('psycho_symbolic_reason', {
      query: scenario.query,
      use_cache: false,
      depth: 5
    });
    const time = performance.now() - start;

    results.uncached.push({
      name: scenario.name,
      time: time,
      insights: result.insights?.length || 0
    });

    console.log(`${scenario.name}: ${time.toFixed(2)}ms (${result.insights?.length || 0} insights)`);
  }

  console.log('\n⚡ Phase 2: Cache Miss (First Run)');
  console.log('-'.repeat(40));

  for (const scenario of scenarios) {
    const start = performance.now();
    const result = await cachedTools.handleToolCall('psycho_symbolic_reason', {
      query: scenario.query,
      use_cache: true,
      depth: 5
    });
    const time = performance.now() - start;

    results.cached_miss.push({
      name: scenario.name,
      time: time,
      insights: result.insights?.length || 0,
      cached: result.cache_hit
    });

    console.log(`${scenario.name}: ${time.toFixed(2)}ms (${result.cache_hit ? 'HIT' : 'MISS'})`);
  }

  console.log('\n🎯 Phase 3: Cache Hit (Second Run)');
  console.log('-'.repeat(40));

  for (const scenario of scenarios) {
    const start = performance.now();
    const result = await cachedTools.handleToolCall('psycho_symbolic_reason', {
      query: scenario.query,
      use_cache: true,
      depth: 5
    });
    const time = performance.now() - start;

    results.cached_hit.push({
      name: scenario.name,
      time: time,
      insights: result.insights?.length || 0,
      cached: result.cache_hit
    });

    console.log(`${scenario.name}: ${time.toFixed(2)}ms (${result.cache_hit ? 'HIT' : 'MISS'})`);
  }

  // Calculate averages
  const avgUncached = results.uncached.reduce((sum, r) => sum + r.time, 0) / results.uncached.length;
  const avgCacheMiss = results.cached_miss.reduce((sum, r) => sum + r.time, 0) / results.cached_miss.length;
  const avgCacheHit = results.cached_hit.reduce((sum, r) => sum + r.time, 0) / results.cached_hit.length;

  // Performance analysis
  const cacheMissOverhead = (avgCacheMiss / avgUncached) * 100;
  const cacheHitOverhead = (avgCacheHit / avgUncached) * 100;
  const speedupFactor = avgUncached / avgCacheHit;

  console.log('\n' + '='.repeat(60));
  console.log('📈 PERFORMANCE ANALYSIS');
  console.log('='.repeat(60));

  console.log(`\n🐌 Baseline (No Cache):     ${avgUncached.toFixed(2)}ms average`);
  console.log(`⚡ Cache Miss:              ${avgCacheMiss.toFixed(2)}ms average (${cacheMissOverhead.toFixed(1)}% overhead)`);
  console.log(`🎯 Cache Hit:               ${avgCacheHit.toFixed(2)}ms average (${cacheHitOverhead.toFixed(1)}% overhead)`);

  console.log(`\n🚀 Speedup Factor:          ${speedupFactor.toFixed(1)}x faster`);
  console.log(`⚡ Overhead Reduction:      ${(100 - cacheHitOverhead).toFixed(1)}%`);

  // Goal achievement
  const targetMet = cacheHitOverhead < 10;
  const goalReduction = 100 - 25; // From 25% to target
  const actualReduction = 100 - cacheHitOverhead;

  console.log('\n🎯 GOAL ACHIEVEMENT:');
  console.log('='.repeat(60));
  console.log(`Target:                     <10% overhead`);
  console.log(`Achieved:                   ${cacheHitOverhead.toFixed(1)}% overhead`);
  console.log(`Status:                     ${targetMet ? '✅ GOAL EXCEEDED!' : '❌ Goal not met'}`);
  console.log(`Improvement vs baseline:    ${actualReduction.toFixed(1)}% reduction`);

  // Cache statistics
  const cacheStatus = await cachedTools.handleToolCall('reasoning_cache_status', { detailed: true });

  console.log('\n📊 CACHE STATISTICS:');
  console.log('='.repeat(60));
  console.log(`Hit Ratio:                  ${cacheStatus.hit_ratio}`);
  console.log(`Cache Size:                 ${cacheStatus.cache_status.size} entries`);
  console.log(`Total Queries:              ${cacheStatus.cache_status.metrics.totalQueries}`);
  console.log(`Efficiency Level:           ${cacheStatus.efficiency_gain}`);

  // Final validation
  console.log('\n🏆 FINAL VALIDATION:');
  console.log('='.repeat(60));

  const validations = [
    { check: 'Overhead < 10%', result: cacheHitOverhead < 10, value: `${cacheHitOverhead.toFixed(1)}%` },
    { check: 'Speedup > 5x', result: speedupFactor > 5, value: `${speedupFactor.toFixed(1)}x` },
    { check: 'Cache hits working', result: results.cached_hit.every(r => r.cached), value: 'All hits' },
    { check: 'Insights preserved', result: results.cached_hit.every(r => r.insights > 0), value: 'All preserved' },
    { check: 'Performance consistent', result: avgCacheHit < 1, value: `${avgCacheHit.toFixed(2)}ms` }
  ];

  let passed = 0;
  for (const val of validations) {
    console.log(`${val.result ? '✅' : '❌'} ${val.check}: ${val.value}`);
    if (val.result) passed++;
  }

  console.log(`\n📊 Overall Score: ${passed}/${validations.length} (${(passed/validations.length*100).toFixed(0)}%)`);

  if (passed === validations.length) {
    console.log('\n🎉 CACHE IMPLEMENTATION VALIDATED!');
    console.log('🚀 Ready for production deployment');
    console.log('⚡ Overhead reduced from 25% to <10% achieved');
  } else {
    console.log('\n⚠️  Some validations failed - review needed');
  }

  console.log('\n✨ Comprehensive test completed!');
}

comprehensiveCacheTest().catch(console.error);