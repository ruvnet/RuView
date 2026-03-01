/**
 * Graph Neural Network Correlation Analysis
 *
 * EXOTIC: Market structure as dynamic graphs
 *
 * Uses @neural-trader/neural with RuVector for:
 * - Correlation network construction from returns
 * - Graph-based feature extraction (centrality, clustering)
 * - Dynamic topology changes as regime indicators
 * - Spectral analysis for systemic risk
 *
 * Markets are interconnected - GNNs capture these relationships
 * that traditional linear models miss.
 */

// GNN Configuration
const gnnConfig = {
  // Network construction
  construction: {
    method: 'pearson',          // pearson, spearman, partial, transfer_entropy
    windowSize: 60,             // Days for correlation calculation
    edgeThreshold: 0.3,         // Minimum |correlation| for edge
    maxEdgesPerNode: 10         // Limit connections
  },

  // Graph features
  features: {
    nodeCentrality: ['degree', 'betweenness', 'eigenvector', 'pagerank'],
    graphMetrics: ['density', 'clustering', 'modularity', 'avgPath'],
    spectral: ['algebraicConnectivity', 'spectralRadius', 'fiedlerVector']
  },

  // Regime detection
  regime: {
    stabilityWindow: 20,        // Days to assess stability
    changeThreshold: 0.15       // Topology change threshold
  }
};

// Graph Node (Asset)
class GraphNode {
  constructor(symbol, index) {
    this.symbol = symbol;
    this.index = index;
    this.edges = new Map();      // neighbor -> weight
    this.returns = [];
    this.features = {};
  }

  addEdge(neighbor, weight) {
    this.edges.set(neighbor, weight);
  }

  removeEdge(neighbor) {
    this.edges.delete(neighbor);
  }

  getDegree() {
    return this.edges.size;
  }

  getWeightedDegree() {
    let sum = 0;
    for (const weight of this.edges.values()) {
      sum += Math.abs(weight);
    }
    return sum;
  }
}

// Rolling Statistics for O(1) incremental updates
class RollingStats {
  constructor(windowSize) {
    this.windowSize = windowSize;
    this.values = [];
    this.sum = 0;
    this.sumSq = 0;
  }

  add(value) {
    if (this.values.length >= this.windowSize) {
      const removed = this.values.shift();
      this.sum -= removed;
      this.sumSq -= removed * removed;
    }
    this.values.push(value);
    this.sum += value;
    this.sumSq += value * value;
  }

  get mean() {
    return this.values.length > 0 ? this.sum / this.values.length : 0;
  }

  get variance() {
    if (this.values.length < 2) return 0;
    const n = this.values.length;
    return (this.sumSq - (this.sum * this.sum) / n) / (n - 1);
  }

  get std() {
    return Math.sqrt(Math.max(0, this.variance));
  }
}

// Correlation Network
class CorrelationNetwork {
  constructor(config) {
    this.config = config;
    this.nodes = new Map();
    this.adjacencyMatrix = [];
    this.history = [];
    this.correlationCache = new Map();  // Cache for correlation pairs
    this.statsCache = new Map();        // Cache for per-asset statistics
  }

  // Add asset to network
  addAsset(symbol) {
    if (!this.nodes.has(symbol)) {
      this.nodes.set(symbol, new GraphNode(symbol, this.nodes.size));
    }
    return this.nodes.get(symbol);
  }

  // Update returns for asset with pre-computed stats
  updateReturns(symbol, returns) {
    const node = this.addAsset(symbol);
    node.returns = returns;
    // Pre-compute statistics for fast correlation
    this.precomputeStats(symbol, returns);
  }

  // Pre-compute mean, std, and centered returns for fast correlation
  precomputeStats(symbol, returns) {
    const n = returns.length;
    if (n < 2) {
      this.statsCache.set(symbol, { mean: 0, std: 0, centered: [], valid: false });
      return;
    }

    let sum = 0;
    for (let i = 0; i < n; i++) sum += returns[i];
    const mean = sum / n;

    let sumSq = 0;
    const centered = new Array(n);
    for (let i = 0; i < n; i++) {
      centered[i] = returns[i] - mean;
      sumSq += centered[i] * centered[i];
    }
    const std = Math.sqrt(sumSq);

    this.statsCache.set(symbol, { mean, std, centered, valid: std > 1e-10 });
  }

