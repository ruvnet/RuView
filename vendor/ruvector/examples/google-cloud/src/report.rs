//! Benchmark report generation for RuVector Cloud Run GPU

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

use crate::benchmark::BenchmarkResult;

/// Generate report from benchmark results
pub fn generate_report(input_dir: &Path, output: &Path, format: &str) -> Result<()> {
    println!(
        "📊 Generating {} report from: {}",
        format,
        input_dir.display()
    );

    // Load all benchmark results
    let results = load_results(input_dir)?;

    if results.is_empty() {
        anyhow::bail!("No benchmark results found in {}", input_dir.display());
    }

    println!("   Found {} benchmark results", results.len());

    // Create output directory if needed
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    match format.to_lowercase().as_str() {
        "json" => generate_json_report(&results, output)?,
        "csv" => generate_csv_report(&results, output)?,
        "html" => generate_html_report(&results, output)?,
        "markdown" | "md" => generate_markdown_report(&results, output)?,
        _ => anyhow::bail!(
            "Unknown format: {}. Use json, csv, html, or markdown",
            format
        ),
    }

    println!("✓ Report saved to: {}", output.display());
    Ok(())
}

/// Load all benchmark results from a directory
fn load_results(dir: &Path) -> Result<Vec<BenchmarkResult>> {
    let mut all_results = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "json") {
            let file = File::open(&path)?;
            let reader = BufReader::new(file);

            // Try to parse as either a single result or wrapped results
            if let Ok(data) = serde_json::from_reader::<_, serde_json::Value>(reader) {
                if let Some(results) = data.get("results").and_then(|r| r.as_array()) {
                    for result in results {
                        if let Ok(r) = serde_json::from_value::<BenchmarkResult>(result.clone()) {
                            all_results.push(r);
                        }
                    }
                } else if let Ok(r) = serde_json::from_value::<BenchmarkResult>(data) {
                    all_results.push(r);
                }
            }
        }
    }

    Ok(all_results)
}

/// Generate JSON report
fn generate_json_report(results: &[BenchmarkResult], output: &Path) -> Result<()> {
    let report = generate_report_data(results);

    let file = File::create(output)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &report)?;

    Ok(())
}

/// Generate CSV report
fn generate_csv_report(results: &[BenchmarkResult], output: &Path) -> Result<()> {
    let mut file = File::create(output)?;

    // Write header
    writeln!(
        file,
        "name,operation,dimensions,num_vectors,batch_size,mean_ms,p50_ms,p95_ms,p99_ms,qps,memory_mb,gpu_enabled"
    )?;

    // Write data rows
    for r in results {
        writeln!(
            file,
            "{},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.1},{:.1},{}",
            r.name,
            r.operation,
            r.dimensions,
            r.num_vectors,
            r.batch_size,
            r.mean_time_ms,
            r.p50_ms,
            r.p95_ms,
            r.p99_ms,
            r.qps,
            r.memory_mb,
            r.gpu_enabled
        )?;
    }

    Ok(())
}

