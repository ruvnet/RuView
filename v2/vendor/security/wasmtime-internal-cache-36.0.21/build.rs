use std::process::Command;
use std::str;

fn main() {
    let git_rev = match Command::new("git").args(["rev-parse", "HEAD"]).output() {
        Ok(output) => str::from_utf8(&output.stdout).unwrap().trim().to_string(),
        Err(_) => env!("CARGO_PKG_VERSION").to_string(),
    };
    println!("cargo:rustc-env=GIT_REV={git_rev}");
}
