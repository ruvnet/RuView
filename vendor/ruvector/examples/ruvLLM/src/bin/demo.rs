//! RuvLLM Demo Binary
//!
//! Interactive demonstration of self-learning LLM capabilities.

use ruvllm::{Config, Feedback, Result, RuvLLM};
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("ruvllm=info".parse().unwrap()),
        )
        .init();

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║          RuvLLM - Self-Learning LLM Architecture              ║");
    println!("║     LFM2 Cortex + Ruvector Memory + FastGRNN Router           ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Build configuration
    let config = Config::builder()
        .embedding_dim(768)
        .router_hidden_dim(128)
        .hnsw_params(32, 200, 64)
        .learning_enabled(true)
        .build()?;

    println!("📋 Configuration:");
    println!("   Embedding dimension: {}", config.embedding.dimension);
    println!("   Router hidden dim:   {}", config.router.hidden_dim);
    println!("   HNSW M parameter:    {}", config.memory.hnsw_m);
    println!("   Learning enabled:    {}", config.learning.enabled);
    println!();

    println!("🚀 Initializing RuvLLM...");
    let llm = RuvLLM::new(config).await?;
    println!("✅ RuvLLM initialized successfully!");
    println!();

    // Interactive session
    println!("Enter queries (type 'quit' to exit, 'help' for commands):");
    println!("─────────────────────────────────────────────────────────────────");

    let session = llm.new_session();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("\n> ");
        stdout.flush().unwrap();

        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();
        let query = input.trim();

        if query.is_empty() {
            continue;
        }

        if query.eq_ignore_ascii_case("quit") || query.eq_ignore_ascii_case("exit") {
            println!("\n👋 Goodbye!");
            break;
        }

        if query.eq_ignore_ascii_case("help") {
            println!("\n📖 Commands:");
            println!("   quit/exit  - Exit the demo");
            println!("   help       - Show this help");
            println!("   <query>    - Ask a question");
            continue;
        }

        // Process query
        println!("\n⏳ Processing...");
        let start = std::time::Instant::now();

        match llm.query_session(&session, query).await {
            Ok(response) => {
                let elapsed = start.elapsed();
                println!("\n📝 Response:");
                println!("   {}", response.text);
                println!();
                println!("📈 Metadata:");
                println!("   Model used:    {:?}", response.routing_info.model);
                println!("   Context size:  {}", response.routing_info.context_size);
                println!("   Latency:       {:.2}ms", elapsed.as_secs_f64() * 1000.0);
                println!("   Confidence:    {:.2}%", response.confidence * 100.0);

                // Submit implicit feedback
                if response.text.len() > 50 {
                    let feedback = Feedback {
                        request_id: response.request_id.clone(),
                        rating: Some(4), // 4/5 rating
                        correction: None,
                        task_success: Some(true),
                    };
                    let _ = llm.feedback(feedback).await;
                }
            }
            Err(e) => {
                println!("\n❌ Error: {}", e);
            }
        }
    }

    Ok(())
}
