use env_logger::Builder;

use anndists::dist::DistL1;
use hnsw_rs::hnsw::Hnsw;

// A test program to see if memory from insertions gets deallocated.
// This program sets up a process that iteratively builds a new model and lets it go out of scope.
// Since the models go out of scope, the desired behavior is that memory consumption is constant while this program is running.
fn main() {
    //
    Builder::from_default_env().init();
    //
    let mut counter: usize = 0;
    loop {
        let hnsw: Hnsw<f32, DistL1> = Hnsw::new(15, 100_000, 20, 500_000, DistL1 {});
        let s1 = [1.0, 0.0, 0.0, 0.0];
        hnsw.insert_slice((&s1, 0));
        let s2 = [0.0, 1.0, 1.0];
        hnsw.insert_slice((&s2, 1));
        let s3 = [0.0, 0.0, 1.0];
        hnsw.insert_slice((&s3, 2));
        let s4 = [1.0, 0.0, 0.0, 1.0];
        hnsw.insert_slice((&s4, 3));
        let s5 = [1.0, 1.0, 1.0];
        hnsw.insert_slice((&s5, 4));
        let s6 = [1.0, -1.0, 1.0];
        hnsw.insert_slice((&s6, 5));

        if counter % 1_000_000 == 0 {
            println!("counter : {}", counter)
        }
        counter += 1;
    }
}
