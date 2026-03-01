// build.rs - Build script for ruvector-postgres extension
// Detects CPU features at build time for SIMD optimizations

use std::env;

fn main() {
    // Get the target architecture
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=RUSTFLAGS");

    // Detect CPU features at build time
    // This allows for compile-time optimization when building for specific hardware

    if target_arch == "x86_64" || target_arch == "x86" {
        // Check for AVX-512 support
        if is_x86_feature_detected("avx512f") {
            println!("cargo:rustc-cfg=has_avx512");
            println!("cargo:rustc-cfg=has_avx2");
            println!("cargo:warning=Building with AVX-512 support");
        }
        // Check for AVX2 support
        else if is_x86_feature_detected("avx2") {
            println!("cargo:rustc-cfg=has_avx2");
            println!("cargo:warning=Building with AVX2 support");
        }
        // Check for SSE4.2 support (baseline for x86_64)
        else if is_x86_feature_detected("sse4.2") {
            println!("cargo:rustc-cfg=has_sse42");
            println!("cargo:warning=Building with SSE4.2 support");
        }
    } else if target_arch == "aarch64" {
        // ARM NEON is standard on AArch64
        println!("cargo:rustc-cfg=has_neon");
        println!("cargo:warning=Building with ARM NEON support");
    }

    // Enable native features if simd-native is enabled
    if env::var("CARGO_FEATURE_SIMD_NATIVE").is_ok() {
        println!("cargo:rustc-env=RUSTFLAGS=-C target-cpu=native");
        println!("cargo:warning=Building with native CPU optimizations (-C target-cpu=native)");
    }

    // PostgreSQL version detection
    if let Ok(pg_config) = env::var("PG_CONFIG") {
        println!("cargo:rerun-if-env-changed=PG_CONFIG");
        println!("cargo:warning=Using pg_config at: {}", pg_config);
    }

    // Print feature status
    print_feature_status();
}

fn is_x86_feature_detected(feature: &str) -> bool {
    // Check if the feature is enabled via RUSTFLAGS or target-cpu
    if let Ok(rustflags) = env::var("RUSTFLAGS") {
        if rustflags.contains("target-cpu=native") {
            return check_native_feature(feature);
        }
        if rustflags.contains(&format!("target-feature=+{}", feature)) {
            return true;
        }
    }

    // Check if building with specific feature flag
    match feature {
        "avx512f" => env::var("CARGO_FEATURE_SIMD_AVX512").is_ok(),
        "avx2" => env::var("CARGO_FEATURE_SIMD_AVX2").is_ok(),
        "sse4.2" => true, // Assume SSE4.2 is available on x86_64
        _ => false,
    }
}

fn check_native_feature(feature: &str) -> bool {
    // When building with target-cpu=native, use runtime detection
    // This is a best-effort check during build
    #[cfg(target_arch = "x86_64")]
    {
        match feature {
            "avx512f" => std::is_x86_feature_detected!("avx512f"),
            "avx2" => std::is_x86_feature_detected!("avx2"),
            "sse4.2" => std::is_x86_feature_detected!("sse4.2"),
            _ => false,
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = feature;
        false
    }
}

fn print_feature_status() {
    println!("cargo:warning=Feature Status:");

    // Index features
    if env::var("CARGO_FEATURE_INDEX_HNSW").is_ok() {
        println!("cargo:warning=  ✓ HNSW index enabled");
    }
    if env::var("CARGO_FEATURE_INDEX_IVFFLAT").is_ok() {
        println!("cargo:warning=  ✓ IVFFlat index enabled");
    }

    // Quantization features
    if env::var("CARGO_FEATURE_QUANTIZATION_SCALAR").is_ok() {
        println!("cargo:warning=  ✓ Scalar quantization enabled");
    }
    if env::var("CARGO_FEATURE_QUANTIZATION_PRODUCT").is_ok() {
        println!("cargo:warning=  ✓ Product quantization enabled");
    }
    if env::var("CARGO_FEATURE_QUANTIZATION_BINARY").is_ok() {
        println!("cargo:warning=  ✓ Binary quantization enabled");
    }

    // Optional features
    if env::var("CARGO_FEATURE_HYBRID_SEARCH").is_ok() {
        println!("cargo:warning=  ✓ Hybrid search enabled");
    }
    if env::var("CARGO_FEATURE_FILTERED_SEARCH").is_ok() {
        println!("cargo:warning=  ✓ Filtered search enabled");
    }
    if env::var("CARGO_FEATURE_NEON_COMPAT").is_ok() {
        println!("cargo:warning=  ✓ Neon compatibility enabled");
    }
}
