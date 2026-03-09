use serde::{Deserialize, Serialize};

/// List WASM modules loaded on a specific node.
#[tauri::command]
pub async fn wasm_list(node_ip: String) -> Result<Vec<WasmModuleInfo>, String> {
    let _ = node_ip;
    Ok(vec![])
}

/// Upload a WASM module to a node.
#[tauri::command]
pub async fn wasm_upload(
    node_ip: String,
    wasm_path: String,
) -> Result<WasmUploadResult, String> {
    let _ = (node_ip, wasm_path);
    Ok(WasmUploadResult {
        success: true,
        module_id: "stub-module-0".into(),
        message: "Stub: WASM upload not yet implemented".into(),
    })
}

/// Start, stop, or unload a WASM module on a node.
#[tauri::command]
pub async fn wasm_control(
    node_ip: String,
    module_id: String,
    action: String,
) -> Result<(), String> {
    let _ = (node_ip, module_id, action);
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmModuleInfo {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmUploadResult {
    pub success: bool,
    pub module_id: String,
    pub message: String,
}