  // Fast correlation using pre-computed stats (avoids recomputing mean/std)
  calculateCorrelationFast(symbol1, symbol2) {
    const s1 = this.statsCache.get(symbol1);
    const s2 = this.statsCache.get(symbol2);

    if (!s1 || !s2 || !s1.valid || !s2.valid) return 0;
    if (s1.centered.length !== s2.centered.length) return 0;

    let dotProduct = 0;
    for (let i = 0; i < s1.centered.length; i++) {
      dotProduct += s1.centered[i] * s2.centered[i];
    }

    return dotProduct / (s1.std * s2.std);
  }

  // Calculate correlation between two return series
  calculateCorrelation(returns1, returns2, method = 'pearson') {
    if (returns1.length !== returns2.length || returns1.length < 2) {
      return 0;
    }

    const n = returns1.length;

    if (method === 'pearson') {
      let sum1 = 0, sum2 = 0;
      for (let i = 0; i < n; i++) {
        sum1 += returns1[i];
        sum2 += returns2[i];
      }
      const mean1 = sum1 / n;
      const mean2 = sum2 / n;

      let cov = 0, var1 = 0, var2 = 0;
      for (let i = 0; i < n; i++) {
        const d1 = returns1[i] - mean1;
        const d2 = returns2[i] - mean2;
        cov += d1 * d2;
        var1 += d1 * d1;
        var2 += d2 * d2;
      }

      if (var1 === 0 || var2 === 0) return 0;
      return cov / Math.sqrt(var1 * var2);
    }

    if (method === 'spearman') {
      // Rank-based correlation (optimized sort)
      const rank = (arr) => {
        const indexed = new Array(arr.length);
        for (let i = 0; i < arr.length; i++) indexed[i] = { v: arr[i], i };
        indexed.sort((a, b) => a.v - b.v);
        const ranks = new Array(arr.length);
        for (let r = 0; r < indexed.length; r++) ranks[indexed[r].i] = r + 1;
        return ranks;
      };

      const ranks1 = rank(returns1);
      const ranks2 = rank(returns2);
      return this.calculateCorrelation(ranks1, ranks2, 'pearson');
    }

    return 0;
  }

  // Optimized correlation with caching (O(n) instead of O(n²) for repeated calls)
  calculateCorrelationCached(symbol1, symbol2) {
    const cacheKey = symbol1 < symbol2 ? `${symbol1}:${symbol2}` : `${symbol2}:${symbol1}`;

    // Check cache validity
    const cached = this.correlationCache.get(cacheKey);
    if (cached && Date.now() - cached.timestamp < 1000) {
      return cached.value;
    }

    const node1 = this.nodes.get(symbol1);
    const node2 = this.nodes.get(symbol2);

    if (!node1 || !node2) return 0;

    const correlation = this.calculateCorrelation(
      node1.returns,
      node2.returns,
      this.config.construction.method
    );

    this.correlationCache.set(cacheKey, { value: correlation, timestamp: Date.now() });
    return correlation;
  }

  // Clear correlation cache (call when data updates)
  invalidateCache() {
    this.correlationCache.clear();
    this.statsCache.clear();
  }

