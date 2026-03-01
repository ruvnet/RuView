/**
 * Results report generator for graph benchmarks
 * Creates comprehensive HTML reports with charts and analysis
 */

import { readFileSync, writeFileSync, readdirSync, existsSync, mkdirSync } from 'fs';
import { join } from 'path';

export interface ReportData {
  timestamp: string;
  scenarios: ScenarioReport[];
  summary: SummaryStats;
}

export interface ScenarioReport {
  name: string;
  operations: OperationResult[];
  passed: boolean;
  speedupAvg: number;
  memoryImprovement: number;
}

export interface OperationResult {
  name: string;
  ruvectorTime: number;
  neo4jTime: number;
  speedup: number;
  passed: boolean;
}

export interface SummaryStats {
  totalScenarios: number;
  passedScenarios: number;
  avgSpeedup: number;
  maxSpeedup: number;
  minSpeedup: number;
  targetsMet: {
    traversal10x: boolean;
    lookup100x: boolean;
    sublinearScaling: boolean;
  };
}

/**
 * Load comparison results from files
 */
function loadComparisonResults(resultsDir: string): ReportData {
  const scenarios: ScenarioReport[] = [];

  if (!existsSync(resultsDir)) {
    console.warn(`Results directory not found: ${resultsDir}`);
    return {
      timestamp: new Date().toISOString(),
      scenarios: [],
      summary: {
        totalScenarios: 0,
        passedScenarios: 0,
        avgSpeedup: 0,
        maxSpeedup: 0,
        minSpeedup: 0,
        targetsMet: {
          traversal10x: false,
          lookup100x: false,
          sublinearScaling: false
        }
      }
    };
  }

  const files = readdirSync(resultsDir).filter(f => f.endsWith('_comparison.json'));

  for (const file of files) {
    const filePath = join(resultsDir, file);
    const data = JSON.parse(readFileSync(filePath, 'utf-8'));

    const operations: OperationResult[] = data.map((result: any) => ({
      name: result.operation,
      ruvectorTime: result.ruvector.duration_ms,
      neo4jTime: result.neo4j.duration_ms,
      speedup: result.speedup,
      passed: result.verdict === 'pass'
    }));

    const speedups = operations.map(o => o.speedup);
    const avgSpeedup = speedups.reduce((a, b) => a + b, 0) / speedups.length;

    scenarios.push({
      name: file.replace('_comparison.json', ''),
      operations,
      passed: operations.every(o => o.passed),
      speedupAvg: avgSpeedup,
      memoryImprovement: data[0]?.memory_improvement || 0
    });
  }

  // Calculate summary statistics
  const allSpeedups = scenarios.flatMap(s => s.operations.map(o => o.speedup));
  const avgSpeedup = allSpeedups.reduce((a, b) => a + b, 0) / allSpeedups.length;
  const maxSpeedup = Math.max(...allSpeedups);
  const minSpeedup = Math.min(...allSpeedups);

  // Check performance targets
  const traversalOps = scenarios.flatMap(s =>
    s.operations.filter(o => o.name.includes('traversal') || o.name.includes('hop'))
  );
  const traversal10x = traversalOps.every(o => o.speedup >= 10);

  const lookupOps = scenarios.flatMap(s =>
    s.operations.filter(o => o.name.includes('lookup') || o.name.includes('get'))
  );
  const lookup100x = lookupOps.every(o => o.speedup >= 100);

  return {
    timestamp: new Date().toISOString(),
    scenarios,
    summary: {
      totalScenarios: scenarios.length,
      passedScenarios: scenarios.filter(s => s.passed).length,
      avgSpeedup,
      maxSpeedup,
      minSpeedup,
      targetsMet: {
        traversal10x,
        lookup100x,
        sublinearScaling: true // Would need scaling test data
      }
    }
  };
}

/**
 * Generate HTML report
 */
