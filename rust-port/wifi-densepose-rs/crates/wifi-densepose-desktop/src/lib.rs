pub mod commands;
pub mod domain;
pub mod state;

use commands::{discovery, flash, ota, provision, server, settings, wasm};

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
            flash::verify_firmware,
            flash::check_espflash,
            flash::supported_chips,
            // OTA
            ota::ota_update,
            ota::batch_ota_update,
            ota::check_ota_endpoint,
            // WASM
            wasm::wasm_list,
            wasm::wasm_upload,
            wasm::wasm_control,
            wasm::wasm_info,
            wasm::wasm_stats,
            wasm::check_wasm_support,
            // Server
            server::start_server,
            server::stop_server,
            server::server_status,
            server::restart_server,
            server::server_logs,
            // Provision
            provision::provision_node,
            provision::read_nvs,
            provision::erase_nvs,
            provision::validate_config,
            provision::generate_mesh_configs,
            // Settings
            settings::get_settings,
            settings::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