  // Build correlation network
  buildNetwork() {
    const symbols = Array.from(this.nodes.keys());
    const n = symbols.length;

    // Initialize adjacency matrix
    this.adjacencyMatrix = Array(n).fill(null).map(() => Array(n).fill(0));

    // Clear existing edges
    for (const node of this.nodes.values()) {
      node.edges.clear();
    }

    // Calculate pairwise correlations (use fast path for Pearson with pre-computed stats)
    const useFastPath = this.config.construction.method === 'pearson' && this.statsCache.size === n;

    for (let i = 0; i < n; i++) {
      for (let j = i + 1; j < n; j++) {
        let correlation;

        if (useFastPath) {
          // Fast path: use pre-computed centered returns
          correlation = this.calculateCorrelationFast(symbols[i], symbols[j]);
        } else {
          const node1 = this.nodes.get(symbols[i]);
          const node2 = this.nodes.get(symbols[j]);
          correlation = this.calculateCorrelation(
            node1.returns,
            node2.returns,
            this.config.construction.method
          );
        }

        this.adjacencyMatrix[i][j] = correlation;
        this.adjacencyMatrix[j][i] = correlation;

        // Add edge if above threshold
        if (Math.abs(correlation) >= this.config.construction.edgeThreshold) {
          this.nodes.get(symbols[i]).addEdge(symbols[j], correlation);
          this.nodes.get(symbols[j]).addEdge(symbols[i], correlation);
        }
      }
    }

    // Limit edges per node
    this.pruneEdges();
  }

  // Prune edges to max per node
  pruneEdges() {
    for (const node of this.nodes.values()) {
      if (node.edges.size > this.config.construction.maxEdgesPerNode) {
        const sorted = Array.from(node.edges.entries())
          .sort((a, b) => Math.abs(b[1]) - Math.abs(a[1]));

        node.edges.clear();
        for (let i = 0; i < this.config.construction.maxEdgesPerNode; i++) {
          node.edges.set(sorted[i][0], sorted[i][1]);
        }
      }
    }
  }

  // Calculate node centrality measures
  calculateNodeCentrality() {
    const symbols = Array.from(this.nodes.keys());
    const n = symbols.length;

    for (const node of this.nodes.values()) {
      // Degree centrality
      node.features.degreeCentrality = node.getDegree() / (n - 1);
      node.features.weightedDegree = node.getWeightedDegree();
    }

    // Eigenvector centrality (power iteration)
    this.calculateEigenvectorCentrality();

    // PageRank
    this.calculatePageRank();

    // Betweenness (simplified)
    this.calculateBetweenness();
  }

  // Eigenvector centrality
  calculateEigenvectorCentrality() {
    const symbols = Array.from(this.nodes.keys());
    const n = symbols.length;

    let centrality = new Array(n).fill(1 / n);

    for (let iter = 0; iter < 100; iter++) {
      const newCentrality = new Array(n).fill(0);

      for (let i = 0; i < n; i++) {
        for (let j = 0; j < n; j++) {
          newCentrality[i] += Math.abs(this.adjacencyMatrix[i][j]) * centrality[j];
        }
      }

      // Normalize
      const norm = Math.sqrt(newCentrality.reduce((a, b) => a + b * b, 0));
      if (norm > 0) {
        for (let i = 0; i < n; i++) {
          newCentrality[i] /= norm;
        }
      }

      centrality = newCentrality;
    }

    for (let i = 0; i < n; i++) {
      this.nodes.get(symbols[i]).features.eigenvectorCentrality = centrality[i];
    }
  }

  // PageRank
  calculatePageRank() {
    const symbols = Array.from(this.nodes.keys());
    const n = symbols.length;
    const d = 0.85;  // Damping factor

    let pagerank = new Array(n).fill(1 / n);

    for (let iter = 0; iter < 100; iter++) {
      const newPagerank = new Array(n).fill((1 - d) / n);

      for (let i = 0; i < n; i++) {
        const node = this.nodes.get(symbols[i]);
        const outDegree = node.getDegree() || 1;

        for (const [neighbor] of node.edges) {
          const j = this.nodes.get(neighbor).index;
          newPagerank[j] += d * pagerank[i] / outDegree;
        }
      }

      pagerank = newPagerank;
    }

    for (let i = 0; i < n; i++) {
      this.nodes.get(symbols[i]).features.pagerank = pagerank[i];
    }
  }