/// Generate HTML report
fn generate_html_report(results: &[BenchmarkResult], output: &Path) -> Result<()> {
    let report = generate_report_data(results);

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>RuVector Cloud Run GPU Benchmark Report</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        :root {{
            --primary: #2563eb;
            --success: #16a34a;
            --warning: #d97706;
            --danger: #dc2626;
            --bg: #f8fafc;
            --card-bg: #ffffff;
            --text: #1e293b;
            --text-muted: #64748b;
            --border: #e2e8f0;
        }}

        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            background: var(--bg);
            color: var(--text);
            line-height: 1.6;
        }}

        .container {{
            max-width: 1400px;
            margin: 0 auto;
            padding: 2rem;
        }}

        header {{
            background: linear-gradient(135deg, var(--primary) 0%, #1d4ed8 100%);
            color: white;
            padding: 3rem 2rem;
            margin-bottom: 2rem;
            border-radius: 1rem;
            box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
        }}

        header h1 {{
            font-size: 2.5rem;
            margin-bottom: 0.5rem;
        }}

        header p {{
            opacity: 0.9;
            font-size: 1.1rem;
        }}

        .stats-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 1.5rem;
            margin-bottom: 2rem;
        }}

        .stat-card {{
            background: var(--card-bg);
            border-radius: 0.75rem;
            padding: 1.5rem;
            box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
            border: 1px solid var(--border);
        }}

        .stat-card h3 {{
            font-size: 0.875rem;
            color: var(--text-muted);
            text-transform: uppercase;
            letter-spacing: 0.05em;
            margin-bottom: 0.5rem;
        }}

        .stat-card .value {{
            font-size: 2rem;
            font-weight: 700;
            color: var(--primary);
        }}

        .stat-card .unit {{
            font-size: 1rem;
            color: var(--text-muted);
            margin-left: 0.25rem;
        }}

        .card {{
            background: var(--card-bg);
            border-radius: 0.75rem;
            padding: 1.5rem;
            margin-bottom: 1.5rem;
            box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
            border: 1px solid var(--border);
        }}

        .card h2 {{
            font-size: 1.25rem;
            margin-bottom: 1rem;
            padding-bottom: 0.5rem;
            border-bottom: 2px solid var(--border);
        }}

        table {{
            width: 100%;
            border-collapse: collapse;
            font-size: 0.9rem;
        }}

        th, td {{
            padding: 0.75rem 1rem;
            text-align: left;
            border-bottom: 1px solid var(--border);
        }}

        th {{
            background: var(--bg);
            font-weight: 600;
            color: var(--text-muted);
            text-transform: uppercase;
            font-size: 0.75rem;
            letter-spacing: 0.05em;
        }}

        tr:hover {{
            background: var(--bg);
        }}

        .chart-container {{
            position: relative;
            height: 400px;
            margin-bottom: 1rem;
        }}

        .badge {{
            display: inline-block;
            padding: 0.25rem 0.75rem;
            border-radius: 9999px;
            font-size: 0.75rem;
            font-weight: 600;
        }}

        .badge-success {{
            background: #dcfce7;
            color: var(--success);
        }}

        .badge-warning {{
            background: #fef3c7;
            color: var(--warning);
        }}

        .two-col {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(500px, 1fr));
            gap: 1.5rem;
        }}

        footer {{
            text-align: center;
            padding: 2rem;
            color: var(--text-muted);
            font-size: 0.875rem;
        }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🚀 RuVector GPU Benchmark Report</h1>
            <p>Cloud Run GPU Performance Analysis | Generated: {timestamp}</p>
        </header>

        <div class="stats-grid">
            <div class="stat-card">
                <h3>Total Benchmarks</h3>
                <div class="value">{total_benchmarks}</div>
            </div>
            <div class="stat-card">
                <h3>Peak QPS</h3>
                <div class="value">{peak_qps:.0}<span class="unit">q/s</span></div>
            </div>
            <div class="stat-card">
                <h3>Best P99 Latency</h3>
                <div class="value">{best_p99:.2}<span class="unit">ms</span></div>
            </div>
            <div class="stat-card">
                <h3>GPU Enabled</h3>
                <div class="value">{gpu_status}</div>
            </div>
        </div>

        <div class="two-col">
            <div class="card">
                <h2>📈 Latency Distribution</h2>
                <div class="chart-container">
                    <canvas id="latencyChart"></canvas>
                </div>
            </div>

            <div class="card">
                <h2>⚡ Throughput Comparison</h2>
                <div class="chart-container">
                    <canvas id="throughputChart"></canvas>
                </div>
            </div>
        </div>

        <div class="card">
            <h2>📊 Detailed Results</h2>
            <table>
                <thead>
                    <tr>
                        <th>Operation</th>
                        <th>Dimensions</th>
                        <th>Vectors</th>
                        <th>Mean (ms)</th>
                        <th>P50 (ms)</th>
                        <th>P95 (ms)</th>
                        <th>P99 (ms)</th>
                        <th>QPS</th>
                        <th>Memory</th>
                    </tr>
                </thead>
                <tbody>
                    {table_rows}
                </tbody>
            </table>
        </div>

        <footer>
            <p>Generated by RuVector Cloud Run GPU Benchmark Suite</p>
            <p>© 2024 RuVector Team | MIT License</p>
        </footer>
    </div>

    <script>
        // Latency Chart
        const latencyCtx = document.getElementById('latencyChart').getContext('2d');
        new Chart(latencyCtx, {{
            type: 'bar',
            data: {{
                labels: {latency_labels},
                datasets: [
                    {{
                        label: 'P50',
                        data: {latency_p50},
                        backgroundColor: 'rgba(37, 99, 235, 0.8)',
                    }},
                    {{
                        label: 'P95',
                        data: {latency_p95},
                        backgroundColor: 'rgba(217, 119, 6, 0.8)',
                    }},
                    {{
                        label: 'P99',
                        data: {latency_p99},
                        backgroundColor: 'rgba(220, 38, 38, 0.8)',
                    }}
                ]
            }},
            options: {{
                responsive: true,
                maintainAspectRatio: false,
                plugins: {{
                    legend: {{
                        position: 'top',
                    }},
                    title: {{
                        display: false,
                    }}
                }},
                scales: {{
                    y: {{
                        beginAtZero: true,
                        title: {{
                            display: true,
                            text: 'Latency (ms)'
                        }}
                    }}
                }}
            }}
        }});

        // Throughput Chart
        const throughputCtx = document.getElementById('throughputChart').getContext('2d');
        new Chart(throughputCtx, {{
            type: 'bar',
            data: {{
                labels: {throughput_labels},
                datasets: [{{
                    label: 'QPS',
                    data: {throughput_values},
                    backgroundColor: 'rgba(22, 163, 74, 0.8)',
                }}]
            }},
            options: {{
                responsive: true,
                maintainAspectRatio: false,
                plugins: {{
                    legend: {{
                        display: false,
                    }}
                }},
                scales: {{
                    y: {{
                        beginAtZero: true,
                        title: {{
                            display: true,
                            text: 'Queries per Second'
                        }}
                    }}
                }}
            }}
        }});
    </script>
</body>
</html>
"#,
        timestamp = report.timestamp,
        total_benchmarks = report.total_benchmarks,
        peak_qps = report.peak_qps,
        best_p99 = report.best_p99_ms,
        gpu_status = if report.gpu_enabled { "Yes ✓" } else { "No" },
        table_rows = generate_table_rows(results),
        latency_labels = serde_json::to_string(&report.chart_labels).unwrap(),
        latency_p50 = serde_json::to_string(&report.latency_p50).unwrap(),
        latency_p95 = serde_json::to_string(&report.latency_p95).unwrap(),
        latency_p99 = serde_json::to_string(&report.latency_p99).unwrap(),
        throughput_labels = serde_json::to_string(&report.chart_labels).unwrap(),
        throughput_values = serde_json::to_string(&report.throughput_qps).unwrap(),
    );

    let mut file = File::create(output)?;
    file.write_all(html.as_bytes())?;

    Ok(())
}

