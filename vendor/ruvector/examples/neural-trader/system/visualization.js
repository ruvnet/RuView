/**
 * Visualization Module
 *
 * Terminal-based charts for equity curves, signals, and metrics
 * Uses ASCII art for compatibility across all terminals
 */

// Chart Configuration
const chartConfig = {
  width: 80,
  height: 20,
  padding: { left: 10, right: 2, top: 1, bottom: 3 },
  colors: {
    positive: '\x1b[32m',
    negative: '\x1b[31m',
    neutral: '\x1b[33m',
    reset: '\x1b[0m',
    dim: '\x1b[2m',
    bold: '\x1b[1m'
  }
};

/**
 * ASCII Line Chart
 */
class LineChart {
  constructor(config = {}) {
    this.width = config.width || chartConfig.width;
    this.height = config.height || chartConfig.height;
    this.padding = { ...chartConfig.padding, ...config.padding };
  }

  render(data, options = {}) {
    const { title = 'Chart', showGrid = true, colored = true } = options;

    if (!data || data.length === 0) return 'No data to display';

    const plotWidth = this.width - this.padding.left - this.padding.right;
    const plotHeight = this.height - this.padding.top - this.padding.bottom;

    // Calculate min/max
    const values = data.map(d => typeof d === 'number' ? d : d.value);
    const min = Math.min(...values);
    const max = Math.max(...values);
    const range = max - min || 1;

    // Create canvas
    const canvas = [];
    for (let y = 0; y < this.height; y++) {
      canvas.push(new Array(this.width).fill(' '));
    }

    // Draw title
    const titleStr = ` ${title} `;
    const titleStart = Math.floor((this.width - titleStr.length) / 2);
    for (let i = 0; i < titleStr.length; i++) {
      canvas[0][titleStart + i] = titleStr[i];
    }

    // Draw Y-axis labels
    for (let y = 0; y < plotHeight; y++) {
      const value = max - (y / (plotHeight - 1)) * range;
      const label = this.formatNumber(value).padStart(this.padding.left - 1);
      for (let i = 0; i < label.length; i++) {
        canvas[this.padding.top + y][i] = label[i];
      }
      canvas[this.padding.top + y][this.padding.left - 1] = '│';
    }

    // Draw X-axis
    for (let x = this.padding.left; x < this.width - this.padding.right; x++) {
      canvas[this.height - this.padding.bottom][x] = '─';
    }
    canvas[this.height - this.padding.bottom][this.padding.left - 1] = '└';

    // Plot data points
    const step = Math.max(1, Math.floor(data.length / plotWidth));
    let prevY = null;

    for (let i = 0; i < plotWidth && i * step < data.length; i++) {
      const idx = Math.min(i * step, data.length - 1);
      const value = values[idx];
      const normalizedY = (max - value) / range;
      const y = Math.floor(normalizedY * (plotHeight - 1));
      const x = this.padding.left + i;

      // Draw point
      const chartY = this.padding.top + y;
      if (chartY >= 0 && chartY < this.height) {
        if (prevY !== null && Math.abs(y - prevY) > 1) {
          // Draw connecting lines for gaps
          const startY = Math.min(y, prevY);
          const endY = Math.max(y, prevY);
          for (let cy = startY; cy <= endY; cy++) {
            const connectY = this.padding.top + cy;
            if (connectY >= 0 && connectY < this.height) {
              canvas[connectY][x - 1] = '│';
            }
          }
        }
        canvas[chartY][x] = '●';
      }
      prevY = y;
    }

    // Add grid if enabled
    if (showGrid) {
      for (let y = this.padding.top; y < this.height - this.padding.bottom; y += 4) {
        for (let x = this.padding.left; x < this.width - this.padding.right; x += 10) {
          if (canvas[y][x] === ' ') {
            canvas[y][x] = '·';
          }
        }
      }
    }

    // Convert to string with colors
    let result = '';
    const c = colored ? chartConfig.colors : { positive: '', negative: '', neutral: '', reset: '', dim: '', bold: '' };

    for (let y = 0; y < this.height; y++) {
      let line = '';
      for (let x = 0; x < this.width; x++) {
        const char = canvas[y][x];
        if (char === '●') {
          const dataIdx = Math.floor((x - this.padding.left) * step);
          const value = values[Math.min(dataIdx, values.length - 1)];
          const prevValue = dataIdx > 0 ? values[dataIdx - 1] : value;
          const color = value >= prevValue ? c.positive : c.negative;
          line += color + char + c.reset;
        } else if (char === '·') {
          line += c.dim + char + c.reset;
        } else {
          line += char;
        }
      }
      result += line + '\n';
    }

    return result;
  }

  formatNumber(n) {
    if (Math.abs(n) >= 1000000) return (n / 1000000).toFixed(1) + 'M';
    if (Math.abs(n) >= 1000) return (n / 1000).toFixed(1) + 'K';
    return n.toFixed(Math.abs(n) < 10 ? 2 : 0);
  }
}

/**
 * Bar Chart (for returns, volume, etc.)
 */
