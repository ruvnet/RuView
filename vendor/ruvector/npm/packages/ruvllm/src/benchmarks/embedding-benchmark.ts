/**
 * Embedding Quality Benchmark for RuvLTRA Models
 *
 * Tests embedding quality for Claude Code use cases:
 * - Code similarity detection
 * - Task clustering
 * - Semantic search accuracy
 */

export interface EmbeddingPair {
  id: string;
  text1: string;
  text2: string;
  similarity: 'high' | 'medium' | 'low' | 'none';
  category: string;
}

export interface EmbeddingResult {
  pairId: string;
  expectedSimilarity: string;
  computedScore: number;
  correct: boolean;
  latencyMs: number;
}

export interface ClusterTestCase {
  id: string;
  items: string[];
  expectedCluster: string;
}

export interface EmbeddingBenchmarkResults {
  // Similarity detection
  similarityAccuracy: number;
  similarityByCategory: Record<string, number>;
  avgSimilarityLatencyMs: number;

  // Clustering quality
  clusterPurity: number;
  silhouetteScore: number;

  // Search quality
  searchMRR: number; // Mean Reciprocal Rank
  searchNDCG: number; // Normalized Discounted Cumulative Gain

  // Details
  similarityResults: EmbeddingResult[];
  totalPairs: number;
}

/**
 * Ground truth similarity pairs for testing
 * Tests whether embeddings correctly capture semantic similarity
 */
export const SIMILARITY_TEST_PAIRS: EmbeddingPair[] = [
  // === HIGH SIMILARITY (same concept, different wording) ===
  { id: 'H001', text1: 'implement user authentication', text2: 'create login functionality', similarity: 'high', category: 'code-task' },
  { id: 'H002', text1: 'write unit tests for the API', text2: 'create test cases for REST endpoints', similarity: 'high', category: 'code-task' },
  { id: 'H003', text1: 'fix the null pointer exception', text2: 'resolve the NullPointerException bug', similarity: 'high', category: 'debugging' },
  { id: 'H004', text1: 'optimize database queries', text2: 'improve SQL query performance', similarity: 'high', category: 'performance' },
  { id: 'H005', text1: 'deploy to production', text2: 'release to prod environment', similarity: 'high', category: 'devops' },
  { id: 'H006', text1: 'refactor the legacy code', text2: 'restructure old codebase', similarity: 'high', category: 'refactoring' },
  { id: 'H007', text1: 'add error handling', text2: 'implement exception handling', similarity: 'high', category: 'code-task' },
  { id: 'H008', text1: 'create REST API endpoint', text2: 'build HTTP API route', similarity: 'high', category: 'code-task' },
  { id: 'H009', text1: 'check for SQL injection', text2: 'audit for SQLi vulnerabilities', similarity: 'high', category: 'security' },
  { id: 'H010', text1: 'document the API', text2: 'write API documentation', similarity: 'high', category: 'documentation' },

  // Code snippets - same functionality
  { id: 'H011', text1: 'function add(a, b) { return a + b; }', text2: 'const sum = (x, y) => x + y;', similarity: 'high', category: 'code-snippet' },
  { id: 'H012', text1: 'for (let i = 0; i < arr.length; i++)', text2: 'arr.forEach((item, index) => {})', similarity: 'high', category: 'code-snippet' },
  { id: 'H013', text1: 'async function fetchData() { await fetch(url); }', text2: 'const getData = async () => { await axios.get(url); }', similarity: 'high', category: 'code-snippet' },

  // === MEDIUM SIMILARITY (related but different) ===
  { id: 'M001', text1: 'implement user authentication', text2: 'create user registration', similarity: 'medium', category: 'code-task' },
  { id: 'M002', text1: 'write unit tests', text2: 'write integration tests', similarity: 'medium', category: 'testing' },
  { id: 'M003', text1: 'fix the bug in checkout', text2: 'debug the payment flow', similarity: 'medium', category: 'debugging' },
  { id: 'M004', text1: 'optimize frontend performance', text2: 'improve backend response time', similarity: 'medium', category: 'performance' },
  { id: 'M005', text1: 'deploy to staging', text2: 'deploy to production', similarity: 'medium', category: 'devops' },
  { id: 'M006', text1: 'React component', text2: 'Vue component', similarity: 'medium', category: 'code-snippet' },
  { id: 'M007', text1: 'PostgreSQL query', text2: 'MySQL query', similarity: 'medium', category: 'code-snippet' },
  { id: 'M008', text1: 'REST API', text2: 'GraphQL API', similarity: 'medium', category: 'code-task' },
  { id: 'M009', text1: 'Node.js server', text2: 'Python Flask server', similarity: 'medium', category: 'code-snippet' },
  { id: 'M010', text1: 'add caching layer', text2: 'implement rate limiting', similarity: 'medium', category: 'performance' },

  // === LOW SIMILARITY (same domain, different task) ===
  { id: 'L001', text1: 'implement authentication', text2: 'write documentation', similarity: 'low', category: 'code-task' },
  { id: 'L002', text1: 'fix bug', text2: 'add new feature', similarity: 'low', category: 'code-task' },
  { id: 'L003', text1: 'optimize query', text2: 'review pull request', similarity: 'low', category: 'mixed' },
  { id: 'L004', text1: 'deploy application', text2: 'design architecture', similarity: 'low', category: 'mixed' },
  { id: 'L005', text1: 'frontend React code', text2: 'backend database migration', similarity: 'low', category: 'code-snippet' },
  { id: 'L006', text1: 'security audit', text2: 'performance benchmark', similarity: 'low', category: 'mixed' },
  { id: 'L007', text1: 'write unit tests', text2: 'create CI/CD pipeline', similarity: 'low', category: 'mixed' },
  { id: 'L008', text1: 'CSS styling', text2: 'database schema', similarity: 'low', category: 'code-snippet' },

  // === NO SIMILARITY (unrelated) ===
  { id: 'N001', text1: 'implement user login', text2: 'the weather is nice today', similarity: 'none', category: 'unrelated' },
  { id: 'N002', text1: 'fix JavaScript bug', text2: 'recipe for chocolate cake', similarity: 'none', category: 'unrelated' },
  { id: 'N003', text1: 'deploy Kubernetes cluster', text2: 'book a flight to Paris', similarity: 'none', category: 'unrelated' },
  { id: 'N004', text1: 'optimize SQL query', text2: 'learn to play guitar', similarity: 'none', category: 'unrelated' },
  { id: 'N005', text1: 'const x = 42;', text2: 'roses are red violets are blue', similarity: 'none', category: 'unrelated' },
];

