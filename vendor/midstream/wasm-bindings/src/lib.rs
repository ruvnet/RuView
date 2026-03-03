//! WASM bindings for MidStream Lean Agentic Learning System
//!
//! Provides WebSocket, SSE, and HTTP streaming support for browser and Node.js

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{WebSocket, EventSource, Request, RequestInit, Response};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;
use js_sys::{Function, Promise, Uint8Array};

// Re-export types from midstream
use midstream::{
    TemporalComparator, RealtimeScheduler, SchedulingPolicy, Priority,
    AttractorAnalyzer, TemporalNeuralSolver, MetaLearner, MetaLevel,
    ComparisonAlgorithm, TemporalFormula, TemporalTrace, TemporalState,
};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

/// Initialize panic hook for better error messages
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    log("MidStream WASM initialized");
}

// ============================================================================
// WebSocket Client
// ============================================================================

#[wasm_bindgen]
pub struct WebSocketClient {
    socket: WebSocket,
    on_message: Rc<RefCell<Option<Function>>>,
    on_error: Rc<RefCell<Option<Function>>>,
    on_close: Rc<RefCell<Option<Function>>>,
}

#[wasm_bindgen]
impl WebSocketClient {
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str) -> Result<WebSocketClient, JsValue> {
        let socket = WebSocket::new(url)?;
        socket.set_binary_type(web_sys::BinaryType::Arraybuffer);

        Ok(WebSocketClient {
            socket,
            on_message: Rc::new(RefCell::new(None)),
            on_error: Rc::new(RefCell::new(None)),
            on_close: Rc::new(RefCell::new(None)),
        })
    }

    pub fn connect(&self) -> Result<(), JsValue> {
        // Set up message handler
        let on_message_cb = self.on_message.clone();
        let onmessage_callback = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            if let Some(callback) = on_message_cb.borrow().as_ref() {
                if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                    let _ = callback.call1(&JsValue::NULL, &txt);
                } else if let Ok(array) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                    let uint8_array = Uint8Array::new(&array);
                    let _ = callback.call1(&JsValue::NULL, &uint8_array);
                }
            }
        }) as Box<dyn FnMut(web_sys::MessageEvent)>);

        self.socket.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        // Set up error handler
        let on_error_cb = self.on_error.clone();
        let onerror_callback = Closure::wrap(Box::new(move |e: web_sys::ErrorEvent| {
            if let Some(callback) = on_error_cb.borrow().as_ref() {
                let _ = callback.call1(&JsValue::NULL, &e.into());
            }
        }) as Box<dyn FnMut(web_sys::ErrorEvent)>);

        self.socket.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        // Set up close handler
        let on_close_cb = self.on_close.clone();
        let onclose_callback = Closure::wrap(Box::new(move |e: web_sys::CloseEvent| {
            if let Some(callback) = on_close_cb.borrow().as_ref() {
                let _ = callback.call1(&JsValue::NULL, &e.into());
            }
        }) as Box<dyn FnMut(web_sys::CloseEvent)>);

        self.socket.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
        onclose_callback.forget();

        Ok(())
    }

    pub fn send(&self, data: &str) -> Result<(), JsValue> {
        self.socket.send_with_str(data)
    }

    pub fn send_bytes(&self, data: &[u8]) -> Result<(), JsValue> {
        self.socket.send_with_u8_array(data)
    }

    pub fn close(&self) -> Result<(), JsValue> {
        self.socket.close()
    }

    pub fn set_on_message(&self, callback: Function) {
        *self.on_message.borrow_mut() = Some(callback);
    }

    pub fn set_on_error(&self, callback: Function) {
        *self.on_error.borrow_mut() = Some(callback);
    }

    pub fn set_on_close(&self, callback: Function) {
        *self.on_close.borrow_mut() = Some(callback);
    }

    pub fn ready_state(&self) -> u16 {
        self.socket.ready_state()
    }
}

// ============================================================================
// SSE Client
// ============================================================================

#[wasm_bindgen]
pub struct SSEClient {
    event_source: EventSource,
    on_message: Rc<RefCell<Option<Function>>>,
    on_error: Rc<RefCell<Option<Function>>>,
}