class BarChart {
  constructor(config = {}) {
    this.width = config.width || 60;
    this.height = config.height || 15;
    this.barWidth = config.barWidth || 1;
  }

  render(data, options = {}) {
    const { title = 'Bar Chart', labels = [], colored = true } = options;

    if (!data || data.length === 0) return 'No data to display';

    const values = data.map(d => typeof d === 'number' ? d : d.value);
    const maxVal = Math.max(...values.map(Math.abs));
    const hasNegative = values.some(v => v < 0);

    const c = colored ? chartConfig.colors : { positive: '', negative: '', neutral: '', reset: '' };
    let result = '';

    // Title
    result += `\n  ${title}\n`;
    result += '  ' + '─'.repeat(this.width) + '\n';

    if (hasNegative) {
      // Diverging bar chart
      const midLine = Math.floor(this.height / 2);

      for (let y = 0; y < this.height; y++) {
        let line = '  ';
        const threshold = maxVal * (1 - (y / this.height) * 2);

        for (let i = 0; i < Math.min(values.length, this.width); i++) {
          const v = values[i];
          const normalizedV = v / maxVal;

          if (y < midLine && v > 0 && normalizedV >= (midLine - y) / midLine) {
            line += c.positive + '█' + c.reset;
          } else if (y > midLine && v < 0 && Math.abs(normalizedV) >= (y - midLine) / midLine) {
            line += c.negative + '█' + c.reset;
          } else if (y === midLine) {
            line += '─';
          } else {
            line += ' ';
          }
        }
        result += line + '\n';
      }
    } else {
      // Standard bar chart
      for (let y = 0; y < this.height; y++) {
        let line = '  ';
        const threshold = maxVal * (1 - y / this.height);

        for (let i = 0; i < Math.min(values.length, this.width); i++) {
          const v = values[i];
          if (v >= threshold) {
            line += c.positive + '█' + c.reset;
          } else {
            line += ' ';
          }
        }
        result += line + '\n';
      }
    }

    // X-axis labels
    if (labels.length > 0) {
      result += '  ' + labels.slice(0, this.width).map(l => l[0] || ' ').join('') + '\n';
    }

    return result;
  }
}

/**
 * Sparkline (inline mini chart)
 */
class Sparkline {
  static render(data, options = {}) {
    const { width = 20, colored = true } = options;
    const chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

    if (!data || data.length === 0) return '';

    const values = data.slice(-width);
    const min = Math.min(...values);
    const max = Math.max(...values);
    const range = max - min || 1;

    const c = colored ? chartConfig.colors : { positive: '', negative: '', reset: '' };
    let result = '';
    let prev = values[0];

    for (const v of values) {
      const normalized = (v - min) / range;
      const idx = Math.min(Math.floor(normalized * chars.length), chars.length - 1);
      const color = v >= prev ? c.positive : c.negative;
      result += color + chars[idx] + c.reset;
      prev = v;
    }

    return result;
  }
}

/**
 * Table Renderer
 */
class Table {
  static render(data, options = {}) {
    const { headers = [], title = '' } = options;

    if (!data || data.length === 0) return 'No data';

    // Calculate column widths
    const allRows = headers.length > 0 ? [headers, ...data] : data;
    const numCols = Math.max(...allRows.map(r => r.length));
    const colWidths = new Array(numCols).fill(0);

    for (const row of allRows) {
      for (let i = 0; i < row.length; i++) {
        colWidths[i] = Math.max(colWidths[i], String(row[i]).length);
      }
    }

    const totalWidth = colWidths.reduce((a, b) => a + b, 0) + (numCols * 3) + 1;
    let result = '';

    // Title
    if (title) {
      result += '\n  ' + title + '\n';
    }

    // Top border
    result += '  ┌' + colWidths.map(w => '─'.repeat(w + 2)).join('┬') + '┐\n';

    // Headers
    if (headers.length > 0) {
      result += '  │';
      for (let i = 0; i < numCols; i++) {
        const cell = String(headers[i] || '').padEnd(colWidths[i]);
        result += ` ${cell} │`;
      }
      result += '\n';
      result += '  ├' + colWidths.map(w => '─'.repeat(w + 2)).join('┼') + '┤\n';
    }

    // Data rows
    for (const row of data) {
      result += '  │';
      for (let i = 0; i < numCols; i++) {
        const cell = String(row[i] || '').padEnd(colWidths[i]);
        result += ` ${cell} │`;
      }
      result += '\n';
    }

    // Bottom border
    result += '  └' + colWidths.map(w => '─'.repeat(w + 2)).join('┴') + '┘\n';

    return result;
  }
}

/**
 * Dashboard - Combines multiple visualizations
 */
class Dashboard {
  constructor(title = 'Trading Dashboard') {
    this.title = title;
    this.panels = [];
  }

  addPanel(content, options = {}) {
    this.panels.push({ content, ...options });
    return this;
  }

  addEquityCurve(data, title = 'Equity Curve') {
    const chart = new LineChart({ width: 70, height: 12 });
    return this.addPanel(chart.render(data, { title }));
  }

