#!/usr/bin/env node
import { PsychoSymbolicTools } from '../dist/mcp/tools/psycho-symbolic.js';

async function testCachePerformance() {
  console.log('🚀 Testing High-Performance Reasoning Cache\n');
  console.log('='.repeat(60));

  // Test queries to benchmark
  const testQueries = [
    'What are the security vulnerabilities in JWT token validation?',
    'What are the hidden complexities in API rate limiting?',
    'What edge cases exist in distributed user authentication?',
    'What are the performance implications of Redis caching?',
    'How do microservices handle service mesh failures?'
  ];

  // Initialize with cache enabled
  const toolsWithCache = new PsychoSymbolicTools({
    enableCache: true,
    maxCacheSize: 1000,
    enableWarmup: true
  });

  // Initialize without cache
  const toolsWithoutCache = new PsychoSymbolicTools({
    enableCache: false,
    enableWarmup: false
  });

  console.log('\n📊 Performance Comparison: Cache vs No Cache\n');

  const results = {
    withCache: [],
    withoutCache: []
  };

  // Test WITHOUT cache first
  console.log('🔄 Testing without cache...');
  for (const query of testQueries) {
    const startTime = performance.now();

    const result = await toolsWithoutCache.handleToolCall('psycho_symbolic_reason', {
      query,
      use_cache: false,
      depth: 5
    });

    const endTime = performance.now();
    const duration = endTime - startTime;

    results.withoutCache.push({
      query: query.substring(0, 50) + '...',
      duration: duration.toFixed(2),
      insights: result.insights?.length || 0
    });

    console.log(`  ⏱️  ${duration.toFixed(2)}ms - ${result.insights?.length || 0} insights`);
  }

  console.log('\n🚀 Testing with cache...');

  // Test WITH cache (first run - cache misses)
  for (const query of testQueries) {
    const startTime = performance.now();

    const result = await toolsWithCache.handleToolCall('psycho_symbolic_reason', {
      query,
      use_cache: true,
      depth: 5
    });

    const endTime = performance.now();
    const duration = endTime - startTime;

    results.withCache.push({
      query: query.substring(0, 50) + '...',
      duration: duration.toFixed(2),
      insights: result.insights?.length || 0,
      cached: result.cache_hit || false
    });

    console.log(`  ⚡ ${duration.toFixed(2)}ms - ${result.insights?.length || 0} insights - Cache: ${result.cache_hit ? 'HIT' : 'MISS'}`);
  }

  // Test WITH cache (second run - should be cache hits)
  console.log('\n⚡ Testing cached queries (should be fast)...');
  const cachedResults = [];

  for (const query of testQueries.slice(0, 3)) { // Test first 3 for cache hits
    const startTime = performance.now();

    const result = await toolsWithCache.handleToolCall('psycho_symbolic_reason', {
      query,
      use_cache: true,
      depth: 5
    });

    const endTime = performance.now();
    const duration = endTime - startTime;

    cachedResults.push({
      query: query.substring(0, 50) + '...',
      duration: duration.toFixed(2),
      cached: result.cache_hit || false
    });

    console.log(`  🎯 ${duration.toFixed(2)}ms - Cache: ${result.cache_hit ? 'HIT' : 'MISS'}`);
  }

  // Calculate performance metrics
  const avgWithoutCache = results.withoutCache.reduce((sum, r) => sum + parseFloat(r.duration), 0) / results.withoutCache.length;
  const avgWithCache = results.withCache.reduce((sum, r) => sum + parseFloat(r.duration), 0) / results.withCache.length;
  const avgCacheHits = cachedResults.reduce((sum, r) => sum + parseFloat(r.duration), 0) / cachedResults.length;

  console.log('\n' + '='.repeat(60));
  console.log('📈 PERFORMANCE RESULTS:');
  console.log('='.repeat(60));

  console.log(`\n🐌 Without Cache:`);
  console.log(`   Average: ${avgWithoutCache.toFixed(2)}ms`);

  console.log(`\n⚡ With Cache (first run):`);
  console.log(`   Average: ${avgWithCache.toFixed(2)}ms`);
  console.log(`   Improvement: ${((avgWithoutCache - avgWithCache) / avgWithoutCache * 100).toFixed(1)}%`);

  console.log(`\n🎯 Cache Hits:`);
  console.log(`   Average: ${avgCacheHits.toFixed(2)}ms`);
  console.log(`   Improvement: ${((avgWithoutCache - avgCacheHits) / avgWithoutCache * 100).toFixed(1)}%`);
  console.log(`   Overhead Reduction: ${(100 - (avgCacheHits / avgWithoutCache * 100)).toFixed(1)}%`);

  // Cache status
  const cacheStatus = await toolsWithCache.handleToolCall('reasoning_cache_status', { detailed: true });

  console.log('\n📊 CACHE STATISTICS:');
  console.log('='.repeat(60));
  console.log(`Hit Ratio: ${cacheStatus.hit_ratio}`);
  console.log(`Cache Size: ${cacheStatus.cache_status.size} entries`);
  console.log(`Efficiency: ${cacheStatus.efficiency_gain}`);
  console.log(`Overhead Reduction: ${cacheStatus.overhead_reduction}`);

  // Performance goal check
  const actualOverhead = (avgCacheHits / avgWithoutCache * 100);
  const targetMet = actualOverhead < 10;

  console.log('\n🎯 PERFORMANCE TARGET:');
  console.log('='.repeat(60));
  console.log(`Target: <10% overhead`);
  console.log(`Actual: ${actualOverhead.toFixed(1)}% overhead`);
  console.log(`Status: ${targetMet ? '✅ TARGET MET!' : '❌ Target not met'}`);

  console.log('\n✨ Cache performance test completed!');
}

testCachePerformance().catch(console.error);