//! Browser-based MCP Transport Layer
//!
//! Provides MessagePort and BroadcastChannel transports for browser environments.
//!
//! ## Usage
//!
//! ```javascript
//! // Main thread
//! const worker = new Worker('edge-net-worker.js');
//! const transport = new WasmMcpTransport(worker);
//!
//! // Send request
//! const response = await transport.send({
//!   method: "tools/call",
//!   params: { name: "credits_balance", arguments: { node_id: "..." } }
//! });
//!
//! // Worker thread
//! import { WasmMcpServer, WasmMcpWorkerHandler } from '@ruvector/edge-net';
//!
//! const server = new WasmMcpServer();
//! const handler = new WasmMcpWorkerHandler(server, self);
//! handler.start();
//! ```

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MessageEvent, MessagePort, Worker, BroadcastChannel};
use serde_json::json;
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;

use super::{McpRequest, McpResponse, WasmMcpServer};

/// Pending request tracker
struct PendingRequest {
    resolve: js_sys::Function,
    reject: js_sys::Function,
}

/// Browser-based MCP transport using MessagePort
#[wasm_bindgen]
pub struct WasmMcpTransport {
    /// Message port for communication
    port: MessagePort,
    /// Pending requests awaiting responses
    pending: Arc<RwLock<HashMap<String, PendingRequest>>>,
    /// Request ID counter
    next_id: Arc<RwLock<u64>>,
}

#[wasm_bindgen]
impl WasmMcpTransport {
    /// Create transport from a Worker
    #[wasm_bindgen(constructor)]
    pub fn new(worker: &Worker) -> Result<WasmMcpTransport, JsValue> {
        // Create a message channel
        let channel = web_sys::MessageChannel::new()?;
        let port1 = channel.port1();
        let port2 = channel.port2();

        // Send port2 to worker
        let transfer = js_sys::Array::new();
        transfer.push(&port2);
        worker.post_message_with_transfer(&port2, &transfer)?;

        Ok(Self {
            port: port1,
            pending: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(RwLock::new(0)),
        })
    }

    /// Create transport from existing MessagePort
    #[wasm_bindgen(js_name = fromPort)]
    pub fn from_port(port: MessagePort) -> WasmMcpTransport {
        Self {
            port,
            pending: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(RwLock::new(0)),
        }
    }