/**
 * Search relevance test cases
 * Query + documents with relevance scores
 */
export interface SearchTestCase {
  id: string;
  query: string;
  documents: { text: string; relevance: number }[]; // relevance: 0-3 (0=irrelevant, 3=highly relevant)
}

export const SEARCH_TEST_CASES: SearchTestCase[] = [
  {
    id: 'S001',
    query: 'how to implement user authentication in Node.js',
    documents: [
      { text: 'Implementing JWT authentication in Express.js with passport', relevance: 3 },
      { text: 'Node.js login system with bcrypt password hashing', relevance: 3 },
      { text: 'Building a React login form component', relevance: 2 },
      { text: 'PostgreSQL user table schema design', relevance: 1 },
      { text: 'How to deploy Docker containers', relevance: 0 },
    ],
  },
  {
    id: 'S002',
    query: 'fix memory leak in JavaScript',
    documents: [
      { text: 'Debugging memory leaks with Chrome DevTools heap snapshots', relevance: 3 },
      { text: 'Common causes of memory leaks in Node.js applications', relevance: 3 },
      { text: 'JavaScript garbage collection explained', relevance: 2 },
      { text: 'Optimizing React component re-renders', relevance: 1 },
      { text: 'CSS flexbox layout tutorial', relevance: 0 },
    ],
  },
  {
    id: 'S003',
    query: 'database migration best practices',
    documents: [
      { text: 'Schema migration strategies for zero-downtime deployments', relevance: 3 },
      { text: 'Using Prisma migrate for PostgreSQL schema changes', relevance: 3 },
      { text: 'Database backup and recovery procedures', relevance: 2 },
      { text: 'SQL query optimization techniques', relevance: 1 },
      { text: 'React state management with Redux', relevance: 0 },
    ],
  },
  {
    id: 'S004',
    query: 'write unit tests for React components',
    documents: [
      { text: 'Testing React components with Jest and React Testing Library', relevance: 3 },
      { text: 'Snapshot testing for UI components', relevance: 3 },
      { text: 'Mocking API calls in frontend tests', relevance: 2 },
      { text: 'End-to-end testing with Cypress', relevance: 1 },
      { text: 'Kubernetes pod configuration', relevance: 0 },
    ],
  },
  {
    id: 'S005',
    query: 'optimize API response time',
    documents: [
      { text: 'Implementing Redis caching for API endpoints', relevance: 3 },
      { text: 'Database query optimization with indexes', relevance: 3 },
      { text: 'Using CDN for static asset delivery', relevance: 2 },
      { text: 'Load balancing strategies for microservices', relevance: 2 },
      { text: 'Writing clean JavaScript code', relevance: 0 },
    ],
  },
];

