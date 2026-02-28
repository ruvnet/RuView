//! MCP Transport implementations (stdio and SSE)

use crate::{McpRequest, McpServer};
use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// STDIO transport for MCP
pub struct StdioTransport {
    server: McpServer,
}

impl StdioTransport {
    pub fn new(server: McpServer) -> Self {
        Self { server }
    }

    /// Run the stdio transport (reads from stdin, writes to stdout)
    pub async fn run(&self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await?;

            if bytes_read == 0 {
                // EOF
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Parse request
            match serde_json::from_str::<McpRequest>(trimmed) {
                Ok(request) => {
                    // Handle request
                    let response = self.server.handle_request(request).await;

                    // Write response
                    let response_json = serde_json::to_string(&response)?;
                    stdout.write_all(response_json.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                }
                Err(e) => {
                    eprintln!("Failed to parse request: {}", e);
                }
            }
        }

        Ok(())
    }
}

/// SSE (Server-Sent Events) transport for MCP
#[cfg(feature = "sse")]
pub mod sse {
    use super::*;
    use axum::{
        extract::State,
        response::sse::{Event, KeepAlive, Sse},
        routing::{get, post},
        Json, Router,
    };
    use std::sync::Arc;
    use tokio_stream::StreamExt as _;

    pub async fn run_sse_server(server: McpServer, addr: &str) -> Result<()> {
        let app = Router::new()
            .route("/mcp", post(handle_mcp_request))
            .route("/mcp/stream", get(handle_mcp_stream))
            .with_state(Arc::new(server));

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }

    async fn handle_mcp_request(
        State(server): State<Arc<McpServer>>,
        Json(request): Json<McpRequest>,
    ) -> Json<McpResponse> {
        let response = server.handle_request(request).await;
        Json(response)
    }

    async fn handle_mcp_stream(
        State(_server): State<Arc<McpServer>>,
    ) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
        let stream = tokio_stream::iter(vec![
            Ok(Event::default().data("connected")),
        ]);

        Sse::new(stream).keep_alive(KeepAlive::default())
    }
}