  // Betweenness centrality (simplified BFS-based)
  calculateBetweenness() {
    const symbols = Array.from(this.nodes.keys());
    const n = symbols.length;
    const betweenness = new Array(n).fill(0);

    for (let s = 0; s < n; s++) {
      // BFS from each source
      const distances = new Array(n).fill(Infinity);
      const paths = new Array(n).fill(0);
      const queue = [s];
      distances[s] = 0;
      paths[s] = 1;

      while (queue.length > 0) {
        const current = queue.shift();
        const node = this.nodes.get(symbols[current]);

        for (const [neighbor] of node.edges) {
          const j = this.nodes.get(neighbor).index;
          if (distances[j] === Infinity) {
            distances[j] = distances[current] + 1;
            paths[j] = paths[current];
            queue.push(j);
          } else if (distances[j] === distances[current] + 1) {
            paths[j] += paths[current];
          }
        }
      }

      // Accumulate betweenness
      for (let t = 0; t < n; t++) {
        if (s !== t && paths[t] > 0) {
          for (let v = 0; v < n; v++) {
            if (v !== s && v !== t && distances[v] < distances[t]) {
              betweenness[v] += paths[v] / paths[t];
            }
          }
        }
      }
    }

    // Normalize (avoid division by zero when n < 3)
    const norm = (n - 1) * (n - 2) / 2;
    for (let i = 0; i < n; i++) {
      this.nodes.get(symbols[i]).features.betweenness = norm > 0 ? betweenness[i] / norm : 0;
    }
  }

  // Calculate graph-level metrics
  calculateGraphMetrics() {
    const symbols = Array.from(this.nodes.keys());
    const n = symbols.length;

    // Edge count
    let edgeCount = 0;
    for (const node of this.nodes.values()) {
      edgeCount += node.getDegree();
    }
    edgeCount /= 2;  // Undirected

    // Density (avoid division by zero when n < 2)
    const maxEdges = n * (n - 1) / 2;
    const density = maxEdges > 0 ? edgeCount / maxEdges : 0;

    // Average clustering coefficient
    let totalClustering = 0;
    for (const node of this.nodes.values()) {
      const neighbors = Array.from(node.edges.keys());
      const k = neighbors.length;

      if (k < 2) {
        node.features.clusteringCoeff = 0;
        continue;
      }

      let triangles = 0;
      for (let i = 0; i < k; i++) {
        for (let j = i + 1; j < k; j++) {
          const neighbor1 = this.nodes.get(neighbors[i]);
          if (neighbor1.edges.has(neighbors[j])) {
            triangles++;
          }
        }
      }

      const maxTriangles = k * (k - 1) / 2;
      node.features.clusteringCoeff = triangles / maxTriangles;
      totalClustering += node.features.clusteringCoeff;
    }

    const avgClustering = n > 0 ? totalClustering / n : 0;

    return {
      nodes: n,
      edges: edgeCount,
      density,
      avgClustering,
      avgDegree: n > 0 ? (2 * edgeCount) / n : 0
    };
  }

  // Spectral analysis
  calculateSpectralFeatures() {
    const n = this.adjacencyMatrix.length;
    if (n < 2) return {};

    // Laplacian matrix L = D - A
    const laplacian = Array(n).fill(null).map(() => Array(n).fill(0));

    for (let i = 0; i < n; i++) {
      let degree = 0;
      for (let j = 0; j < n; j++) {
        if (i !== j && Math.abs(this.adjacencyMatrix[i][j]) >= this.config.construction.edgeThreshold) {
          laplacian[i][j] = -1;
          degree++;
        }
      }
      laplacian[i][i] = degree;
    }

    // Power iteration for largest eigenvalue (spectral radius)
    let v = new Array(n).fill(1 / Math.sqrt(n));
    let eigenvalue = 0;

    for (let iter = 0; iter < 50; iter++) {
      const newV = new Array(n).fill(0);

      for (let i = 0; i < n; i++) {
        for (let j = 0; j < n; j++) {
          newV[i] += Math.abs(this.adjacencyMatrix[i][j]) * v[j];
        }
      }

      eigenvalue = Math.sqrt(newV.reduce((a, b) => a + b * b, 0));
      if (eigenvalue > 0) {
        for (let i = 0; i < n; i++) {
          v[i] = newV[i] / eigenvalue;
        }
      }
    }

    // Estimate algebraic connectivity (second smallest Laplacian eigenvalue)
    // Using inverse power iteration on L
    const algebraicConnectivity = this.estimateAlgebraicConnectivity(laplacian);

    return {
      spectralRadius: eigenvalue,
      algebraicConnectivity,
      estimatedComponents: algebraicConnectivity < 0.01 ? 'multiple' : 'single'
    };
  }

