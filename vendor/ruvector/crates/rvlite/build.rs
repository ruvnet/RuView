use std::env;

fn main() {
    let target = env::var("TARGET").unwrap_or_default();

    if target.starts_with("wasm32") {
        // Configure getrandom for WASM
        // This tells getrandom 0.3 to use the wasm_js backend
        println!("cargo:rustc-env=GETRANDOM_BACKEND=wasm_js");
        println!("cargo:rustc-cfg=getrandom_backend=\"wasm_js\"");

        // Enable check-cfg to avoid warnings
        println!("cargo:rustc-check-cfg=cfg(getrandom_backend, values(\"wasm_js\"))");

        // Also set the cfg for getrandom 0.2 compatibility
        println!("cargo:rustc-cfg=getrandom_v0_2");
    }
}
