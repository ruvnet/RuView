pub mod commands;
pub mod domain;
pub mod state;

use commands::{discovery, flash, ota, provision, server, wasm};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            // Discovery
            discovery::discover_nodes,
            discovery::list_serial_ports,
            // Flash
            flash::flash_firmware,
            flash::flash_progress,
            // OTA
            ota::ota_update,
            ota::batch_ota_update,
            // WASM
            wasm::wasm_list,
            wasm::wasm_upload,
            wasm::wasm_control,
            // Server
            server::start_server,
            server::stop_server,
            server::server_status,
            // Provision
            provision::provision_node,
            provision::read_nvs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
