/**
 * Benchmark Scenarios for RuVector
 *
 * Defines comprehensive test scenarios including baseline, burst, failover, and stress tests
 */

import { LoadConfig } from './load-generator';

export interface Scenario {
  name: string;
  description: string;
  config: LoadConfig;
  k6Options: any;
  expectedMetrics: {
    p99Latency: number; // milliseconds
    errorRate: number; // percentage
    throughput: number; // queries per second
    availability: number; // percentage
  };
  preTestHook?: string;
  postTestHook?: string;
  regions?: string[];
  duration: string;
  tags: string[];
}

export const SCENARIOS: Record<string, Scenario> = {
  // ==================== BASELINE SCENARIOS ====================

  baseline_500m: {
    name: 'Baseline 500M Concurrent',
    description: 'Steady-state operation with 500M concurrent connections',
    config: {
      targetConnections: 500000000,
      rampUpDuration: '30m',
      steadyStateDuration: '2h',
      rampDownDuration: '15m',
      queriesPerConnection: 100,
      queryInterval: '1000',
      protocol: 'http',
      vectorDimension: 768,
      queryPattern: 'uniform',
    },
    k6Options: {
      scenarios: {
        baseline: {
          executor: 'ramping-vus',
          startVUs: 0,
          stages: [
            { duration: '30m', target: 500000 },
            { duration: '2h', target: 500000 },
            { duration: '15m', target: 0 },
          ],
          gracefulRampDown: '30s',
        },
      },
      thresholds: {
        'query_latency': ['p(99)<50'],
        'error_rate': ['rate<0.0001'],
      },
    },
    expectedMetrics: {
      p99Latency: 50,
      errorRate: 0.01,
      throughput: 50000000, // 50M queries/sec
      availability: 99.99,
    },
    preTestHook: 'npx claude-flow@alpha hooks pre-task --description "Baseline 500M concurrent test"',
    postTestHook: 'npx claude-flow@alpha hooks post-task --task-id "baseline_500m"',
    regions: ['all'],
    duration: '3h15m',
    tags: ['baseline', 'steady-state', 'production-simulation'],
  },

  baseline_100m: {
    name: 'Baseline 100M Concurrent',
    description: 'Smaller baseline for quick validation',
    config: {
      targetConnections: 100000000,
      rampUpDuration: '10m',
      steadyStateDuration: '30m',
      rampDownDuration: '5m',
      queriesPerConnection: 50,
      queryInterval: '1000',
      protocol: 'http',
      vectorDimension: 768,
      queryPattern: 'uniform',
    },
    k6Options: {
      scenarios: {
        baseline: {
          executor: 'ramping-vus',
          startVUs: 0,
          stages: [
            { duration: '10m', target: 100000 },
            { duration: '30m', target: 100000 },
            { duration: '5m', target: 0 },
          ],
        },
      },
    },
    expectedMetrics: {
      p99Latency: 50,
      errorRate: 0.01,
      throughput: 10000000,
      availability: 99.99,
    },
    duration: '45m',
    tags: ['baseline', 'quick-test'],
  },

  // ==================== BURST SCENARIOS ====================

  burst_10x: {
    name: 'Burst 10x (5B Concurrent)',
    description: 'Sudden spike to 5 billion concurrent connections',
    config: {
      targetConnections: 5000000000,
      rampUpDuration: '5m',
      steadyStateDuration: '10m',
      rampDownDuration: '5m',
      queriesPerConnection: 20,
      queryInterval: '500',
      protocol: 'http',
      vectorDimension: 768,
      queryPattern: 'burst',
      burstConfig: {
        multiplier: 10,
        duration: '300000', // 5 minutes
        frequency: '600000', // every 10 minutes
      },
    },
    k6Options: {
      scenarios: {
        burst: {
          executor: 'ramping-arrival-rate',
          startRate: 50000000,
          timeUnit: '1s',
          preAllocatedVUs: 500000,
          maxVUs: 5000000,
          stages: [
            { duration: '5m', target: 500000000 }, // 500M/sec
            { duration: '10m', target: 500000000 },
            { duration: '5m', target: 50000000 },
          ],
        },
      },
    },
    expectedMetrics: {
      p99Latency: 100,
      errorRate: 0.1,
      throughput: 500000000,
      availability: 99.9,
    },
    preTestHook: 'npx claude-flow@alpha hooks pre-task --description "Burst 10x test"',
    postTestHook: 'npx claude-flow@alpha hooks post-task --task-id "burst_10x"',
    duration: '20m',
    tags: ['burst', 'spike', 'stress-test'],
  },

  burst_25x: {
    name: 'Burst 25x (12.5B Concurrent)',
    description: 'Extreme spike to 12.5 billion concurrent connections',
    config: {
      targetConnections: 12500000000,
      rampUpDuration: '10m',
      steadyStateDuration: '15m',
      rampDownDuration: '10m',
      queriesPerConnection: 10,
      queryInterval: '500',
      protocol: 'http2',
      vectorDimension: 768,
      queryPattern: 'burst',
      burstConfig: {
        multiplier: 25,
        duration: '900000', // 15 minutes
        frequency: '1800000', // every 30 minutes
      },
    },
    k6Options: {
      scenarios: {
        extreme_burst: {
          executor: 'ramping-arrival-rate',
          startRate: 50000000,
          timeUnit: '1s',
          preAllocatedVUs: 1000000,
          maxVUs: 12500000,
          stages: [
            { duration: '10m', target: 1250000000 },
            { duration: '15m', target: 1250000000 },
            { duration: '10m', target: 50000000 },
          ],
        },
      },
    },
    expectedMetrics: {
      p99Latency: 150,
      errorRate: 0.5,
      throughput: 1250000000,
      availability: 99.5,
    },
    duration: '35m',
    tags: ['burst', 'extreme', 'stress-test'],
  },

  burst_50x: {
    name: 'Burst 50x (25B Concurrent)',
    description: 'Maximum spike to 25 billion concurrent connections',
    config: {
      targetConnections: 25000000000,
      rampUpDuration: '15m',
      steadyStateDuration: '20m',
      rampDownDuration: '15m',
      queriesPerConnection: 5,
      queryInterval: '500',
      protocol: 'http2',
      vectorDimension: 768,
      queryPattern: 'burst',
      burstConfig: {
        multiplier: 50,
        duration: '1200000', // 20 minutes
        frequency: '3600000', // every hour
      },
    },
    k6Options: {
      scenarios: {
        maximum_burst: {
          executor: 'ramping-arrival-rate',
          startRate: 50000000,
          timeUnit: '1s',
          preAllocatedVUs: 2000000,
          maxVUs: 25000000,
          stages: [
            { duration: '15m', target: 2500000000 },
            { duration: '20m', target: 2500000000 },
            { duration: '15m', target: 50000000 },
          ],
        },
      },
    },
    expectedMetrics: {
      p99Latency: 200,
      errorRate: 1.0,
      throughput: 2500000000,
      availability: 99.0,
    },
    duration: '50m',
    tags: ['burst', 'maximum', 'stress-test'],
  },

  // ==================== FAILOVER SCENARIOS ====================

  regional_failover: {
    name: 'Regional Failover',
    description: 'Test failover when a region goes down',
    config: {
      targetConnections: 500000000,
      rampUpDuration: '10m',
      steadyStateDuration: '30m',
      rampDownDuration: '5m',
      queriesPerConnection: 100,
      queryInterval: '1000',
      protocol: 'http',
      vectorDimension: 768,
      queryPattern: 'uniform',
    },
    k6Options: {
      scenarios: {
        normal_traffic: {
          executor: 'constant-vus',
          vus: 500000,
          duration: '45m',
        },
        // Simulate region failure at 15 minutes
        region_failure: {
          executor: 'shared-iterations',
          vus: 1,
          iterations: 1,
          startTime: '15m',
          exec: 'simulateRegionFailure',
        },
      },
      thresholds: {
        'query_latency': ['p(99)<100'], // Allow higher latency during failover
        'error_rate': ['rate<0.01'], // Allow some errors during failover
      },
    },
    expectedMetrics: {
      p99Latency: 100,
      errorRate: 1.0, // Some errors expected during failover
      throughput: 45000000, // ~10% degradation
      availability: 99.0,
    },
    duration: '45m',
    tags: ['failover', 'disaster-recovery', 'high-availability'],
  },

  multi_region_failover: {
    name: 'Multi-Region Failover',
    description: 'Test failover when multiple regions go down',
    config: {
      targetConnections: 500000000,
      rampUpDuration: '10m',
      steadyStateDuration: '40m',
      rampDownDuration: '5m',
      queriesPerConnection: 100,
      queryInterval: '1000',
      protocol: 'http',
      vectorDimension: 768,
      queryPattern: 'uniform',
    },
    k6Options: {
      scenarios: {
        normal_traffic: {
          executor: 'constant-vus',
          vus: 500000,
          duration: '55m',
        },
        first_region_failure: {
          executor: 'shared-iterations',
          vus: 1,
          iterations: 1,
          startTime: '15m',
          exec: 'simulateRegionFailure',
        },
        second_region_failure: {
          executor: 'shared-iterations',
          vus: 1,
          iterations: 1,
          startTime: '30m',
          exec: 'simulateRegionFailure',
        },
      },
    },
    expectedMetrics: {
      p99Latency: 150,
      errorRate: 2.0,
      throughput: 40000000,
      availability: 98.0,
    },
    duration: '55m',
    tags: ['failover', 'multi-region', 'disaster-recovery'],
  },

  // ==================== COLD START SCENARIOS ====================

  cold_start: {
    name: 'Cold Start',
    description: 'Test scaling from 0 to full capacity',
    config: {
      targetConnections: 500000000,
      rampUpDuration: '30m',
      steadyStateDuration: '30m',
      rampDownDuration: '10m',
      queriesPerConnection: 50,
      queryInterval: '1000',
      protocol: 'http',
      vectorDimension: 768,
      queryPattern: 'uniform',
    },
    k6Options: {
      scenarios: {
        cold_start: {
          executor: 'ramping-vus',
          startVUs: 0,
          stages: [
            { duration: '30m', target: 500000 },
            { duration: '30m', target: 500000 },
            { duration: '10m', target: 0 },
          ],
        },
      },
      thresholds: {
        'query_latency': ['p(99)<100'], // Allow higher latency during warm-up
      },
    },
    expectedMetrics: {
      p99Latency: 100,
      errorRate: 0.1,
      throughput: 48000000,
      availability: 99.9,
    },
    duration: '70m',
    tags: ['cold-start', 'scaling', 'initialization'],
  },

  // ==================== MIXED WORKLOAD SCENARIOS ====================

  read_heavy: {
    name: 'Read-Heavy Workload',
    description: '95% reads, 5% writes',
    config: {
      targetConnections: 500000000,
      rampUpDuration: '20m',
      steadyStateDuration: '1h',
      rampDownDuration: '10m',
      queriesPerConnection: 200,
      queryInterval: '500',
      protocol: 'http',
      vectorDimension: 768,
      queryPattern: 'hotspot',
    },
    k6Options: {
      scenarios: {
        reads: {
          executor: 'constant-vus',
          vus: 475000, // 95%
          duration: '1h30m',
          exec: 'readQuery',
        },
        writes: {
          executor: 'constant-vus',
          vus: 25000, // 5%
          duration: '1h30m',
          exec: 'writeQuery',
        },
      },
    },
    expectedMetrics: {
      p99Latency: 50,
      errorRate: 0.01,
      throughput: 50000000,
      availability: 99.99,
    },
    duration: '1h50m',
    tags: ['workload', 'read-heavy', 'production-simulation'],
  },

  write_heavy: {
    name: 'Write-Heavy Workload',
    description: '30% reads, 70% writes',
    config: {
      targetConnections: 500000000,
      rampUpDuration: '20m',
      steadyStateDuration: '1h',
      rampDownDuration: '10m',
      queriesPerConnection: 100,
      queryInterval: '1000',
      protocol: 'http',
      vectorDimension: 768,
      queryPattern: 'uniform',
    },
    k6Options: {
      scenarios: {
        reads: {
          executor: 'constant-vus',
          vus: 150000, // 30%
          duration: '1h30m',
          exec: 'readQuery',
        },
        writes: {
          executor: 'constant-vus',
          vus: 350000, // 70%
          duration: '1h30m',
          exec: 'writeQuery',
        },
      },
    },
    expectedMetrics: {
      p99Latency: 80,
      errorRate: 0.05,
      throughput: 45000000,
      availability: 99.95,
    },
    duration: '1h50m',
    tags: ['workload', 'write-heavy', 'stress-test'],
  },

  balanced_workload: {
    name: 'Balanced Workload',
    description: '50% reads, 50% writes',
    config: {
      targetConnections: 500000000,
      rampUpDuration: '20m',
      steadyStateDuration: '1h',
      rampDownDuration: '10m',
      queriesPerConnection: 150,
      queryInterval: '750',
      protocol: 'http',
      vectorDimension: 768,
      queryPattern: 'zipfian',
    },
    k6Options: {
      scenarios: {
        reads: {
          executor: 'constant-vus',
          vus: 250000,
          duration: '1h30m',
          exec: 'readQuery',
        },
        writes: {
          executor: 'constant-vus',
          vus: 250000,
          duration: '1h30m',
          exec: 'writeQuery',
        },
      },
    },
    expectedMetrics: {
      p99Latency: 60,
      errorRate: 0.02,
      throughput: 48000000,
      availability: 99.98,
    },
    duration: '1h50m',
    tags: ['workload', 'balanced', 'production-simulation'],
  },

  // ==================== REAL-WORLD SCENARIOS ====================

  world_cup: {
    name: 'World Cup Scenario',
    description: 'Predictable spike with geographic concentration',
    config: {
      targetConnections: 5000000000,
      rampUpDuration: '15m',
      steadyStateDuration: '2h',
      rampDownDuration: '30m',
      queriesPerConnection: 500,
      queryInterval: '200',
      protocol: 'ws',
      vectorDimension: 768,
      queryPattern: 'burst',
      burstConfig: {
        multiplier: 10,
        duration: '5400000', // 90 minutes (match duration)
        frequency: '7200000', // every 2 hours
      },
    },
    k6Options: {
      scenarios: {
        normal_traffic: {
          executor: 'constant-vus',
          vus: 500000,
          duration: '3h',
        },
        match_traffic: {
          executor: 'ramping-vus',
          startTime: '30m',
          startVUs: 500000,
          stages: [
            { duration: '15m', target: 5000000 }, // Match starts
            { duration: '90m', target: 5000000 }, // Match duration
            { duration: '15m', target: 500000 },  // Match ends
          ],
        },
      },
    },
    expectedMetrics: {
      p99Latency: 100,
      errorRate: 0.1,
      throughput: 500000000,
      availability: 99.9,
    },
    regions: ['europe-west1', 'europe-west2', 'europe-north1'], // Focus on Europe
    duration: '3h',
    tags: ['real-world', 'predictable-spike', 'geographic'],
  },

  black_friday: {
    name: 'Black Friday Scenario',
    description: 'Sustained high load with periodic spikes',
    config: {
      targetConnections: 2000000000,
      rampUpDuration: '1h',
      steadyStateDuration: '12h',
      rampDownDuration: '1h',
      queriesPerConnection: 1000,
      queryInterval: '100',
      protocol: 'http2',
      vectorDimension: 768,
      queryPattern: 'burst',
      burstConfig: {
        multiplier: 5,
        duration: '3600000', // 1 hour spikes
        frequency: '7200000', // every 2 hours
      },
    },
    k6Options: {
      scenarios: {
        baseline: {
          executor: 'constant-vus',
          vus: 2000000,
          duration: '14h',
        },
        hourly_spikes: {
          executor: 'ramping-vus',
          startVUs: 0,
          stages: [
            // Repeat spike pattern every 2 hours
            { duration: '1h', target: 10000000 },
            { duration: '1h', target: 0 },
          ],
        },
      },
    },
    expectedMetrics: {
      p99Latency: 80,
      errorRate: 0.05,
      throughput: 200000000,
      availability: 99.95,
    },
    duration: '14h',
    tags: ['real-world', 'sustained-high-load', 'retail'],
  },
};