  addReturnsBar(returns, title = 'Daily Returns') {
    const chart = new BarChart({ width: 50, height: 8 });
    return this.addPanel(chart.render(returns.slice(-50), { title }));
  }

  addMetricsTable(metrics) {
    const data = [
      ['Total Return', `${(metrics.totalReturn * 100).toFixed(2)}%`],
      ['Sharpe Ratio', metrics.sharpeRatio.toFixed(2)],
      ['Max Drawdown', `${(metrics.maxDrawdown * 100).toFixed(2)}%`],
      ['Win Rate', `${(metrics.winRate * 100).toFixed(1)}%`],
      ['Profit Factor', metrics.profitFactor.toFixed(2)]
    ];
    return this.addPanel(Table.render(data, { headers: ['Metric', 'Value'], title: 'Performance' }));
  }

  addSignals(signals) {
    const c = chartConfig.colors;
    let content = '\n  SIGNALS\n  ' + '─'.repeat(40) + '\n';

    for (const [symbol, signal] of Object.entries(signals)) {
      const color = signal.direction === 'long' ? c.positive :
                    signal.direction === 'short' ? c.negative : c.neutral;
      const arrow = signal.direction === 'long' ? '▲' :
                    signal.direction === 'short' ? '▼' : '●';
      content += `  ${color}${arrow}${c.reset} ${symbol.padEnd(6)} ${signal.direction.toUpperCase().padEnd(6)} `;
      content += `${(signal.strength * 100).toFixed(0)}% confidence\n`;
    }

    return this.addPanel(content);
  }

  render() {
    const c = chartConfig.colors;
    let result = '\n';
    result += c.bold + '═'.repeat(80) + c.reset + '\n';
    result += c.bold + ' '.repeat((80 - this.title.length) / 2) + this.title + c.reset + '\n';
    result += c.bold + '═'.repeat(80) + c.reset + '\n';

    for (const panel of this.panels) {
      result += panel.content;
      result += '\n';
    }

    result += c.dim + '─'.repeat(80) + c.reset + '\n';
    result += c.dim + `Generated at ${new Date().toLocaleString()}` + c.reset + '\n';

    return result;
  }
}

/**
 * Quick visualization helpers
 */
const viz = {
  // Quick equity curve
  equity: (data, title = 'Equity Curve') => {
    const chart = new LineChart();
    return chart.render(data, { title });
  },

  // Quick returns bar
  returns: (data, title = 'Returns') => {
    const chart = new BarChart();
    return chart.render(data, { title });
  },

  // Inline sparkline
  spark: (data) => Sparkline.render(data),

  // Quick table
  table: (data, headers) => Table.render(data, { headers }),

  // Progress bar
  progress: (current, total, width = 30) => {
    const pct = current / total;
    const filled = Math.floor(pct * width);
    const empty = width - filled;
    return `[${'█'.repeat(filled)}${'░'.repeat(empty)}] ${(pct * 100).toFixed(1)}%`;
  },

  // Status indicator
  status: (value, thresholds = { good: 0, warn: -0.05, bad: -0.1 }) => {
    const c = chartConfig.colors;
    if (value >= thresholds.good) return c.positive + '●' + c.reset;
    if (value >= thresholds.warn) return c.neutral + '●' + c.reset;
    return c.negative + '●' + c.reset;
  }
};

export {
  LineChart,
  BarChart,
  Sparkline,
  Table,
  Dashboard,
  viz,
  chartConfig
};

// Demo if run directly
const isMainModule = import.meta.url === `file://${process.argv[1]}`;
if (isMainModule) {
  console.log('═'.repeat(70));
  console.log('VISUALIZATION MODULE DEMO');
  console.log('═'.repeat(70));

  // Generate sample data
  const equityData = [100000];
  for (let i = 0; i < 100; i++) {
    equityData.push(equityData[i] * (1 + (Math.random() - 0.48) * 0.02));
  }

  const returns = [];
  for (let i = 1; i < equityData.length; i++) {
    returns.push((equityData[i] - equityData[i-1]) / equityData[i-1]);
  }

  // Line chart
  const lineChart = new LineChart();
  console.log(lineChart.render(equityData, { title: 'Portfolio Equity' }));

  // Sparkline
  console.log('Sparkline: ' + Sparkline.render(equityData.slice(-30)));
  console.log();

  // Table
  console.log(Table.render([
    ['AAPL', '+2.5%', '150.25', 'BUY'],
    ['MSFT', '-1.2%', '378.50', 'HOLD'],
    ['GOOGL', '+0.8%', '141.75', 'BUY']
  ], { headers: ['Symbol', 'Change', 'Price', 'Signal'], title: 'Portfolio' }));

  // Dashboard
  const dashboard = new Dashboard('Trading Dashboard');
  dashboard.addEquityCurve(equityData.slice(-50));
  dashboard.addSignals({
    AAPL: { direction: 'long', strength: 0.75 },
    TSLA: { direction: 'short', strength: 0.60 },
    MSFT: { direction: 'neutral', strength: 0.40 }
  });
  console.log(dashboard.render());
}