/// Generate Markdown report
fn generate_markdown_report(results: &[BenchmarkResult], output: &Path) -> Result<()> {
    let report = generate_report_data(results);

    let mut md = String::new();

    md.push_str("# RuVector Cloud Run GPU Benchmark Report\n\n");
    md.push_str(&format!("**Generated:** {}\n\n", report.timestamp));

    md.push_str("## Summary\n\n");
    md.push_str(&format!(
        "- **Total Benchmarks:** {}\n",
        report.total_benchmarks
    ));
    md.push_str(&format!("- **Peak QPS:** {:.0}\n", report.peak_qps));
    md.push_str(&format!(
        "- **Best P99 Latency:** {:.2} ms\n",
        report.best_p99_ms
    ));
    md.push_str(&format!(
        "- **GPU Enabled:** {}\n\n",
        if report.gpu_enabled { "Yes" } else { "No" }
    ));

    md.push_str("## Detailed Results\n\n");
    md.push_str("| Operation | Dims | Vectors | Mean (ms) | P50 (ms) | P95 (ms) | P99 (ms) | QPS | Memory (MB) |\n");
    md.push_str("|-----------|------|---------|-----------|----------|----------|----------|-----|-------------|\n");

    for r in results {
        md.push_str(&format!(
            "| {} | {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.0} | {:.1} |\n",
            r.operation,
            r.dimensions,
            r.num_vectors,
            r.mean_time_ms,
            r.p50_ms,
            r.p95_ms,
            r.p99_ms,
            r.qps,
            r.memory_mb
        ));
    }

    md.push_str("\n---\n");
    md.push_str("*Generated by RuVector Cloud Run GPU Benchmark Suite*\n");

    let mut file = File::create(output)?;
    file.write_all(md.as_bytes())?;

    Ok(())
}

/// Report data structure
#[derive(Debug, Serialize)]
struct ReportData {
    timestamp: String,
    total_benchmarks: usize,
    peak_qps: f64,
    best_p99_ms: f64,
    gpu_enabled: bool,
    chart_labels: Vec<String>,
    latency_p50: Vec<f64>,
    latency_p95: Vec<f64>,
    latency_p99: Vec<f64>,
    throughput_qps: Vec<f64>,
    results: Vec<BenchmarkResult>,
}

fn generate_report_data(results: &[BenchmarkResult]) -> ReportData {
    let peak_qps = results.iter().map(|r| r.qps).fold(0.0f64, f64::max);
    let best_p99 = results
        .iter()
        .map(|r| r.p99_ms)
        .filter(|&p| p > 0.0)
        .fold(f64::INFINITY, f64::min);
    let gpu_enabled = results.iter().any(|r| r.gpu_enabled);

    let chart_labels: Vec<String> = results
        .iter()
        .take(10)
        .map(|r| format!("{}d", r.dimensions))
        .collect();

    let latency_p50: Vec<f64> = results.iter().take(10).map(|r| r.p50_ms).collect();
    let latency_p95: Vec<f64> = results.iter().take(10).map(|r| r.p95_ms).collect();
    let latency_p99: Vec<f64> = results.iter().take(10).map(|r| r.p99_ms).collect();
    let throughput_qps: Vec<f64> = results.iter().take(10).map(|r| r.qps).collect();

    ReportData {
        timestamp: chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string(),
        total_benchmarks: results.len(),
        peak_qps,
        best_p99_ms: if best_p99.is_infinite() {
            0.0
        } else {
            best_p99
        },
        gpu_enabled,
        chart_labels,
        latency_p50,
        latency_p95,
        latency_p99,
        throughput_qps,
        results: results.to_vec(),
    }
}

fn generate_table_rows(results: &[BenchmarkResult]) -> String {
    results
        .iter()
        .map(|r| {
            format!(
                r#"<tr>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{:.3}</td>
                    <td>{:.3}</td>
                    <td>{:.3}</td>
                    <td>{:.3}</td>
                    <td>{:.0}</td>
                    <td>{:.1} MB</td>
                </tr>"#,
                r.operation,
                r.dimensions,
                r.num_vectors,
                r.mean_time_ms,
                r.p50_ms,
                r.p95_ms,
                r.p99_ms,
                r.qps,
                r.memory_mb
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
