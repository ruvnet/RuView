use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tabled::{settings::Style, Table, Tabled};

#[derive(Clone, clap::ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Json,
    Binary,
    Csv,
    MsgPack,
    Table,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionOutput {
    pub attention_type: String,
    pub dimensions: OutputDimensions,
    pub scores: Vec<Vec<f32>>,
    pub metadata: OutputMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDimensions {
    pub batch_size: usize,
    pub num_heads: usize,
    pub seq_length: usize,
    pub embedding_dim: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputMetadata {
    pub compute_time_ms: f64,
    pub memory_bytes: usize,
    pub num_parameters: usize,
}

#[derive(Debug, Clone, Tabled)]
pub struct BenchmarkRow {
    pub attention_type: String,
    pub dimension: usize,
    pub mean_time_ms: f64,
    pub std_dev_ms: f64,
    pub throughput: f64,
}

pub struct Output {
    data: AttentionOutput,
}

impl Output {
    pub fn new(
        attention_type: impl Into<String>,
        dimensions: OutputDimensions,
        scores: Vec<Vec<f32>>,
        metadata: OutputMetadata,
    ) -> Self {
        Self {
            data: AttentionOutput {
                attention_type: attention_type.into(),
                dimensions,
                scores,
                metadata,
            },
        }
    }

    pub fn write(&self, path: Option<&Path>, format: OutputFormat) -> Result<()> {
        let content = match format {
            OutputFormat::Pretty => self.to_pretty()?,
            OutputFormat::Json => serde_json::to_string_pretty(&self.data)?,
            OutputFormat::Binary => {
                if let Some(p) = path {
                    std::fs::write(p, bincode::serialize(&self.data)?)?;
                    return Ok(());
                } else {
                    return Err(anyhow::anyhow!("Binary format requires output path"));
                }
            }
            OutputFormat::Csv => self.to_csv()?,
            OutputFormat::MsgPack => {
                if let Some(p) = path {
                    let data = rmp_serde::to_vec(&self.data)?;
                    std::fs::write(p, data)?;
                    return Ok(());
                } else {
                    return Err(anyhow::anyhow!("MessagePack format requires output path"));
                }
            }
            OutputFormat::Table => self.to_table()?,
        };

        if let Some(p) = path {
            std::fs::write(p, content)?;
        } else {
            println!("{}", content);
        }

        Ok(())
    }

    fn to_pretty(&self) -> Result<String> {
        let mut output = String::new();
        output.push_str(&format!("Attention Type: {}\n", self.data.attention_type));
        output.push_str(&format!("Dimensions:\n"));
        output.push_str(&format!("  Batch Size: {}\n", self.data.dimensions.batch_size));
        output.push_str(&format!("  Num Heads: {}\n", self.data.dimensions.num_heads));
        output.push_str(&format!("  Sequence Length: {}\n", self.data.dimensions.seq_length));
        output.push_str(&format!("  Embedding Dim: {}\n", self.data.dimensions.embedding_dim));
        output.push_str(&format!("\nMetadata:\n"));
        output.push_str(&format!("  Compute Time: {:.2}ms\n", self.data.metadata.compute_time_ms));
        output.push_str(&format!("  Memory Usage: {} bytes\n", self.data.metadata.memory_bytes));
        output.push_str(&format!("  Parameters: {}\n", self.data.metadata.num_parameters));

        if !self.data.scores.is_empty() {
            output.push_str(&format!("\nAttention Scores (first 5x5):\n"));
            for (i, row) in self.data.scores.iter().take(5).enumerate() {
                output.push_str(&format!("  Row {}: ", i));
                for val in row.iter().take(5) {
                    output.push_str(&format!("{:.4} ", val));
                }
                output.push_str("\n");
            }
        }

        Ok(output)
    }

    fn to_csv(&self) -> Result<String> {
        let mut csv = String::new();
        csv.push_str("row,col,value\n");

        for (i, row) in self.data.scores.iter().enumerate() {
            for (j, val) in row.iter().enumerate() {
                csv.push_str(&format!("{},{},{}\n", i, j, val));
            }
        }

        Ok(csv)
    }

    fn to_table(&self) -> Result<String> {
        let rows: Vec<Vec<String>> = self.data.scores.iter()
            .take(10)
            .map(|row| {
                row.iter()
                    .take(10)
                    .map(|v| format!("{:.4}", v))
                    .collect()
            })
            .collect();

        let mut table_str = String::from("Attention Scores:\n");
        for row in rows {
            table_str.push_str(&row.join(" | "));
            table_str.push('\n');
        }

        Ok(table_str)
    }
}

pub fn print_benchmark_results(results: Vec<BenchmarkRow>) {
    let table = Table::new(results)
        .with(Style::modern())
        .to_string();

    println!("{}", table);
}
