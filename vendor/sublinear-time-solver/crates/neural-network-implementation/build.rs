fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple build script without vergen for now
    println!("cargo:rustc-env=VERGEN_BUILD_TIMESTAMP={}", chrono::Utc::now().to_rfc3339());
    println!("cargo:rustc-env=VERGEN_GIT_SHA=unknown");
    println!("cargo:rustc-env=VERGEN_CARGO_PKG_VERSION={}", env!("CARGO_PKG_VERSION"));

    Ok(())
}