// Scenario groups for batch testing
export const SCENARIO_GROUPS = {
  quick_validation: ['baseline_100m'],
  standard_suite: ['baseline_500m', 'burst_10x', 'read_heavy'],
  stress_suite: ['burst_25x', 'burst_50x', 'write_heavy'],
  reliability_suite: ['regional_failover', 'multi_region_failover', 'cold_start'],
  full_suite: Object.keys(SCENARIOS),
};

// Helper functions
export function getScenario(name: string): Scenario | undefined {
  return SCENARIOS[name];
}

export function getScenariosByTag(tag: string): Scenario[] {
  return Object.values(SCENARIOS).filter(s => s.tags.includes(tag));
}

export function getScenarioGroup(group: keyof typeof SCENARIO_GROUPS): string[] {
  return SCENARIO_GROUPS[group] || [];
}

export function estimateCost(scenario: Scenario): number {
  // Rough cost estimation based on GCP pricing
  // $0.10 per million queries + infrastructure costs
  const totalQueries = scenario.config.targetConnections * scenario.config.queriesPerConnection;
  const queryCost = (totalQueries / 1000000) * 0.10;

  // Infrastructure cost (rough estimate)
  const durationHours = parseDuration(scenario.duration);
  const infraCost = durationHours * 1000; // $1000/hour for infrastructure

  return queryCost + infraCost;
}

function parseDuration(duration: string): number {
  const match = duration.match(/(\d+)([hm])/);
  if (!match) return 0;
  const [, num, unit] = match;
  return unit === 'h' ? parseInt(num) : parseInt(num) / 60;
}

export default SCENARIOS;
