#![cfg_attr(feature = "stdsimd", feature(portable_simd))]
//
// for logging (debug mostly, switched at compile time in cargo.toml)
use env_logger::Builder;

use lazy_static::lazy_static;

pub mod api;
pub mod datamap;
pub mod filter;
pub mod flatten;
pub mod hnsw;
pub mod hnswio;
pub mod libext;
pub mod prelude;

// we impose our version of anndists
pub use anndists;

lazy_static! {
    static ref LOG: u64 = init_log();
}

// install a logger facility
#[allow(unused)]
fn init_log() -> u64 {
    Builder::from_default_env().init();
    println!("\n ************** initializing logger *****************\n");
    1
}