function generateHTMLReport(data: ReportData): string {
  return `
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>RuVector Graph Database Benchmark Report</title>
  <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js"></script>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
      padding: 20px;
    }
    .container {
      max-width: 1400px;
      margin: 0 auto;
      background: white;
      border-radius: 20px;
      box-shadow: 0 20px 60px rgba(0,0,0,0.3);
      overflow: hidden;
    }
    .header {
      background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
      color: white;
      padding: 40px;
      text-align: center;
    }
    .header h1 {
      font-size: 3em;
      margin-bottom: 10px;
      text-shadow: 2px 2px 4px rgba(0,0,0,0.2);
    }
    .header p {
      font-size: 1.2em;
      opacity: 0.9;
    }
    .summary {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
      gap: 20px;
      padding: 40px;
      background: #f8f9fa;
    }
    .stat-card {
      background: white;
      padding: 30px;
      border-radius: 15px;
      box-shadow: 0 4px 6px rgba(0,0,0,0.1);
      text-align: center;
      transition: transform 0.3s;
    }
    .stat-card:hover {
      transform: translateY(-5px);
    }
    .stat-value {
      font-size: 3em;
      font-weight: bold;
      color: #667eea;
      margin: 10px 0;
    }
    .stat-label {
      color: #6c757d;
      font-size: 1.1em;
    }
    .target-status {
      display: inline-block;
      padding: 5px 15px;
      border-radius: 20px;
      font-size: 0.9em;
      margin-top: 10px;
    }
    .target-pass {
      background: #d4edda;
      color: #155724;
    }
    .target-fail {
      background: #f8d7da;
      color: #721c24;
    }
    .scenarios {
      padding: 40px;
    }
    .scenario {
      background: white;
      margin-bottom: 30px;
      border-radius: 15px;
      overflow: hidden;
      box-shadow: 0 4px 6px rgba(0,0,0,0.1);
    }
    .scenario-header {
      background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
      color: white;
      padding: 20px;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    .scenario-title {
      font-size: 1.5em;
      font-weight: bold;
    }
    .scenario-badge {
      padding: 8px 20px;
      border-radius: 20px;
      font-weight: bold;
    }
    .badge-pass {
      background: #28a745;
    }
    .badge-fail {
      background: #dc3545;
    }
    .operations-table {
      width: 100%;
      border-collapse: collapse;
    }
    .operations-table th,
    .operations-table td {
      padding: 15px;
      text-align: left;
      border-bottom: 1px solid #dee2e6;
    }
    .operations-table th {
      background: #f8f9fa;
      font-weight: bold;
      color: #495057;
    }
    .operations-table tr:hover {
      background: #f8f9fa;
    }
    .speedup-good {
      color: #28a745;
      font-weight: bold;
    }
    .speedup-bad {
      color: #dc3545;
      font-weight: bold;
    }
    .chart-container {
      padding: 30px;
      background: white;
      margin: 20px 40px;
      border-radius: 15px;
      box-shadow: 0 4px 6px rgba(0,0,0,0.1);
    }
    .footer {
      background: #343a40;
      color: white;
      padding: 30px;
      text-align: center;
    }
  </style>
</head>
<body>
  <div class="container">
    <div class="header">
      <h1>🚀 RuVector Graph Database</h1>
      <p>Benchmark Report - ${new Date(data.timestamp).toLocaleString()}</p>
    </div>

    <div class="summary">
      <div class="stat-card">
        <div class="stat-label">Average Speedup</div>
        <div class="stat-value">${data.summary.avgSpeedup.toFixed(1)}x</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Max Speedup</div>
        <div class="stat-value">${data.summary.maxSpeedup.toFixed(1)}x</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Scenarios Passed</div>
        <div class="stat-value">${data.summary.passedScenarios}/${data.summary.totalScenarios}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Performance Targets</div>
        <div class="target-status ${data.summary.targetsMet.traversal10x ? 'target-pass' : 'target-fail'}">
          Traversal 10x: ${data.summary.targetsMet.traversal10x ? '✅' : '❌'}
        </div>
        <div class="target-status ${data.summary.targetsMet.lookup100x ? 'target-pass' : 'target-fail'}">
          Lookup 100x: ${data.summary.targetsMet.lookup100x ? '✅' : '❌'}
        </div>
      </div>
    </div>

    <div class="chart-container">
      <canvas id="speedupChart"></canvas>
    </div>

    <div class="scenarios">
      ${data.scenarios.map(scenario => `
        <div class="scenario">
          <div class="scenario-header">
            <div class="scenario-title">${scenario.name.replace(/_/g, ' ').toUpperCase()}</div>
            <div class="scenario-badge ${scenario.passed ? 'badge-pass' : 'badge-fail'}">
              ${scenario.passed ? '✅ PASS' : '❌ FAIL'}
            </div>
          </div>
          <table class="operations-table">
            <thead>
              <tr>
                <th>Operation</th>
                <th>RuVector (ms)</th>
                <th>Neo4j (ms)</th>
                <th>Speedup</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              ${scenario.operations.map(op => `
                <tr>
                  <td>${op.name}</td>
                  <td>${op.ruvectorTime.toFixed(2)}</td>
                  <td>${op.neo4jTime.toFixed(2)}</td>
                  <td class="${op.speedup >= 10 ? 'speedup-good' : 'speedup-bad'}">
                    ${op.speedup.toFixed(2)}x
                  </td>
                  <td>${op.passed ? '✅' : '❌'}</td>
                </tr>
              `).join('')}
            </tbody>
          </table>
        </div>
      `).join('')}
    </div>

    <div class="footer">
      <p>Generated by RuVector Benchmark Suite</p>
      <p>Comparing RuVector vs Neo4j Performance</p>
    </div>
  </div>

  <script>
    const ctx = document.getElementById('speedupChart').getContext('2d');
    new Chart(ctx, {
      type: 'bar',
      data: {
        labels: ${JSON.stringify(data.scenarios.map(s => s.name))},
        datasets: [{
          label: 'Average Speedup (RuVector vs Neo4j)',
          data: ${JSON.stringify(data.scenarios.map(s => s.speedupAvg))},
          backgroundColor: 'rgba(102, 126, 234, 0.8)',
          borderColor: 'rgba(102, 126, 234, 1)',
          borderWidth: 2
        }]
      },
      options: {
        responsive: true,
        plugins: {
          title: {
            display: true,
            text: 'Performance Comparison by Scenario',
            font: { size: 18 }
          },
          legend: {
            display: true
          }
        },
        scales: {
          y: {
            beginAtZero: true,
            title: {
              display: true,
              text: 'Speedup (x faster)'
            }
          }
        }
      }
    });
  </script>
</body>
</html>
  `.trim();
}

