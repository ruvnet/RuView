// Records the target triple so the reproducibility probe can report it.
fn main() {
    println!(
        "cargo:rustc-env=RUVIEW_FANN_TARGET={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown".into())
    );
}