/**
 * Cluster test cases - items that should cluster together
 */
export const CLUSTER_TEST_CASES: ClusterTestCase[] = [
  {
    id: 'CL001',
    expectedCluster: 'authentication',
    items: [
      'implement user login',
      'add JWT token validation',
      'create password reset flow',
      'implement OAuth integration',
      'add two-factor authentication',
    ],
  },
  {
    id: 'CL002',
    expectedCluster: 'testing',
    items: [
      'write unit tests',
      'add integration tests',
      'create E2E test suite',
      'improve test coverage',
      'add snapshot tests',
    ],
  },
  {
    id: 'CL003',
    expectedCluster: 'database',
    items: [
      'optimize SQL queries',
      'add database indexes',
      'create migration script',
      'implement connection pooling',
      'design schema for users table',
    ],
  },
  {
    id: 'CL004',
    expectedCluster: 'frontend',
    items: [
      'build React component',
      'add CSS styling',
      'implement responsive design',
      'create form validation',
      'add loading spinner',
    ],
  },
  {
    id: 'CL005',
    expectedCluster: 'devops',
    items: [
      'set up CI/CD pipeline',
      'configure Kubernetes deployment',
      'create Docker container',
      'add monitoring alerts',
      'implement auto-scaling',
    ],
  },
];

/**
 * Expected similarity score ranges
 */
const SIMILARITY_THRESHOLDS = {
  high: { min: 0.7, max: 1.0 },
  medium: { min: 0.4, max: 0.7 },
  low: { min: 0.2, max: 0.4 },
  none: { min: 0.0, max: 0.2 },
};

/**
 * Check if computed similarity matches expected category
 */
export function isCorrectSimilarity(
  expected: 'high' | 'medium' | 'low' | 'none',
  computed: number
): boolean {
  const threshold = SIMILARITY_THRESHOLDS[expected];
  return computed >= threshold.min && computed <= threshold.max;
}

/**
 * Calculate Mean Reciprocal Rank for search results
 */
export function calculateMRR(
  rankings: { relevant: boolean }[][]
): number {
  let sumRR = 0;
  for (const ranking of rankings) {
    const firstRelevantIdx = ranking.findIndex(r => r.relevant);
    if (firstRelevantIdx >= 0) {
      sumRR += 1 / (firstRelevantIdx + 1);
    }
  }
  return sumRR / rankings.length;
}

/**
 * Calculate NDCG for search results
 */
export function calculateNDCG(
  results: { relevance: number }[],
  idealOrder: { relevance: number }[]
): number {
  const dcg = results.reduce((sum, r, i) => {
    return sum + (Math.pow(2, r.relevance) - 1) / Math.log2(i + 2);
  }, 0);

  const idcg = idealOrder.reduce((sum, r, i) => {
    return sum + (Math.pow(2, r.relevance) - 1) / Math.log2(i + 2);
  }, 0);

  return idcg > 0 ? dcg / idcg : 0;
}

/**
 * Calculate silhouette score for clustering
 */
export function calculateSilhouette(
  embeddings: number[][],
  labels: number[]
): number {
  // Simplified silhouette calculation
  const n = embeddings.length;
  if (n < 2) return 0;

  let totalSilhouette = 0;

  for (let i = 0; i < n; i++) {
    const cluster = labels[i];

    // Calculate mean intra-cluster distance (a)
    let intraSum = 0;
    let intraCount = 0;
    for (let j = 0; j < n; j++) {
      if (i !== j && labels[j] === cluster) {
        intraSum += euclideanDistance(embeddings[i], embeddings[j]);
        intraCount++;
      }
    }
    const a = intraCount > 0 ? intraSum / intraCount : 0;

    // Calculate min mean inter-cluster distance (b)
    const otherClusters = [...new Set(labels)].filter(c => c !== cluster);
    let minInterMean = Infinity;

    for (const otherCluster of otherClusters) {
      let interSum = 0;
      let interCount = 0;
      for (let j = 0; j < n; j++) {
        if (labels[j] === otherCluster) {
          interSum += euclideanDistance(embeddings[i], embeddings[j]);
          interCount++;
        }
      }
      if (interCount > 0) {
        minInterMean = Math.min(minInterMean, interSum / interCount);
      }
    }
    const b = minInterMean === Infinity ? 0 : minInterMean;

    // Silhouette for this point
    const s = Math.max(a, b) > 0 ? (b - a) / Math.max(a, b) : 0;
    totalSilhouette += s;
  }

  return totalSilhouette / n;
}

