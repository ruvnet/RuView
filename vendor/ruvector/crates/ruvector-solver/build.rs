fn main() {
    // Emit cfg flags for SIMD detection at build time
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(target_arch = "x86_64")]
    {
        if std::env::var("CARGO_CFG_TARGET_FEATURE").map_or(false, |f| f.contains("avx2")) {
            println!("cargo:rustc-cfg=has_avx2");
        }
        if std::env::var("CARGO_CFG_TARGET_FEATURE").map_or(false, |f| f.contains("avx512f")) {
            println!("cargo:rustc-cfg=has_avx512");
        }
    }
}