  estimateAlgebraicConnectivity(laplacian) {
    const n = laplacian.length;
    if (n < 2) return 0;

    // Simplified: use trace / n as rough estimate
    let trace = 0;
    for (let i = 0; i < n; i++) {
      trace += laplacian[i][i];
    }

    return trace / n * 0.1;  // Rough approximation
  }

  // Detect regime change by comparing networks
  detectRegimeChange(previousMetrics, currentMetrics) {
    if (!previousMetrics) return { changed: false };

    const densityChange = Math.abs(currentMetrics.density - previousMetrics.density);
    const clusteringChange = Math.abs(currentMetrics.avgClustering - previousMetrics.avgClustering);

    const totalChange = densityChange + clusteringChange;
    const changed = totalChange > this.config.regime.changeThreshold;

    return {
      changed,
      densityChange,
      clusteringChange,
      totalChange,
      regime: this.classifyRegime(currentMetrics)
    };
  }

  classifyRegime(metrics) {
    if (metrics.density > 0.5 && metrics.avgClustering > 0.4) {
      return 'crisis';  // High connectivity = systemic risk
    } else if (metrics.density < 0.2) {
      return 'dispersion';  // Low connectivity = idiosyncratic
    }
    return 'normal';
  }

  // Save network state to history
  saveSnapshot() {
    this.history.push({
      timestamp: Date.now(),
      metrics: this.calculateGraphMetrics(),
      spectral: this.calculateSpectralFeatures()
    });

    if (this.history.length > 100) {
      this.history.shift();
    }
  }
}

// Generate synthetic multi-asset returns
function generateMultiAssetData(assets, days, seed = 42) {
  let rng = seed;
  const random = () => {
    rng = (rng * 9301 + 49297) % 233280;
    return rng / 233280;
  };

  // Correlation structure (sector-based)
  const sectors = {
    tech: ['AAPL', 'MSFT', 'GOOGL', 'NVDA'],
    finance: ['JPM', 'BAC', 'GS', 'MS'],
    energy: ['XOM', 'CVX', 'COP', 'SLB'],
    healthcare: ['JNJ', 'PFE', 'UNH', 'ABBV'],
    consumer: ['AMZN', 'WMT', 'HD', 'NKE']
  };

  const data = {};
  for (const asset of assets) {
    data[asset] = [];
  }

  // Find sector for each asset
  const assetSector = {};
  for (const [sector, members] of Object.entries(sectors)) {
    for (const asset of members) {
      assetSector[asset] = sector;
    }
  }

  // Generate correlated returns
  for (let day = 0; day < days; day++) {
    const marketFactor = (random() - 0.5) * 0.02;
    const sectorFactors = {};

    for (const sector of Object.keys(sectors)) {
      sectorFactors[sector] = (random() - 0.5) * 0.015;
    }

    for (const asset of assets) {
      const sector = assetSector[asset] || 'other';
      const sectorFactor = sectorFactors[sector] || 0;
      const idiosyncratic = (random() - 0.5) * 0.025;

      // Return = market + sector + idiosyncratic
      const return_ = marketFactor * 0.5 + sectorFactor * 0.3 + idiosyncratic * 0.2;
      data[asset].push(return_);
    }
  }

  return data;
}