    /// Initialize transport (set up message handler)
    #[wasm_bindgen]
    pub fn init(&self) -> Result<(), JsValue> {
        let pending = self.pending.clone();

        // Create message handler closure
        let handler = Closure::wrap(Box::new(move |event: MessageEvent| {
            let data = event.data();

            // Parse response
            if let Ok(json_str) = data.dyn_into::<js_sys::JsString>() {
                let json: String = json_str.into();
                if let Ok(response) = serde_json::from_str::<McpResponse>(&json) {
                    // Find pending request
                    if let Some(id) = &response.id {
                        let id_str = id.to_string();
                        let mut pending = pending.write();
                        if let Some(req) = pending.remove(&id_str) {
                            let response_js = JsValue::from_str(&json);
                            if response.error.is_some() {
                                let _ = req.reject.call1(&JsValue::NULL, &response_js);
                            } else {
                                let _ = req.resolve.call1(&JsValue::NULL, &response_js);
                            }
                        }
                    }
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);

        self.port.set_onmessage(Some(handler.as_ref().unchecked_ref()));
        handler.forget(); // Don't drop the closure

        self.port.start();
        Ok(())
    }

    /// Send an MCP request and get a Promise for the response
    #[wasm_bindgen]
    pub fn send(&self, request: JsValue) -> js_sys::Promise {
        let port = self.port.clone();
        let pending = self.pending.clone();
        let next_id = self.next_id.clone();

        js_sys::Promise::new(&mut move |resolve, reject| {
            // Generate request ID
            let id = {
                let mut counter = next_id.write();
                *counter += 1;
                *counter
            };

            // Parse and augment request
            let request: Result<McpRequest, _> = serde_wasm_bindgen::from_value(request.clone());
            let mut req = match request {
                Ok(r) => r,
                Err(e) => {
                    let _ = reject.call1(&JsValue::NULL, &JsValue::from_str(&e.to_string()));
                    return;
                }
            };

            // Set request ID
            req.id = Some(json!(id));
            req.jsonrpc = "2.0".to_string();

            // Store pending request
            {
                let mut pending = pending.write();
                pending.insert(id.to_string(), PendingRequest {
                    resolve: resolve.clone(),
                    reject: reject.clone(),
                });
            }

            // Send request
            let json = serde_json::to_string(&req).unwrap();
            if let Err(e) = port.post_message(&JsValue::from_str(&json)) {
                let _ = reject.call1(&JsValue::NULL, &e);
            }
        })
    }

    /// Close the transport
    #[wasm_bindgen]
    pub fn close(&self) {
        self.port.close();
    }
}

/// Worker-side handler for MCP requests
#[wasm_bindgen]
pub struct WasmMcpWorkerHandler {
    server: WasmMcpServer,
    port: Option<MessagePort>,
}

#[wasm_bindgen]
impl WasmMcpWorkerHandler {
    /// Create handler with MCP server
    #[wasm_bindgen(constructor)]
    pub fn new(server: WasmMcpServer) -> WasmMcpWorkerHandler {
        Self {
            server,
            port: None,
        }
    }

    /// Start handling messages (call in worker)
    #[wasm_bindgen]
    pub fn start(&mut self) -> Result<(), JsValue> {
        let server = std::mem::replace(&mut self.server, WasmMcpServer::new()?);
        let server = Arc::new(server);

        // In worker context, listen for port from main thread
        let global = js_sys::global();

        let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
            let data = event.data();

            // Check if this is a MessagePort being transferred
            if let Ok(port) = data.dyn_into::<MessagePort>() {
                // Set up handler for this port
                let server_for_handler = Arc::clone(&server);
                let port_for_response = port.clone();

                let handler = Closure::wrap(Box::new(move |event: MessageEvent| {
                    let data = event.data();

                    if let Ok(json_str) = data.dyn_into::<js_sys::JsString>() {
                        let json: String = json_str.into();
                        let server_for_async = Arc::clone(&server_for_handler);
                        let port_clone = port_for_response.clone();

                        // Parse and handle request
                        wasm_bindgen_futures::spawn_local(async move {
                            if let Ok(response) = server_for_async.handle_request(&json).await {
                                // Send response back
                                let _ = port_clone.post_message(&JsValue::from_str(&response));
                            }
                        });
                    }
                }) as Box<dyn FnMut(MessageEvent)>);

                port.set_onmessage(Some(handler.as_ref().unchecked_ref()));
                handler.forget();
                port.start();
            }
        }) as Box<dyn FnMut(MessageEvent)>);

        // Set global onmessage
        js_sys::Reflect::set(
            &global,
            &JsValue::from_str("onmessage"),
            onmessage.as_ref(),
        )?;
        onmessage.forget();

        Ok(())
    }
}

impl Clone for WasmMcpServer {
    fn clone(&self) -> Self {
        // Create a new server with shared state
        // NOTE: Identity is not cloned (contains private key)
        // NOTE: Learning engine is not cloned (state is complex)
        Self {
            identity: None,
            ledger: self.ledger.clone(),
            coherence: self.coherence.clone(),
            learning: None,
            config: self.config.clone(),
            request_counter: self.request_counter.clone(),
            rate_limit: self.rate_limit.clone(), // Share rate limit state
        }
    }
}

/// BroadcastChannel-based transport for multi-tab communication
#[wasm_bindgen]
pub struct WasmMcpBroadcast {
    channel: BroadcastChannel,
    server: Option<WasmMcpServer>,
}

#[wasm_bindgen]
impl WasmMcpBroadcast {
    /// Create a broadcast transport
    #[wasm_bindgen(constructor)]
    pub fn new(channel_name: &str) -> Result<WasmMcpBroadcast, JsValue> {
        let channel = BroadcastChannel::new(channel_name)?;

        Ok(Self {
            channel,
            server: None,
        })
    }

    /// Set as server mode (responds to requests)
    #[wasm_bindgen(js_name = setServer)]
    pub fn set_server(&mut self, server: WasmMcpServer) {
        self.server = Some(server);
    }

    /// Start listening for requests (server mode)
    #[wasm_bindgen]
    pub fn listen(&self) -> Result<(), JsValue> {
        if self.server.is_none() {
            return Err(JsValue::from_str("No server set"));
        }

        let channel = self.channel.clone();
        let server = self.server.as_ref().unwrap().clone();

        let handler = Closure::wrap(Box::new(move |event: MessageEvent| {
            let data = event.data();

            if let Ok(json_str) = data.dyn_into::<js_sys::JsString>() {
                let json: String = json_str.into();
                let channel_clone = channel.clone();
                let server_clone = server.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(response) = server_clone.handle_request(&json).await {
                        let _ = channel_clone.post_message(&JsValue::from_str(&response));
                    }
                });
            }
        }) as Box<dyn FnMut(MessageEvent)>);

        self.channel.set_onmessage(Some(handler.as_ref().unchecked_ref()));
        handler.forget();

        Ok(())
    }

    /// Send a request (client mode)
    #[wasm_bindgen]
    pub fn send(&self, request_json: &str) -> Result<(), JsValue> {
        self.channel.post_message(&JsValue::from_str(request_json))
    }

    /// Close the channel
    #[wasm_bindgen]
    pub fn close(&self) {
        self.channel.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Transport tests require browser environment
    #[cfg(target_arch = "wasm32")]
    mod wasm_tests {
        use super::*;
        use wasm_bindgen_test::*;

        wasm_bindgen_test_configure!(run_in_browser);

        #[wasm_bindgen_test]
        fn test_broadcast_creation() {
            let broadcast = WasmMcpBroadcast::new("test-channel").unwrap();
            broadcast.close();
        }
    }
}