#[wasm_bindgen]
impl SSEClient {
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str) -> Result<SSEClient, JsValue> {
        let event_source = EventSource::new(url)?;

        Ok(SSEClient {
            event_source,
            on_message: Rc::new(RefCell::new(None)),
            on_error: Rc::new(RefCell::new(None)),
        })
    }

    pub fn connect(&self) -> Result<(), JsValue> {
        // Set up message handler
        let on_message_cb = self.on_message.clone();
        let onmessage_callback = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            if let Some(callback) = on_message_cb.borrow().as_ref() {
                if let Ok(data) = e.data().dyn_into::<js_sys::JsString>() {
                    let _ = callback.call1(&JsValue::NULL, &data);
                }
            }
        }) as Box<dyn FnMut(web_sys::MessageEvent)>);

        self.event_source.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        // Set up error handler
        let on_error_cb = self.on_error.clone();
        let onerror_callback = Closure::wrap(Box::new(move |e: web_sys::Event| {
            if let Some(callback) = on_error_cb.borrow().as_ref() {
                let _ = callback.call1(&JsValue::NULL, &e);
            }
        }) as Box<dyn FnMut(web_sys::Event)>);

        self.event_source.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        Ok(())
    }

    pub fn close(&self) {
        self.event_source.close();
    }

    pub fn set_on_message(&self, callback: Function) {
        *self.on_message.borrow_mut() = Some(callback);
    }

    pub fn set_on_error(&self, callback: Function) {
        *self.on_error.borrow_mut() = Some(callback);
    }

    pub fn ready_state(&self) -> u16 {
        self.event_source.ready_state()
    }
}

// ============================================================================
// HTTP Streaming Client
// ============================================================================

#[wasm_bindgen]
pub struct StreamingHTTPClient;

#[wasm_bindgen]
impl StreamingHTTPClient {
    #[wasm_bindgen(constructor)]
    pub fn new() -> StreamingHTTPClient {
        StreamingHTTPClient
    }

    pub async fn stream(&self, url: &str, on_chunk: Function) -> Result<(), JsValue> {
        let mut opts = RequestInit::new();
        opts.method("GET");

        let request = Request::new_with_str_and_init(url, &opts)?;

        let window = web_sys::window().ok_or("No window object")?;
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
        let resp: Response = resp_value.dyn_into()?;

        if let Some(body) = resp.body() {
            let reader = body.get_reader();

            loop {
                let result = JsFuture::from(reader.read()).await?;
                let done = js_sys::Reflect::get(&result, &JsValue::from_str("done"))?;

                if done.as_bool().unwrap_or(false) {
                    break;
                }

                let value = js_sys::Reflect::get(&result, &JsValue::from_str("value"))?;
                if let Ok(uint8_array) = value.dyn_into::<Uint8Array>() {
                    let _ = on_chunk.call1(&JsValue::NULL, &uint8_array);
                }
            }
        }

        Ok(())
    }
}

// ============================================================================
// Lean Agentic System WASM Bindings
// ============================================================================

#[wasm_bindgen]
pub struct TemporalComparatorWasm {
    inner: TemporalComparator<String>,
}

