//! Build script: when the `tch-backend` feature binds libtorch from the system
//! PyTorch install (`LIBTORCH_USE_PYTORCH=1`), embed an rpath to the PyTorch
//! `lib` directory so the produced binaries and test executables can locate
//! `libtorch_cpu` at runtime. torch-sys only adds the rpath to its own
//! cc-built static library, which does not propagate to downstream binaries on
//! macOS/Linux. A strict no-op otherwise (default / `--no-default-features`
//! builds, including the Python wheel, are unaffected).
fn main() {
    println!("cargo:rerun-if-env-changed=LIBTORCH_USE_PYTORCH");
    println!("cargo:rerun-if-env-changed=PYTHON_SYS_EXECUTABLE");
    if std::env::var_os("CARGO_FEATURE_TCH_BACKEND").is_none()
        || std::env::var_os("LIBTORCH_USE_PYTORCH").is_none()
    {
        return;
    }
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" && target_os != "linux" {
        return;
    }
    // Same interpreter selection torch-sys uses, defaulting to python3.
    let python = std::env::var("PYTHON_SYS_EXECUTABLE").unwrap_or_else(|_| "python3".into());
    let output = std::process::Command::new(python)
        .args([
            "-c",
            "import os, torch; print(os.path.join(os.path.dirname(torch.__file__), 'lib'))",
        ])
        .output();
    if let Ok(output) = output {
        if output.status.success() {
            let lib_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !lib_dir.is_empty() {
                // Plain rustc-link-arg (not -bins/-tests): it is the only
                // variant that also reaches the lib unit-test binary.
                println!("cargo:rustc-link-arg=-Wl,-rpath,{lib_dir}");
            }
        }
    }
}
