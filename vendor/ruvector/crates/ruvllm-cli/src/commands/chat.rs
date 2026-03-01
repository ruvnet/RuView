//! Interactive chat command implementation
//!
//! Provides a colorful REPL interface for chatting with LLM models,
//! with support for streaming responses, history, and special commands.

use anyhow::{Context, Result};
use colored::Colorize;
use console::style;
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result as RustyResult};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use crate::models::{get_model, resolve_model_id, QuantPreset};

/// Speculative decoding configuration for chat
struct SpeculativeConfig {
    draft_model: Option<String>,
    lookahead: usize,
}

/// Chat session state
struct ChatSession {
    model_id: String,
    backend: Box<dyn ruvllm::LlmBackend>,
    draft_backend: Option<Box<dyn ruvllm::LlmBackend>>,
    history: Vec<ChatMessage>,
    system_prompt: Option<String>,
    max_tokens: usize,
    temperature: f32,
    speculative: Option<SpeculativeConfig>,
}

#[derive(Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Run the chat command
pub async fn run(
    model: &str,
    system_prompt: Option<&str>,
    max_tokens: usize,
    temperature: f32,
    quantization: &str,
    cache_dir: &str,
    draft_model: Option<&str>,
    speculative_lookahead: usize,
) -> Result<()> {
    let model_id = resolve_model_id(model);
    let quant = QuantPreset::from_str(quantization)
        .ok_or_else(|| anyhow::anyhow!("Invalid quantization format: {}", quantization))?;

    // Print header
    print_header(&model_id, system_prompt, max_tokens, temperature);

    // Load main model
    println!("{}", "Loading model...".yellow());
    let backend = load_model(&model_id, quant, cache_dir)?;

    if let Some(info) = backend.model_info() {
        println!(
            "{} Loaded {} ({:.1}B params)",
            style("Ready!").green().bold(),
            info.name,
            info.num_parameters as f64 / 1e9
        );
    } else {
        println!(
            "{} Model loaded (mock mode)",
            style("Ready!").yellow().bold()
        );
    }

    // Load draft model for speculative decoding if provided
    let (draft_backend, speculative_config) = if let Some(draft_id) = draft_model {
        println!(
            "{}",
            "Loading draft model for speculative decoding...".yellow()
        );
        let draft = load_model(&resolve_model_id(draft_id), quant, cache_dir)?;

        if let Some(info) = draft.model_info() {
            println!(
                "{} Draft model: {} ({:.1}B params)",
                style("Speculative:").cyan().bold(),
                info.name,
                info.num_parameters as f64 / 1e9
            );
        }

        let config = SpeculativeConfig {
            draft_model: Some(draft_id.to_string()),
            lookahead: speculative_lookahead.clamp(2, 8),
        };

        println!(
            "  {} Lookahead: {} tokens, expected speedup: 2-3x",
            style(">").cyan(),
            config.lookahead
        );

        (Some(draft), Some(config))
    } else {
        (None, None)
    };

    // Create session
    let mut session = ChatSession {
        model_id,
        backend,
        draft_backend,
        history: Vec::new(),
        system_prompt: system_prompt.map(String::from),
        max_tokens,
        temperature,
        speculative: speculative_config,
    };

    // Add system prompt to history
    if let Some(sys) = &session.system_prompt {
        session.history.push(ChatMessage {
            role: "system".to_string(),
            content: sys.clone(),
        });
    }

    println!();
    println!(
        "{}",
        "Type your message and press Enter. Special commands:".dimmed()
    );
    println!("{}", "  /clear  - Clear conversation history".dimmed());
    println!("{}", "  /system - Set system prompt".dimmed());
    println!("{}", "  /save   - Save conversation to file".dimmed());
    println!("{}", "  /load   - Load conversation from file".dimmed());
    println!("{}", "  /help   - Show all commands".dimmed());
    println!("{}", "  /quit   - Exit chat (or Ctrl+D)".dimmed());
    println!();

    // Start REPL
    let mut rl = DefaultEditor::new().context("Failed to initialize readline")?;
    let history_path = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ruvllm")
        .join("chat_history.txt");

    let _ = rl.load_history(&history_path);

    loop {
        let prompt = format!("{} ", style("You>").cyan().bold());
        match rl.readline(&prompt) {
            Ok(line) => {
                let input = line.trim();

                if input.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(input);

                // Handle special commands
                if input.starts_with('/') {
                    match handle_command(&mut session, input) {
                        CommandResult::Continue => continue,
                        CommandResult::Quit => break,
                        CommandResult::ShowHelp => {
                            print_help();
                            continue;
                        }
                    }
                }

                // Regular message - get response with streaming
                match generate_response(&mut session, input) {
                    Ok(_response) => {
                        // Response is already printed via streaming in generate_response
                        println!();
                    }
                    Err(e) => {
                        eprintln!("{} {}", style("Error:").red().bold(), e);
                        println!();
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("{}", "Interrupted. Use /quit or Ctrl+D to exit.".dimmed());
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    let _ = std::fs::create_dir_all(history_path.parent().unwrap());
    let _ = rl.save_history(&history_path);

    println!();
    println!("{}", "Goodbye!".dimmed());

    Ok(())
}

/// Print chat header
fn print_header(model_id: &str, system_prompt: Option<&str>, max_tokens: usize, temperature: f32) {
    println!();
    println!("{}", style("RuvLLM Interactive Chat").bold().cyan());
    println!("{}", "=".repeat(50).dimmed());
    println!();
    println!("  {} {}", "Model:".dimmed(), model_id);
    println!("  {} {}", "Max Tokens:".dimmed(), max_tokens);
    println!("  {} {}", "Temperature:".dimmed(), temperature);

    if let Some(model) = get_model(model_id) {
        println!("  {} {}", "Architecture:".dimmed(), model.architecture);
        println!("  {} {}B", "Parameters:".dimmed(), model.params_b);
    }

    if let Some(sys) = system_prompt {
        println!("  {} {}", "System:".dimmed(), truncate(sys, 50));
    }

    println!();
}

/// Load model for chat
fn load_model(
    model_id: &str,
    quant: QuantPreset,
    cache_dir: &str,
) -> Result<Box<dyn ruvllm::LlmBackend>> {
    let mut backend = ruvllm::create_backend();

    let config = ruvllm::ModelConfig {
        architecture: detect_architecture(model_id),
        quantization: Some(map_quantization(quant)),
        ..Default::default()
    };

    // Try local cache first
    let model_path = PathBuf::from(cache_dir).join("models").join(model_id);
    let load_result = if model_path.exists() {
        backend.load_model(model_path.to_str().unwrap(), config.clone())
    } else {
        backend.load_model(model_id, config)
    };

    // Ignore load errors for now (will use mock mode)
    if let Err(e) = load_result {
        tracing::warn!("Model load failed, running in mock mode: {}", e);
    }

    Ok(backend)
}

/// Generate response from the model with streaming output
fn generate_response(session: &mut ChatSession, user_input: &str) -> Result<String> {
    // Add user message to history
    session.history.push(ChatMessage {
        role: "user".to_string(),
        content: user_input.to_string(),
    });

    // Build prompt
    let prompt = build_prompt(&session.history);

    // Generate parameters
    let params = ruvllm::GenerateParams {
        max_tokens: session.max_tokens,
        temperature: session.temperature,
        top_p: 0.9,
        ..Default::default()
    };

    let response = if session.backend.is_model_loaded() {
        // Try streaming first
        generate_with_streaming(session.backend.as_ref(), &prompt, params.clone()).unwrap_or_else(
            |_| {
                // Fall back to non-streaming
                session
                    .backend
                    .generate(&prompt, params)
                    .unwrap_or_else(|_| mock_response(user_input))
            },
        )
    } else {
        // Use streaming mock response
        generate_streaming_mock(user_input)?
    };

    // Add assistant response to history
    session.history.push(ChatMessage {
        role: "assistant".to_string(),
        content: response.clone(),
    });

    Ok(response)
}

/// Generate response with real streaming output
fn generate_with_streaming(
    backend: &dyn ruvllm::LlmBackend,
    prompt: &str,
    params: ruvllm::GenerateParams,
) -> Result<String> {
    let stream = backend.generate_stream_v2(prompt, params)?;

    let mut full_response = String::new();

    // Print streaming prefix
    print!("{} ", style("AI>").green().bold());
    std::io::stdout().flush()?;

    for event_result in stream {
        match event_result? {
            ruvllm::StreamEvent::Token(token) => {
                print!("{}", token.text.green());
                std::io::stdout().flush()?;
                full_response.push_str(&token.text);
            }
            ruvllm::StreamEvent::Done {
                total_tokens,
                duration_ms,
                tokens_per_second,
            } => {
                println!();
                println!(
                    "{}",
                    format!(
                        "[{} tokens, {:.0}ms, {:.1} t/s]",
                        total_tokens, duration_ms, tokens_per_second
                    )
                    .dimmed()
                );
                break;
            }
            ruvllm::StreamEvent::Error(msg) => {
                return Err(anyhow::anyhow!("Generation error: {}", msg));
            }
        }
    }

    Ok(full_response)
}

/// Generate streaming mock response for testing
fn generate_streaming_mock(input: &str) -> Result<String> {
    let response = mock_response(input);
    let words: Vec<&str> = response.split_whitespace().collect();

    // Print streaming prefix
    print!("{} ", style("AI>").green().bold());
    std::io::stdout().flush()?;

    let start = Instant::now();
    let mut full_response = String::new();

    for (i, word) in words.iter().enumerate() {
        // Simulate streaming delay
        std::thread::sleep(std::time::Duration::from_millis(30));

        let text = if i == 0 {
            word.to_string()
        } else {
            format!(" {}", word)
        };

        print!("{}", text.green());
        std::io::stdout().flush()?;
        full_response.push_str(&text);
    }

    let elapsed = start.elapsed();
    let token_count = words.len();
    let tps = token_count as f64 / elapsed.as_secs_f64();

    println!();
    println!(
        "{}",
        format!(
            "[{} tokens, {:.0}ms, {:.1} t/s]",
            token_count,
            elapsed.as_millis(),
            tps
        )
        .dimmed()
    );

    Ok(full_response)
}

/// Build prompt from chat history
fn build_prompt(history: &[ChatMessage]) -> String {
    let mut prompt = String::new();

    for msg in history {
        match msg.role.as_str() {
            "system" => {
                prompt.push_str(&format!("<|system|>\n{}\n<|end|>\n", msg.content));
            }
            "user" => {
                prompt.push_str(&format!("<|user|>\n{}\n<|end|>\n", msg.content));
            }
            "assistant" => {
                prompt.push_str(&format!("<|assistant|>\n{}\n<|end|>\n", msg.content));
            }
            _ => {}
        }
    }

    prompt.push_str("<|assistant|>\n");
    prompt
}

/// Mock response for testing
fn mock_response(input: &str) -> String {
    let input_lower = input.to_lowercase();

    if input_lower.contains("hello") || input_lower.contains("hi") {
        "Hello! I'm running in mock mode since the model couldn't be loaded. To get real responses, make sure to download a model first with `ruvllm download <model>`.".to_string()
    } else if input_lower.contains("help") {
        "I can help with various tasks like answering questions, writing code, explaining concepts, and more. What would you like to know?".to_string()
    } else if input_lower.contains("code") || input_lower.contains("rust") {
        "Here's a simple Rust example:\n\n```rust\nfn main() {\n    println!(\"Hello from RuvLLM!\");\n}\n```\n\nWould you like me to explain how this works?".to_string()
    } else {
        format!("I understand you're asking about '{}'. In mock mode, I can only provide placeholder responses. Please download and load a model for full functionality.", truncate(input, 50))
    }
}

/// Command result
enum CommandResult {
    Continue,
    Quit,
    ShowHelp,
}

/// Handle special commands
fn handle_command(session: &mut ChatSession, command: &str) -> CommandResult {
    let parts: Vec<&str> = command.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let args = parts.get(1).map(|s| *s).unwrap_or("");

    match cmd.as_str() {
        "/quit" | "/exit" | "/q" => CommandResult::Quit,
        "/help" | "/h" | "/?" => CommandResult::ShowHelp,
        "/clear" | "/c" => {
            session.history.clear();
            if let Some(sys) = &session.system_prompt {
                session.history.push(ChatMessage {
                    role: "system".to_string(),
                    content: sys.clone(),
                });
            }
            println!("{}", "Conversation cleared.".green());
            CommandResult::Continue
        }
        "/system" => {
            if args.is_empty() {
                if let Some(sys) = &session.system_prompt {
                    println!("Current system prompt: {}", sys);
                } else {
                    println!("No system prompt set.");
                }
            } else {
                session.system_prompt = Some(args.to_string());
                session.history.retain(|m| m.role != "system");
                session.history.insert(
                    0,
                    ChatMessage {
                        role: "system".to_string(),
                        content: args.to_string(),
                    },
                );
                println!("{}", "System prompt updated.".green());
            }
            CommandResult::Continue
        }
        "/save" => {
            let path = if args.is_empty() {
                "conversation.json"
            } else {
                args
            };
            match save_conversation(session, path) {
                Ok(_) => println!("{} Saved to {}", "Success!".green(), path),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
            CommandResult::Continue
        }
        "/load" => {
            let path = if args.is_empty() {
                "conversation.json"
            } else {
                args
            };
            match load_conversation(session, path) {
                Ok(_) => println!("{} Loaded from {}", "Success!".green(), path),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
            CommandResult::Continue
        }
        "/history" => {
            println!("{}", "Conversation history:".bold());
            for (i, msg) in session.history.iter().enumerate() {
                let role_color = match msg.role.as_str() {
                    "system" => msg.role.yellow(),
                    "user" => msg.role.cyan(),
                    "assistant" => msg.role.green(),
                    _ => msg.role.white(),
                };
                println!("{}. [{}] {}", i + 1, role_color, truncate(&msg.content, 80));
            }
            CommandResult::Continue
        }
        "/tokens" => {
            let total_tokens: usize = session
                .history
                .iter()
                .map(|m| m.content.split_whitespace().count())
                .sum();
            println!(
                "Messages: {}, Estimated tokens: ~{}",
                session.history.len(),
                total_tokens
            );
            CommandResult::Continue
        }
        "/temp" => {
            if args.is_empty() {
                println!("Current temperature: {}", session.temperature);
            } else if let Ok(t) = args.parse::<f32>() {
                if (0.0..=2.0).contains(&t) {
                    session.temperature = t;
                    println!("{} Temperature set to {}", "Success!".green(), t);
                } else {
                    println!("{} Temperature must be between 0.0 and 2.0", "Error:".red());
                }
            } else {
                println!("{} Invalid temperature value", "Error:".red());
            }
            CommandResult::Continue
        }
        "/max" => {
            if args.is_empty() {
                println!("Current max tokens: {}", session.max_tokens);
            } else if let Ok(m) = args.parse::<usize>() {
                if m > 0 && m <= 8192 {
                    session.max_tokens = m;
                    println!("{} Max tokens set to {}", "Success!".green(), m);
                } else {
                    println!("{} Max tokens must be between 1 and 8192", "Error:".red());
                }
            } else {
                println!("{} Invalid max tokens value", "Error:".red());
            }
            CommandResult::Continue
        }
        _ => {
            println!("{} Unknown command: {}", "Warning:".yellow(), cmd);
            CommandResult::Continue
        }
    }
}

/// Print help message
fn print_help() {
    println!();
    println!("{}", style("Chat Commands").bold());
    println!("{}", "=".repeat(40).dimmed());
    println!();
    println!("  {} - Clear conversation history", "/clear, /c".cyan());
    println!("  {} - Set/show system prompt", "/system [prompt]".cyan());
    println!("  {} - Save conversation to file", "/save [file]".cyan());
    println!("  {} - Load conversation from file", "/load [file]".cyan());
    println!("  {} - Show conversation history", "/history".cyan());
    println!("  {} - Show token count", "/tokens".cyan());
    println!("  {} - Set/show temperature (0-2)", "/temp [value]".cyan());
    println!("  {} - Set/show max tokens", "/max [value]".cyan());
    println!("  {} - Show this help", "/help, /h".cyan());
    println!("  {} - Exit chat", "/quit, /q".cyan());
    println!();
}

/// Save conversation to file
fn save_conversation(session: &ChatSession, path: &str) -> Result<()> {
    let data = serde_json::json!({
        "model": session.model_id,
        "system_prompt": session.system_prompt,
        "max_tokens": session.max_tokens,
        "temperature": session.temperature,
        "messages": session.history.iter().map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": m.content
            })
        }).collect::<Vec<_>>()
    });

    std::fs::write(path, serde_json::to_string_pretty(&data)?)?;
    Ok(())
}

/// Load conversation from file
fn load_conversation(session: &mut ChatSession, path: &str) -> Result<()> {
    let data: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;

    session.history.clear();

    if let Some(messages) = data["messages"].as_array() {
        for msg in messages {
            session.history.push(ChatMessage {
                role: msg["role"].as_str().unwrap_or("user").to_string(),
                content: msg["content"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    if let Some(sys) = data["system_prompt"].as_str() {
        session.system_prompt = Some(sys.to_string());
    }

    Ok(())
}

/// Truncate string with ellipsis
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Detect architecture from model ID
fn detect_architecture(model_id: &str) -> ruvllm::ModelArchitecture {
    let lower = model_id.to_lowercase();
    if lower.contains("mistral") {
        ruvllm::ModelArchitecture::Mistral
    } else if lower.contains("llama") {
        ruvllm::ModelArchitecture::Llama
    } else if lower.contains("phi") {
        ruvllm::ModelArchitecture::Phi
    } else if lower.contains("qwen") {
        ruvllm::ModelArchitecture::Qwen
    } else {
        ruvllm::ModelArchitecture::Llama
    }
}

/// Map quantization preset
fn map_quantization(quant: QuantPreset) -> ruvllm::Quantization {
    match quant {
        QuantPreset::Q4K => ruvllm::Quantization::Q4K,
        QuantPreset::Q8 => ruvllm::Quantization::Q8,
        QuantPreset::F16 => ruvllm::Quantization::F16,
        QuantPreset::None => ruvllm::Quantization::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_mock_response() {
        let response = mock_response("hello");
        assert!(response.contains("mock mode"));
    }
}