#[wasm_bindgen]
impl TemporalComparatorWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> TemporalComparatorWasm {
        TemporalComparatorWasm {
            inner: TemporalComparator::new(),
        }
    }

    pub fn compare(&mut self, seq1: JsValue, seq2: JsValue, algorithm: &str) -> Result<f64, JsValue> {
        let seq1_vec: Vec<String> = serde_wasm_bindgen::from_value(seq1)?;
        let seq2_vec: Vec<String> = serde_wasm_bindgen::from_value(seq2)?;

        let algo = match algorithm {
            "dtw" | "DTW" => ComparisonAlgorithm::DTW,
            "lcs" | "LCS" => ComparisonAlgorithm::LCS,
            "edit" | "EditDistance" => ComparisonAlgorithm::EditDistance,
            "corr" | "Correlation" => ComparisonAlgorithm::Correlation,
            _ => return Err(JsValue::from_str("Unknown algorithm")),
        };

        Ok(self.inner.compare(&seq1_vec, &seq2_vec, algo))
    }

    pub fn detect_pattern(&self, sequence: JsValue, pattern: JsValue) -> Result<JsValue, JsValue> {
        let seq_vec: Vec<String> = serde_wasm_bindgen::from_value(sequence)?;
        let pat_vec: Vec<String> = serde_wasm_bindgen::from_value(pattern)?;

        let positions = self.inner.detect_pattern(&seq_vec, &pat_vec);
        serde_wasm_bindgen::to_value(&positions).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn cache_stats(&self) -> Result<JsValue, JsValue> {
        let stats = self.inner.cache_stats();
        serde_wasm_bindgen::to_value(&stats).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

#[wasm_bindgen]
pub struct AttractorAnalyzerWasm {
    inner: AttractorAnalyzer,
}

#[wasm_bindgen]
impl AttractorAnalyzerWasm {
    #[wasm_bindgen(constructor)]
    pub fn new(embedding_dim: usize, time_delay: usize) -> AttractorAnalyzerWasm {
        AttractorAnalyzerWasm {
            inner: AttractorAnalyzer::new(embedding_dim, time_delay),
        }
    }

    pub fn analyze(&self, data: JsValue) -> Result<JsValue, JsValue> {
        let data_vec: Vec<f64> = serde_wasm_bindgen::from_value(data)?;

        let result = self.inner.analyze(&data_vec)
            .map_err(|e| JsValue::from_str(&e))?;

        serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

#[wasm_bindgen]
pub struct MetaLearnerWasm {
    inner: MetaLearner,
}

#[wasm_bindgen]
impl MetaLearnerWasm {
    #[wasm_bindgen(constructor)]
    pub fn new(max_history: usize) -> MetaLearnerWasm {
        MetaLearnerWasm {
            inner: MetaLearner::new(max_history),
        }
    }

    pub fn learn(&mut self, content: &str, reward: f64) {
        self.inner.learn(content.to_string(), reward);
    }

    pub fn ascend(&mut self) -> Result<String, JsValue> {
        self.inner.ascend()
            .map(|level| format!("{:?}", level))
            .map_err(|e| JsValue::from_str(&e))
    }

    pub fn descend(&mut self) -> Result<String, JsValue> {
        self.inner.descend()
            .map(|level| format!("{:?}", level))
            .map_err(|e| JsValue::from_str(&e))
    }

    pub fn get_summary(&self) -> Result<JsValue, JsValue> {
        let summary = self.inner.get_summary();
        serde_wasm_bindgen::to_value(&summary).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn current_level(&self) -> String {
        format!("{:?}", self.inner.current_level())
    }
}

// ============================================================================
// MidStream Agent
// ============================================================================

#[derive(Serialize, Deserialize)]
pub struct AgentConfig {
    pub max_history: usize,
    pub embedding_dim: usize,
    pub scheduling_policy: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_history: 1000,
            embedding_dim: 3,
            scheduling_policy: "EDF".to_string(),
        }
    }
}

#[wasm_bindgen]
pub struct MidStreamAgent {
    temporal: TemporalComparator<String>,
    attractor: AttractorAnalyzer,
    meta_learner: MetaLearner,
    config: AgentConfig,
}

#[wasm_bindgen]
impl MidStreamAgent {
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<MidStreamAgent, JsValue> {
        let config: AgentConfig = if config.is_undefined() {
            AgentConfig::default()
        } else {
            serde_wasm_bindgen::from_value(config)?
        };

        Ok(MidStreamAgent {
            temporal: TemporalComparator::new(),
            attractor: AttractorAnalyzer::new(config.embedding_dim, 1),
            meta_learner: MetaLearner::new(config.max_history),
            config,
        })
    }

    pub fn process_message(&mut self, message: &str) -> Result<JsValue, JsValue> {
        // Simple message processing with learning
        self.meta_learner.learn(message.to_string(), 0.8);

        let summary = self.meta_learner.get_summary();
        serde_wasm_bindgen::to_value(&summary).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn analyze_conversation(&mut self, messages: JsValue) -> Result<JsValue, JsValue> {
        let msg_vec: Vec<String> = serde_wasm_bindgen::from_value(messages)?;

        let mut result = serde_json::json!({
            "message_count": msg_vec.len(),
            "patterns": [],
            "meta_learning": self.meta_learner.get_summary(),
        });

        serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn get_status(&self) -> Result<JsValue, JsValue> {
        let status = serde_json::json!({
            "meta_level": format!("{:?}", self.meta_learner.current_level()),
            "config": {
                "max_history": self.config.max_history,
                "embedding_dim": self.config.embedding_dim,
            }
        });

        serde_wasm_bindgen::to_value(&status).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

// ============================================================================
// Benchmarking utilities
// ============================================================================

#[wasm_bindgen]
pub fn benchmark_dtw(size: usize, iterations: usize) -> f64 {
    let mut comparator = TemporalComparator::<i32>::new();
    let seq1: Vec<i32> = (0..size as i32).collect();
    let seq2: Vec<i32> = (0..size as i32).map(|x| x + 1).collect();

    let start = js_sys::Date::now();

    for _ in 0..iterations {
        comparator.compare(&seq1, &seq2, ComparisonAlgorithm::DTW);
    }

    let end = js_sys::Date::now();
    (end - start) / iterations as f64
}

#[wasm_bindgen]
pub fn benchmark_lcs(size: usize, iterations: usize) -> f64 {
    let mut comparator = TemporalComparator::<i32>::new();
    let seq1: Vec<i32> = (0..size as i32).collect();
    let seq2: Vec<i32> = (0..size as i32).map(|x| x + 1).collect();

    let start = js_sys::Date::now();

    for _ in 0..iterations {
        comparator.compare(&seq1, &seq2, ComparisonAlgorithm::LCS);
    }

    let end = js_sys::Date::now();
    (end - start) / iterations as f64
}

#[wasm_bindgen]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
