#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
#![cfg_attr(
    all(not(debug_assertions), target_os = "macos"),
    allow(unused_imports)
)]

fn main() {
    wifi_densepose_desktop::run();
}
