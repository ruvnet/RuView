fn main() {
    let windows_gnu = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu");

    // PE/MinGW auto-exports all global symbols when a DLL has no explicit
    // exports. This Rust-only desktop entry point can then exceed PE's export
    // ordinal limit in debug builds. Disable automatic exports for this cdylib
    // only; explicit exports (if added later) and the Rust rlib are unaffected.
    // Read the target, not the build-script host, for cross-compilation.
    if windows_gnu {
        println!("cargo:rustc-link-arg-cdylib=-Wl,--exclude-all-symbols");
    }
    tauri_build::build();

    if windows_gnu {
        // tauri-winres attaches the generated app resource only to binaries.
        // The library's unit-test executable also imports TaskDialogIndirect,
        // which needs the manifest's Common Controls v6 activation context.
        // src/lib.rs links this resource only for library tests; the tests
        // directive below covers integration tests. Do not use a general
        // link-arg: binaries would receive Tauri's resource twice.
        let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let resource = out_dir.join("libresource.a");
        assert!(
            resource.is_file(),
            "Tauri's Windows GNU resource is missing"
        );
        println!("cargo:rustc-link-search=native={}", out_dir.display());
        println!("cargo:rustc-link-arg-tests={}", resource.display());
    }
}
