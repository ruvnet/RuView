use clap::Args;
use crate::config::Config;
use rustyline::{Editor, error::ReadlineError, history::DefaultHistory};
use ruvector_attention::{
    attention::{ScaledDotProductAttention, MultiHeadAttention},
    hyperbolic::HyperbolicAttention,
    sparse::{FlashAttention, LinearAttention},
    moe::MoEAttention,
};

#[derive(Args)]
pub struct ReplArgs {
    /// Initial dimension
    #[arg(short, long, default_value = "512")]
    dim: usize,
}

enum Command {
    Help,
    Load(String),
    Compute(ComputeArgs),
    SetType(String),
    SetDim(usize),
    Config,
    Quit,
    Unknown(String),
}

struct ComputeArgs {
    query: Vec<Vec<f32>>,
    keys: Vec<Vec<f32>>,
    values: Vec<Vec<f32>>,
}

struct ReplState {
    config: Config,
    dim: usize,
    attention_type: String,
    last_query: Option<Vec<Vec<f32>>>,
    last_keys: Option<Vec<Vec<f32>>>,
    last_values: Option<Vec<Vec<f32>>>,
}

impl ReplState {
    fn new(config: &Config, dim: usize) -> anyhow::Result<Self> {
        Ok(Self {
            config: config.clone(),
            dim,
            attention_type: config.attention.default_type.clone(),
            last_query: None,
            last_keys: None,
            last_values: None,
        })
    }

    fn load(&mut self, path: &str) -> anyhow::Result<()> {
        let data = super::load_input(&std::path::Path::new(path))?;
        self.last_query = Some(data.query);
        self.last_keys = Some(data.keys);
        self.last_values = Some(data.values);
        self.dim = data.dim;
        println!("Loaded data from {}", path);
        Ok(())
    }

    fn compute(&self, args: &ComputeArgs) -> anyhow::Result<Vec<Vec<f32>>> {
        let keys_refs: Vec<&[f32]> = args.keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = args.values.iter().map(|v| v.as_slice()).collect();

        match self.attention_type.as_str() {
            "scaled_dot" => {
                let attention = ScaledDotProductAttention::new(self.dim, None);
                attention.compute(&args.query, &keys_refs, &values_refs)
            }
            "multi_head" => {
                let attention = MultiHeadAttention::new(self.dim, self.config.attention.default_heads)?;
                attention.compute(&args.query, &keys_refs, &values_refs)
            }
            "hyperbolic" => {
                let attention = HyperbolicAttention::new(self.dim, 1.0)?;
                attention.compute(&args.query, &keys_refs, &values_refs)
            }
            "flash" => {
                let attention = FlashAttention::new(self.dim, 64)?;
                attention.compute(&args.query, &keys_refs, &values_refs)
            }
            "linear" => {
                let attention = LinearAttention::new(self.dim)?;
                attention.compute(&args.query, &keys_refs, &values_refs)
            }
            "moe" => {
                let attention = MoEAttention::new(self.dim, 4, 2)?;
                attention.compute(&args.query, &keys_refs, &values_refs)
            }
            _ => Err(anyhow::anyhow!("Unknown attention type: {}", self.attention_type)),
        }
    }

    fn set_attention_type(&mut self, attention_type: String) {
        self.attention_type = attention_type;
        println!("Attention type set to: {}", self.attention_type);
    }

    fn set_dim(&mut self, dim: usize) {
        self.dim = dim;
        println!("Dimension set to: {}", self.dim);
    }

    fn config(&self) -> &Config {
        &self.config
    }
}

pub async fn run(args: ReplArgs, config: &Config) -> anyhow::Result<()> {
    let mut rl = Editor::<(), DefaultHistory>::new()?;

    println!("RuVector Attention REPL v{}", env!("CARGO_PKG_VERSION"));
    println!("Type 'help' for commands, 'quit' to exit\n");

    let mut state = ReplState::new(config, args.dim)?;

    loop {
        match rl.readline("attention> ") {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }

                rl.add_history_entry(&line)?;

                match parse_command(&line) {
                    Command::Help => print_help(),
                    Command::Load(path) => {
                        if let Err(e) = state.load(&path) {
                            eprintln!("Error loading file: {}", e);
                        }
                    }
                    Command::Compute(args) => {
                        match state.compute(&args) {
                            Ok(result) => {
                                println!("Result shape: {}x{}", result.len(), result.first().map(|r| r.len()).unwrap_or(0));
                                println!("First row (first 5 values): {:?}",
                                    result.first().map(|r| &r[..5.min(r.len())]));
                            }
                            Err(e) => eprintln!("Error computing attention: {}", e),
                        }
                    }
                    Command::SetType(t) => state.set_attention_type(t),
                    Command::SetDim(d) => state.set_dim(d),
                    Command::Config => println!("{:#?}", state.config()),
                    Command::Quit => break,
                    Command::Unknown(cmd) => println!("Unknown command: '{}'. Type 'help' for available commands.", cmd),
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(err) => return Err(err.into()),
        }
    }

    println!("Goodbye!");
    Ok(())
}

fn parse_command(line: &str) -> Command {
    let parts: Vec<&str> = line.trim().split_whitespace().collect();

    if parts.is_empty() {
        return Command::Unknown(String::new());
    }

    match parts[0] {
        "help" | "h" | "?" => Command::Help,
        "load" => {
            if parts.len() > 1 {
                Command::Load(parts[1].to_string())
            } else {
                Command::Unknown("load requires a file path".to_string())
            }
        }
        "compute" => {
            // For simplicity, use random data
            let query = vec![vec![0.1, 0.2, 0.3]];
            let keys = vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]];
            let values = vec![vec![0.7, 0.8, 0.9], vec![1.0, 1.1, 1.2]];
            Command::Compute(ComputeArgs { query, keys, values })
        }
        "type" => {
            if parts.len() > 1 {
                Command::SetType(parts[1].to_string())
            } else {
                Command::Unknown("type requires an attention type".to_string())
            }
        }
        "dim" => {
            if parts.len() > 1 {
                if let Ok(d) = parts[1].parse() {
                    Command::SetDim(d)
                } else {
                    Command::Unknown("dim requires a number".to_string())
                }
            } else {
                Command::Unknown("dim requires a dimension".to_string())
            }
        }
        "config" => Command::Config,
        "quit" | "exit" | "q" => Command::Quit,
        _ => Command::Unknown(parts[0].to_string()),
    }
}

fn print_help() {
    println!("Available commands:");
    println!("  help                  - Show this help message");
    println!("  load <file>          - Load input data from file");
    println!("  compute              - Compute attention with loaded data");
    println!("  type <type>          - Set attention type (scaled_dot, multi_head, hyperbolic, flash, linear, moe)");
    println!("  dim <size>           - Set dimension size");
    println!("  config               - Show current configuration");
    println!("  quit                 - Exit REPL");
}