function euclideanDistance(a: number[], b: number[]): number {
  let sum = 0;
  for (let i = 0; i < a.length; i++) {
    sum += Math.pow(a[i] - b[i], 2);
  }
  return Math.sqrt(sum);
}

/**
 * Run the embedding benchmark
 */
export function runEmbeddingBenchmark(
  embedder: (text: string) => number[],
  similarityFn: (a: number[], b: number[]) => number
): EmbeddingBenchmarkResults {
  const similarityResults: EmbeddingResult[] = [];
  const latencies: number[] = [];

  // Test similarity pairs
  for (const pair of SIMILARITY_TEST_PAIRS) {
    const start = performance.now();
    const emb1 = embedder(pair.text1);
    const emb2 = embedder(pair.text2);
    const score = similarityFn(emb1, emb2);
    const latencyMs = performance.now() - start;

    latencies.push(latencyMs);

    similarityResults.push({
      pairId: pair.id,
      expectedSimilarity: pair.similarity,
      computedScore: score,
      correct: isCorrectSimilarity(pair.similarity, score),
      latencyMs,
    });
  }

  // Calculate similarity accuracy
  const correctSimilarity = similarityResults.filter(r => r.correct).length;
  const similarityAccuracy = correctSimilarity / similarityResults.length;

  // Accuracy by category
  const categories = [...new Set(SIMILARITY_TEST_PAIRS.map(p => p.category))];
  const similarityByCategory: Record<string, number> = {};
  for (const cat of categories) {
    const catResults = similarityResults.filter(
      (r, i) => SIMILARITY_TEST_PAIRS[i].category === cat
    );
    similarityByCategory[cat] = catResults.filter(r => r.correct).length / catResults.length;
  }

  // Test search quality (MRR and NDCG)
  const searchRankings: { relevant: boolean }[][] = [];
  let totalNDCG = 0;

  for (const testCase of SEARCH_TEST_CASES) {
    const queryEmb = embedder(testCase.query);
    const docScores = testCase.documents.map(doc => ({
      ...doc,
      score: similarityFn(queryEmb, embedder(doc.text)),
    }));

    // Sort by computed score
    const sorted = [...docScores].sort((a, b) => b.score - a.score);

    // For MRR
    searchRankings.push(sorted.map(d => ({ relevant: d.relevance >= 2 })));

    // For NDCG
    const idealOrder = [...testCase.documents].sort((a, b) => b.relevance - a.relevance);
    totalNDCG += calculateNDCG(sorted, idealOrder);
  }

  const searchMRR = calculateMRR(searchRankings);
  const searchNDCG = totalNDCG / SEARCH_TEST_CASES.length;

  // Test clustering
  const allClusterItems: { text: string; cluster: number }[] = [];
  CLUSTER_TEST_CASES.forEach((tc, clusterIdx) => {
    tc.items.forEach(item => {
      allClusterItems.push({ text: item, cluster: clusterIdx });
    });
  });

  const clusterEmbeddings = allClusterItems.map(item => embedder(item.text));
  const clusterLabels = allClusterItems.map(item => item.cluster);
  const silhouetteScore = calculateSilhouette(clusterEmbeddings, clusterLabels);

  // Calculate cluster purity (how well items stay in their expected cluster)
  // Using simple nearest-neighbor classification
  let correctCluster = 0;
  for (let i = 0; i < clusterEmbeddings.length; i++) {
    let nearestIdx = -1;
    let nearestDist = Infinity;
    for (let j = 0; j < clusterEmbeddings.length; j++) {
      if (i !== j) {
        const dist = euclideanDistance(clusterEmbeddings[i], clusterEmbeddings[j]);
        if (dist < nearestDist) {
          nearestDist = dist;
          nearestIdx = j;
        }
      }
    }
    if (nearestIdx >= 0 && clusterLabels[nearestIdx] === clusterLabels[i]) {
      correctCluster++;
    }
  }
  const clusterPurity = correctCluster / clusterEmbeddings.length;

  return {
    similarityAccuracy,
    similarityByCategory,
    avgSimilarityLatencyMs: latencies.reduce((a, b) => a + b, 0) / latencies.length,
    clusterPurity,
    silhouetteScore,
    searchMRR,
    searchNDCG,
    similarityResults,
    totalPairs: similarityResults.length,
  };
}

