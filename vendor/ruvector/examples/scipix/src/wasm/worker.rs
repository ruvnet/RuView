//! Web Worker support for off-main-thread OCR processing

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};

use crate::wasm::api::ScipixWasm;
use crate::wasm::types::RecognitionFormat;

static WORKER_INSTANCE: OnceCell<Arc<ScipixWasm>> = OnceCell::new();

/// Messages sent from main thread to worker
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkerRequest {
    /// Initialize the worker
    Init,

    /// Process an image
    Process {
        id: String,
        image_data: Vec<u8>,
        format: String,
    },

    /// Process base64 image
    ProcessBase64 {
        id: String,
        base64: String,
        format: String,
    },

    /// Batch process images
    BatchProcess {
        id: String,
        images: Vec<Vec<u8>>,
        format: String,
    },

    /// Terminate worker
    Terminate,
}

/// Messages sent from worker to main thread
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkerResponse {
    /// Worker is ready
    Ready,

    /// Processing started
    Started { id: String },

    /// Processing progress
    Progress {
        id: String,
        processed: usize,
        total: usize,
    },

    /// Processing completed successfully
    Success {
        id: String,
        result: serde_json::Value,
    },

    /// Processing failed
    Error { id: String, error: String },

    /// Worker terminated
    Terminated,
}

/// Initialize the worker
#[wasm_bindgen(js_name = initWorker)]
pub async fn init_worker() -> Result<(), JsValue> {
    let instance = ScipixWasm::new().await?;
    WORKER_INSTANCE
        .set(Arc::new(instance))
        .map_err(|_| JsValue::from_str("Worker already initialized"))?;

    post_response(WorkerResponse::Ready)?;
    Ok(())
}

/// Handle messages from the main thread
#[wasm_bindgen(js_name = handleWorkerMessage)]
pub async fn handle_worker_message(event: MessageEvent) -> Result<(), JsValue> {
    let data = event.data();

    let request: WorkerRequest = serde_wasm_bindgen::from_value(data)
        .map_err(|e| JsValue::from_str(&format!("Invalid message: {}", e)))?;

    match request {
        WorkerRequest::Init => {
            init_worker().await?;
        }

        WorkerRequest::Process {
            id,
            image_data,
            format,
        } => {
            process_image(id, image_data, format).await?;
        }

        WorkerRequest::ProcessBase64 { id, base64, format } => {
            process_base64(id, base64, format).await?;
        }

        WorkerRequest::BatchProcess { id, images, format } => {
            process_batch(id, images, format).await?;
        }

        WorkerRequest::Terminate => {
            post_response(WorkerResponse::Terminated)?;
        }
    }

    Ok(())
}

async fn process_image(id: String, image_data: Vec<u8>, format: String) -> Result<(), JsValue> {
    post_response(WorkerResponse::Started { id: id.clone() })?;

    let instance = WORKER_INSTANCE
        .get()
        .ok_or_else(|| JsValue::from_str("Worker not initialized"))?;

    let mut worker_instance = ScipixWasm::new().await?;
    worker_instance.set_format(&format);

    match worker_instance.recognize(&image_data).await {
        Ok(result) => {
            let json_result: serde_json::Value = serde_wasm_bindgen::from_value(result)?;
            post_response(WorkerResponse::Success {
                id,
                result: json_result,
            })?;
        }
        Err(e) => {
            post_response(WorkerResponse::Error {
                id,
                error: format!("{:?}", e),
            })?;
        }
    }

    Ok(())
}

async fn process_base64(id: String, base64: String, format: String) -> Result<(), JsValue> {
    post_response(WorkerResponse::Started { id: id.clone() })?;

    let mut worker_instance = ScipixWasm::new().await?;
    worker_instance.set_format(&format);

    match worker_instance.recognize_base64(&base64).await {
        Ok(result) => {
            let json_result: serde_json::Value = serde_wasm_bindgen::from_value(result)?;
            post_response(WorkerResponse::Success {
                id,
                result: json_result,
            })?;
        }
        Err(e) => {
            post_response(WorkerResponse::Error {
                id,
                error: format!("{:?}", e),
            })?;
        }
    }

    Ok(())
}

async fn process_batch(id: String, images: Vec<Vec<u8>>, format: String) -> Result<(), JsValue> {
    post_response(WorkerResponse::Started { id: id.clone() })?;

    let total = images.len();
    let mut results = Vec::new();

    let mut worker_instance = ScipixWasm::new().await?;
    worker_instance.set_format(&format);

    for (idx, image_data) in images.into_iter().enumerate() {
        // Report progress
        post_response(WorkerResponse::Progress {
            id: id.clone(),
            processed: idx,
            total,
        })?;

        match worker_instance.recognize(&image_data).await {
            Ok(result) => {
                let json_result: serde_json::Value = serde_wasm_bindgen::from_value(result)?;
                results.push(json_result);
            }
            Err(e) => {
                web_sys::console::warn_1(&JsValue::from_str(&format!(
                    "Failed to process image {}: {:?}",
                    idx, e
                )));
                results.push(serde_json::Value::Null);
            }
        }
    }

    post_response(WorkerResponse::Success {
        id,
        result: serde_json::json!({ "results": results }),
    })?;

    Ok(())
}

fn post_response(response: WorkerResponse) -> Result<(), JsValue> {
    let global = js_sys::global().dyn_into::<DedicatedWorkerGlobalScope>()?;
    let message = serde_wasm_bindgen::to_value(&response)?;
    global.post_message(&message)?;
    Ok(())
}

/// Setup worker message listener
#[wasm_bindgen(js_name = setupWorker)]
pub fn setup_worker() -> Result<(), JsValue> {
    let global = js_sys::global().dyn_into::<DedicatedWorkerGlobalScope>()?;

    let closure = Closure::wrap(Box::new(move |event: MessageEvent| {
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = handle_worker_message(event).await {
                web_sys::console::error_1(&e);
            }
        });
    }) as Box<dyn FnMut(MessageEvent)>);

    global.set_onmessage(Some(closure.as_ref().unchecked_ref()));
    closure.forget(); // Keep closure alive

    Ok(())
}