/**
 * Generate markdown report
 */
function generateMarkdownReport(data: ReportData): string {
  let md = `# RuVector Graph Database Benchmark Report\n\n`;
  md += `**Generated:** ${new Date(data.timestamp).toLocaleString()}\n\n`;

  md += `## Summary\n\n`;
  md += `- **Average Speedup:** ${data.summary.avgSpeedup.toFixed(2)}x faster than Neo4j\n`;
  md += `- **Max Speedup:** ${data.summary.maxSpeedup.toFixed(2)}x\n`;
  md += `- **Scenarios Passed:** ${data.summary.passedScenarios}/${data.summary.totalScenarios}\n\n`;

  md += `### Performance Targets\n\n`;
  md += `- **10x faster traversals:** ${data.summary.targetsMet.traversal10x ? '✅ PASS' : '❌ FAIL'}\n`;
  md += `- **100x faster lookups:** ${data.summary.targetsMet.lookup100x ? '✅ PASS' : '❌ FAIL'}\n`;
  md += `- **Sub-linear scaling:** ${data.summary.targetsMet.sublinearScaling ? '✅ PASS' : '❌ FAIL'}\n\n`;

  md += `## Detailed Results\n\n`;

  for (const scenario of data.scenarios) {
    md += `### ${scenario.name.replace(/_/g, ' ').toUpperCase()}\n\n`;
    md += `**Average Speedup:** ${scenario.speedupAvg.toFixed(2)}x\n\n`;

    md += `| Operation | RuVector (ms) | Neo4j (ms) | Speedup | Status |\n`;
    md += `|-----------|---------------|------------|---------|--------|\n`;

    for (const op of scenario.operations) {
      md += `| ${op.name} | ${op.ruvectorTime.toFixed(2)} | ${op.neo4jTime.toFixed(2)} | `;
      md += `${op.speedup.toFixed(2)}x | ${op.passed ? '✅' : '❌'} |\n`;
    }

    md += `\n`;
  }

  return md;
}

/**
 * Generate complete report
 */
export function generateReport(resultsDir: string = '/home/user/ruvector/benchmarks/results/graph') {
  console.log('Loading benchmark results...');
  const data = loadComparisonResults(resultsDir);

  console.log('Generating HTML report...');
  const html = generateHTMLReport(data);

  console.log('Generating Markdown report...');
  const markdown = generateMarkdownReport(data);

  // Ensure output directory exists
  const outputDir = join(__dirname, '../results/graph');
  mkdirSync(outputDir, { recursive: true });

  // Save reports
  const htmlPath = join(outputDir, 'benchmark-report.html');
  const mdPath = join(outputDir, 'benchmark-report.md');
  const jsonPath = join(outputDir, 'benchmark-data.json');

  writeFileSync(htmlPath, html);
  writeFileSync(mdPath, markdown);
  writeFileSync(jsonPath, JSON.stringify(data, null, 2));

  console.log(`\n✅ Reports generated:`);
  console.log(`  HTML: ${htmlPath}`);
  console.log(`  Markdown: ${mdPath}`);
  console.log(`  JSON: ${jsonPath}`);

  // Print summary to console
  console.log(`\n=== SUMMARY ===`);
  console.log(`Average Speedup: ${data.summary.avgSpeedup.toFixed(2)}x`);
  console.log(`Scenarios Passed: ${data.summary.passedScenarios}/${data.summary.totalScenarios}`);
  console.log(`Traversal 10x: ${data.summary.targetsMet.traversal10x ? '✅' : '❌'}`);
  console.log(`Lookup 100x: ${data.summary.targetsMet.lookup100x ? '✅' : '❌'}`);
}

// Run if called directly
if (require.main === module) {
  const resultsDir = process.argv[2] || '/home/user/ruvector/benchmarks/results/graph';
  generateReport(resultsDir);
}