/**
 * Format embedding benchmark results for display
 */
export function formatEmbeddingResults(results: EmbeddingBenchmarkResults): string {
  const lines: string[] = [];

  lines.push('');
  lines.push('╔══════════════════════════════════════════════════════════════╗');
  lines.push('║             EMBEDDING BENCHMARK RESULTS                      ║');
  lines.push('╠══════════════════════════════════════════════════════════════╣');
  lines.push(`║  Similarity Detection: ${(results.similarityAccuracy * 100).toFixed(1)}%`.padEnd(63) + '║');
  lines.push('╠══════════════════════════════════════════════════════════════╣');
  lines.push('║  By Category:                                                ║');

  for (const [cat, acc] of Object.entries(results.similarityByCategory).sort((a, b) => b[1] - a[1])) {
    const bar = '█'.repeat(Math.floor(acc * 20)) + '░'.repeat(20 - Math.floor(acc * 20));
    lines.push(`║    ${cat.padEnd(18)} [${bar}] ${(acc * 100).toFixed(0).padStart(3)}%  ║`);
  }

  lines.push('╠══════════════════════════════════════════════════════════════╣');
  lines.push('║  Clustering Quality:                                         ║');
  lines.push(`║    Cluster Purity:    ${(results.clusterPurity * 100).toFixed(1)}%`.padEnd(63) + '║');
  lines.push(`║    Silhouette Score:  ${results.silhouetteScore.toFixed(3)}`.padEnd(63) + '║');
  lines.push('╠══════════════════════════════════════════════════════════════╣');
  lines.push('║  Search Quality:                                             ║');
  lines.push(`║    MRR (Mean Reciprocal Rank):  ${results.searchMRR.toFixed(3)}`.padEnd(63) + '║');
  lines.push(`║    NDCG:                        ${results.searchNDCG.toFixed(3)}`.padEnd(63) + '║');
  lines.push('╠══════════════════════════════════════════════════════════════╣');
  lines.push(`║  Avg Latency: ${results.avgSimilarityLatencyMs.toFixed(2)}ms per pair`.padEnd(63) + '║');
  lines.push('╚══════════════════════════════════════════════════════════════╝');

  // Quality assessment
  lines.push('');
  lines.push('Quality Assessment:');

  if (results.similarityAccuracy >= 0.8) {
    lines.push('  ✓ Similarity detection: EXCELLENT (≥80%)');
  } else if (results.similarityAccuracy >= 0.6) {
    lines.push('  ~ Similarity detection: GOOD (60-80%)');
  } else {
    lines.push('  ✗ Similarity detection: NEEDS IMPROVEMENT (<60%)');
  }

  if (results.searchMRR >= 0.8) {
    lines.push('  ✓ Search quality (MRR): EXCELLENT (≥0.8)');
  } else if (results.searchMRR >= 0.5) {
    lines.push('  ~ Search quality (MRR): ACCEPTABLE (0.5-0.8)');
  } else {
    lines.push('  ✗ Search quality (MRR): NEEDS IMPROVEMENT (<0.5)');
  }

  if (results.clusterPurity >= 0.8) {
    lines.push('  ✓ Clustering: EXCELLENT (≥80% purity)');
  } else if (results.clusterPurity >= 0.6) {
    lines.push('  ~ Clustering: ACCEPTABLE (60-80% purity)');
  } else {
    lines.push('  ✗ Clustering: NEEDS IMPROVEMENT (<60% purity)');
  }

  return lines.join('\n');
}

export default {
  SIMILARITY_TEST_PAIRS,
  SEARCH_TEST_CASES,
  CLUSTER_TEST_CASES,
  runEmbeddingBenchmark,
  formatEmbeddingResults,
  isCorrectSimilarity,
  calculateMRR,
  calculateNDCG,
};