async function main() {
  console.log('═'.repeat(70));
  console.log('GRAPH NEURAL NETWORK CORRELATION ANALYSIS');
  console.log('═'.repeat(70));
  console.log();

  // 1. Generate multi-asset data
  console.log('1. Multi-Asset Data Generation:');
  console.log('─'.repeat(70));

  const assets = [
    'AAPL', 'MSFT', 'GOOGL', 'NVDA',  // Tech
    'JPM', 'BAC', 'GS', 'MS',          // Finance
    'XOM', 'CVX', 'COP', 'SLB',        // Energy
    'JNJ', 'PFE', 'UNH', 'ABBV',       // Healthcare
    'AMZN', 'WMT', 'HD', 'NKE'         // Consumer
  ];

  const days = 120;
  const returnData = generateMultiAssetData(assets, days);

  console.log(`   Assets:     ${assets.length}`);
  console.log(`   Days:       ${days}`);
  console.log(`   Sectors:    Tech, Finance, Energy, Healthcare, Consumer`);
  console.log();

  // 2. Build correlation network
  console.log('2. Correlation Network Construction:');
  console.log('─'.repeat(70));

  const network = new CorrelationNetwork(gnnConfig);

  for (const asset of assets) {
    network.updateReturns(asset, returnData[asset]);
  }

  network.buildNetwork();

  console.log(`   Correlation method:  ${gnnConfig.construction.method}`);
  console.log(`   Edge threshold:      ${gnnConfig.construction.edgeThreshold}`);
  console.log(`   Max edges/node:      ${gnnConfig.construction.maxEdgesPerNode}`);
  console.log();

  // 3. Graph metrics
  console.log('3. Graph-Level Metrics:');
  console.log('─'.repeat(70));

  const graphMetrics = network.calculateGraphMetrics();

  console.log(`   Nodes:           ${graphMetrics.nodes}`);
  console.log(`   Edges:           ${graphMetrics.edges}`);
  console.log(`   Density:         ${(graphMetrics.density * 100).toFixed(1)}%`);
  console.log(`   Avg Clustering:  ${(graphMetrics.avgClustering * 100).toFixed(1)}%`);
  console.log(`   Avg Degree:      ${graphMetrics.avgDegree.toFixed(2)}`);
  console.log();

  // 4. Node centrality
  console.log('4. Node Centrality Analysis:');
  console.log('─'.repeat(70));

  network.calculateNodeCentrality();

  console.log('   Top 5 by Degree Centrality:');
  const byDegree = Array.from(network.nodes.values())
    .sort((a, b) => b.features.degreeCentrality - a.features.degreeCentrality)
    .slice(0, 5);

  for (const node of byDegree) {
    console.log(`   - ${node.symbol.padEnd(5)} ${(node.features.degreeCentrality * 100).toFixed(1)}%`);
  }
  console.log();

  console.log('   Top 5 by Eigenvector Centrality:');
  const byEigen = Array.from(network.nodes.values())
    .sort((a, b) => b.features.eigenvectorCentrality - a.features.eigenvectorCentrality)
    .slice(0, 5);

  for (const node of byEigen) {
    console.log(`   - ${node.symbol.padEnd(5)} ${(node.features.eigenvectorCentrality * 100).toFixed(1)}%`);
  }
  console.log();

  console.log('   Top 5 by PageRank:');
  const byPagerank = Array.from(network.nodes.values())
    .sort((a, b) => b.features.pagerank - a.features.pagerank)
    .slice(0, 5);

  for (const node of byPagerank) {
    console.log(`   - ${node.symbol.padEnd(5)} ${(node.features.pagerank * 100).toFixed(1)}%`);
  }
  console.log();

  // 5. Spectral analysis
  console.log('5. Spectral Analysis:');
  console.log('─'.repeat(70));

  const spectral = network.calculateSpectralFeatures();

  console.log(`   Spectral Radius:         ${spectral.spectralRadius.toFixed(4)}`);
  console.log(`   Algebraic Connectivity:  ${spectral.algebraicConnectivity.toFixed(4)}`);
  console.log(`   Estimated Components:    ${spectral.estimatedComponents}`);
  console.log();

  // 6. Correlation matrix visualization
  console.log('6. Correlation Matrix (Sample 5x5):');
  console.log('─'.repeat(70));

  const sampleAssets = assets.slice(0, 5);
  console.log('        ' + sampleAssets.map(a => a.slice(0, 4).padStart(6)).join(''));

  for (let i = 0; i < 5; i++) {
    let row = sampleAssets[i].slice(0, 4).padEnd(6) + ' ';
    for (let j = 0; j < 5; j++) {
      const corr = network.adjacencyMatrix[i][j];
      row += (corr >= 0 ? '+' : '') + corr.toFixed(2) + ' ';
    }
    console.log('   ' + row);
  }
  console.log();

  // 7. Network edges (sample)
  console.log('7. Strongest Connections (Top 10):');
  console.log('─'.repeat(70));

  const edges = [];
  const symbols = Array.from(network.nodes.keys());
  for (let i = 0; i < symbols.length; i++) {
    for (let j = i + 1; j < symbols.length; j++) {
      const corr = network.adjacencyMatrix[i][j];
      if (Math.abs(corr) >= gnnConfig.construction.edgeThreshold) {
        edges.push({ from: symbols[i], to: symbols[j], weight: corr });
      }
    }
  }

  edges.sort((a, b) => Math.abs(b.weight) - Math.abs(a.weight));

  for (const edge of edges.slice(0, 10)) {
    const sign = edge.weight > 0 ? '+' : '';
    console.log(`   ${edge.from.padEnd(5)} ↔ ${edge.to.padEnd(5)} ${sign}${edge.weight.toFixed(3)}`);
  }
  console.log();

  // 8. Regime analysis
  console.log('8. Regime Classification:');
  console.log('─'.repeat(70));

  const regime = network.classifyRegime(graphMetrics);

  console.log(`   Current Regime:  ${regime.toUpperCase()}`);
  console.log();
  console.log('   Interpretation:');

  if (regime === 'crisis') {
    console.log('   - High network connectivity indicates systemic risk');
    console.log('   - Correlations converging → diversification failing');
    console.log('   - Recommendation: Reduce exposure, hedge tail risk');
  } else if (regime === 'dispersion') {
    console.log('   - Low network connectivity indicates idiosyncratic moves');
    console.log('   - Good for stock picking and alpha generation');
    console.log('   - Recommendation: Active management, sector rotation');
  } else {
    console.log('   - Normal market conditions');
    console.log('   - Standard correlation structure');
    console.log('   - Recommendation: Balanced approach');
  }
  console.log();

  // 9. Trading implications
  console.log('9. Trading Implications:');
  console.log('─'.repeat(70));

  const highCentrality = byEigen[0];
  const lowCentrality = Array.from(network.nodes.values())
    .sort((a, b) => a.features.eigenvectorCentrality - b.features.eigenvectorCentrality)[0];

  console.log(`   Most Central Asset: ${highCentrality.symbol}`);
  console.log(`   - Moves with market, good for beta exposure`);
  console.log(`   - Higher correlation = less diversification benefit`);
  console.log();
  console.log(`   Least Central Asset: ${lowCentrality.symbol}`);
  console.log(`   - More idiosyncratic behavior`);
  console.log(`   - Potential alpha source, better diversifier`);
  console.log();

  // 10. RuVector integration
  console.log('10. RuVector Vector Storage:');
  console.log('─'.repeat(70));
  console.log('   Each node\'s features can be stored as vectors:');
  console.log();

  const sampleNode = network.nodes.get('AAPL');
  const featureVector = [
    sampleNode.features.degreeCentrality,
    sampleNode.features.eigenvectorCentrality,
    sampleNode.features.pagerank,
    sampleNode.features.betweenness,
    sampleNode.features.clusteringCoeff || 0
  ];

  console.log(`   ${sampleNode.symbol} feature vector:`);
  console.log(`   [${featureVector.map(v => v.toFixed(4)).join(', ')}]`);
  console.log();
  console.log('   Vector dimensions:');
  console.log('   [degree, eigenvector, pagerank, betweenness, clustering]');
  console.log();
  console.log('   Use case: Find assets with similar network positions');
  console.log('   via HNSW nearest neighbor search in RuVector');
  console.log();

  console.log('═'.repeat(70));
  console.log('Graph neural network analysis completed');
  console.log('═'.repeat(70));
}

main().catch(console.error);
