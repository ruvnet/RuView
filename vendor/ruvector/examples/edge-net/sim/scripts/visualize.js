#!/usr/bin/env node

/**
 * Visualization Script for Simulation Results
 * Generates charts and graphs from simulation data
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const args = process.argv.slice(2);
const reportFile = args[0] || findLatestReport();

if (!reportFile) {
  console.error('❌ No report file found. Run a simulation first.');
  process.exit(1);
}

console.log(`📊 Visualizing report: ${reportFile}\n`);

const report = JSON.parse(fs.readFileSync(reportFile, 'utf-8'));

// Generate ASCII charts
generateNodeGrowthChart(report);
generateEconomicChart(report);
generatePhaseTimeline(report);
generateHealthDashboard(report);

function findLatestReport() {
  const reportsDir = path.join(__dirname, '../reports');
  if (!fs.existsSync(reportsDir)) return null;

  const files = fs.readdirSync(reportsDir)
    .filter(f => f.endsWith('.json'))
    .map(f => ({
      name: f,
      path: path.join(reportsDir, f),
      time: fs.statSync(path.join(reportsDir, f)).mtime.getTime()
    }))
    .sort((a, b) => b.time - a.time);

  return files.length > 0 ? files[0].path : null;
}

function generateNodeGrowthChart(report) {
  console.log('📈 NODE GROWTH OVER TIME');
  console.log('─'.repeat(70));

  const transitions = report.phases.transitions;
  const maxNodes = report.summary.totalNodes;

  transitions.forEach((t, i) => {
    const barLength = Math.floor((t.nodeCount / maxNodes) * 50);
    const bar = '█'.repeat(barLength) + '░'.repeat(50 - barLength);

    console.log(`${t.to.padEnd(15)} │${bar}│ ${t.nodeCount.toLocaleString()} nodes`);
  });

  console.log('\n');
}

function generateEconomicChart(report) {
  console.log('💰 ECONOMIC DISTRIBUTION');
  console.log('─'.repeat(70));

  const { supply } = report.economics;
  const total = supply.total || 1;

  const pools = [
    { name: 'Contributors', value: supply.contributors, symbol: '█' },
    { name: 'Treasury', value: supply.treasury, symbol: '▓' },
    { name: 'Protocol', value: supply.protocol, symbol: '▒' },
    { name: 'Founders', value: supply.founders, symbol: '░' },
  ];

  pools.forEach(pool => {
    const percentage = (pool.value / total) * 100;
    const barLength = Math.floor(percentage / 2);
    const bar = pool.symbol.repeat(barLength);

    console.log(
      `${pool.name.padEnd(14)} │${bar.padEnd(50)}│ ` +
      `${pool.value.toLocaleString().padStart(10)} rUv (${percentage.toFixed(1)}%)`
    );
  });

  console.log('\n');
}

function generatePhaseTimeline(report) {
  console.log('🔄 PHASE TRANSITION TIMELINE');
  console.log('─'.repeat(70));

  const transitions = report.phases.transitions;

  transitions.forEach((t, i) => {
    const arrow = i === 0 ? '├─' : '├─';
    console.log(`${arrow}> ${t.from.toUpperCase()} → ${t.to.toUpperCase()}`);
    console.log(`│   Tick: ${t.tick.toLocaleString()}`);
    console.log(`│   Nodes: ${t.nodeCount.toLocaleString()}`);
    console.log(`│   Compute: ${Math.floor(t.totalCompute).toLocaleString()} hours`);
    if (i < transitions.length - 1) {
      console.log('│');
    }
  });

  console.log('└─> CURRENT: ' + report.summary.finalPhase.toUpperCase());
  console.log('\n');
}

function generateHealthDashboard(report) {
  console.log('🏥 NETWORK HEALTH DASHBOARD');
  console.log('─'.repeat(70));

  const metrics = [
    {
      name: 'Network Health',
      value: report.metrics.networkHealth,
      threshold: 0.7,
      unit: '%'
    },
    {
      name: 'Success Rate',
      value: report.metrics.averageSuccessRate,
      threshold: 0.85,
      unit: '%'
    },
    {
      name: 'Economic Stability',
      value: report.economics.health.stability,
      threshold: 0.6,
      unit: '%'
    },
    {
      name: 'Economic Velocity',
      value: report.economics.health.velocity,
      threshold: 0.3,
      unit: ''
    },
  ];

  metrics.forEach(metric => {
    const percentage = metric.unit === '%' ? metric.value * 100 : metric.value * 100;
    const barLength = Math.floor(percentage / 2);
    const status = metric.value >= metric.threshold ? '✓' : '✗';
    const color = metric.value >= metric.threshold ? '🟢' : '🔴';

    console.log(
      `${status} ${metric.name.padEnd(20)} ${color} ` +
      `${'█'.repeat(Math.floor(barLength))}${'░'.repeat(50 - Math.floor(barLength))} ` +
      `${(metric.value * 100).toFixed(1)}${metric.unit}`
    );
  });

  console.log('\n');
}

function generateGenesisAnalysis(report) {
  console.log('👑 GENESIS NODE ANALYSIS');
  console.log('─'.repeat(70));

  const genesisNodes = report.nodes.genesis;
  const totalGenesisRuv = genesisNodes.reduce((sum, n) => sum + n.ruvEarned, 0);
  const totalGenesisTasks = genesisNodes.reduce((sum, n) => sum + n.tasksCompleted, 0);
  const avgGenesisCompute = genesisNodes.reduce((sum, n) => sum + n.totalComputeHours, 0) / genesisNodes.length;

  console.log(`Total Genesis Nodes:     ${genesisNodes.length}`);
  console.log(`Active Genesis Nodes:    ${genesisNodes.filter(n => n.active).length}`);
  console.log(`Total rUv Earned:        ${totalGenesisRuv.toLocaleString()}`);
  console.log(`Total Tasks Completed:   ${totalGenesisTasks.toLocaleString()}`);
  console.log(`Avg Compute per Node:    ${Math.floor(avgGenesisCompute).toLocaleString()} hours`);

  console.log('\nTop Genesis Contributors:');
  const topGenesis = [...genesisNodes]
    .sort((a, b) => b.ruvEarned - a.ruvEarned)
    .slice(0, 5);

  topGenesis.forEach((node, i) => {
    console.log(
      `  ${(i + 1)}. ${node.id.padEnd(12)} - ` +
      `${node.ruvEarned.toLocaleString().padStart(8)} rUv, ` +
      `${node.tasksCompleted.toLocaleString().padStart(6)} tasks`
    );
  });

  console.log('\n');
}

generateGenesisAnalysis(report);

console.log('✅ Visualization complete!\n');
