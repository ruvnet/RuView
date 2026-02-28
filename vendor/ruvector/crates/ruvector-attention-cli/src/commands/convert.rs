use clap::Args;
use crate::config::Config;
use serde::{Deserialize, Serialize};

#[derive(Args)]
pub struct ConvertArgs {
    /// Input file
    #[arg(short, long)]
    input: std::path::PathBuf,

    /// Output file
    #[arg(short, long)]
    output: std::path::PathBuf,

    /// Input format (auto-detect if not specified)
    #[arg(long)]
    from: Option<DataFormat>,

    /// Output format
    #[arg(long)]
    to: DataFormat,

    /// Pretty print output (for text formats)
    #[arg(short, long)]
    pretty: bool,
}

#[derive(Clone, clap::ValueEnum)]
pub enum DataFormat {
    Json,
    Binary,
    MsgPack,
    Csv,
    Npy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Data {
    values: Vec<Vec<f32>>,
}

pub fn run(args: ConvertArgs, _config: &Config) -> anyhow::Result<()> {
    tracing::info!("Converting from {:?} to {:?}", args.input, args.output);

    // Read input
    let content = std::fs::read(&args.input)?;
    let data = parse_data(&content, args.from.as_ref())?;

    // Write output
    write_data(&args.output, &data, &args.to, args.pretty)?;

    tracing::info!("Conversion complete");

    Ok(())
}

fn parse_data(content: &[u8], format: Option<&DataFormat>) -> anyhow::Result<Data> {
    if let Some(fmt) = format {
        match fmt {
            DataFormat::Json => Ok(serde_json::from_slice(content)?),
            DataFormat::Binary => Ok(bincode::deserialize(content)?),
            DataFormat::MsgPack => Ok(rmp_serde::from_slice(content)?),
            DataFormat::Csv => parse_csv(content),
            DataFormat::Npy => parse_npy(content),
        }
    } else {
        // Auto-detect
        if let Ok(data) = serde_json::from_slice::<Data>(content) {
            return Ok(data);
        }
        if let Ok(data) = rmp_serde::from_slice::<Data>(content) {
            return Ok(data);
        }
        if let Ok(data) = bincode::deserialize::<Data>(content) {
            return Ok(data);
        }

        Err(anyhow::anyhow!("Failed to auto-detect format"))
    }
}

fn write_data(
    path: &std::path::Path,
    data: &Data,
    format: &DataFormat,
    pretty: bool,
) -> anyhow::Result<()> {
    match format {
        DataFormat::Json => {
            let content = if pretty {
                serde_json::to_string_pretty(data)?
            } else {
                serde_json::to_string(data)?
            };
            std::fs::write(path, content)?;
        }
        DataFormat::Binary => {
            let bytes = bincode::serialize(data)?;
            std::fs::write(path, bytes)?;
        }
        DataFormat::MsgPack => {
            let bytes = rmp_serde::to_vec(data)?;
            std::fs::write(path, bytes)?;
        }
        DataFormat::Csv => {
            let csv = to_csv(data)?;
            std::fs::write(path, csv)?;
        }
        DataFormat::Npy => {
            let npy = to_npy(data)?;
            std::fs::write(path, npy)?;
        }
    }

    Ok(())
}

fn parse_csv(content: &[u8]) -> anyhow::Result<Data> {
    let text = std::str::from_utf8(content)?;
    let mut values = Vec::new();

    for line in text.lines().skip(1) { // Skip header
        let row: Vec<f32> = line.split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if !row.is_empty() {
            values.push(row);
        }
    }

    Ok(Data { values })
}

fn to_csv(data: &Data) -> anyhow::Result<String> {
    let mut csv = String::new();

    // Write header
    if let Some(first_row) = data.values.first() {
        csv.push_str("row");
        for i in 0..first_row.len() {
            csv.push_str(&format!(",col_{}", i));
        }
        csv.push('\n');
    }

    // Write data
    for (i, row) in data.values.iter().enumerate() {
        csv.push_str(&i.to_string());
        for val in row {
            csv.push_str(&format!(",{}", val));
        }
        csv.push('\n');
    }

    Ok(csv)
}

fn parse_npy(_content: &[u8]) -> anyhow::Result<Data> {
    // Simplified NPY parsing (real implementation would use a proper NPY library)
    Err(anyhow::anyhow!("NPY parsing not yet implemented"))
}

fn to_npy(_data: &Data) -> anyhow::Result<Vec<u8>> {
    // Simplified NPY writing (real implementation would use a proper NPY library)
    Err(anyhow::anyhow!("NPY writing not yet implemented"))
}
