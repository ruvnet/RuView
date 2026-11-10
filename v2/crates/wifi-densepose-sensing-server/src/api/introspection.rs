//! Real-time CSI introspection WebSocket and snapshot REST endpoints (ADR-099).

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use tracing::info;
use crate::SharedState;

/// Create sub-router for introspection endpoints.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/ws/introspection", get(ws_introspection_handler))
        .route(
            "/api/v1/introspection/snapshot",
            get(api_introspection_snapshot),
        )
}

pub(crate) async fn ws_introspection_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_introspection_client(socket, state))
}

async fn handle_ws_introspection_client(mut socket: WebSocket, state: SharedState) {
    let mut rx = {
        let s = state.read().await;
        s.intro_tx.subscribe()
    };

    info!("WebSocket client connected (introspection)");

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(json) => {
                        if socket.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }

    info!("WebSocket client disconnected (introspection)");
}

/// `GET /api/v1/introspection/snapshot` — one-shot poll for the latest
/// per-frame snapshot (regime, Lyapunov, top-k similarity).
pub(crate) async fn api_introspection_snapshot(
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let s = state.read().await;
    Json(s.intro.snapshot().clone())
}
