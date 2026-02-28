#![allow(clippy::needless_range_loop)]
#![allow(clippy::range_zip_with_len)]

use cpu_time::ProcessTime;
use rand::distr::Uniform;
use rand::prelude::*;
use std::time::{Duration, SystemTime};

use anndists::dist::*;
use hnsw_rs::prelude::*;

fn main() {
    env_logger::Builder::from_default_env().init();
    //
    let nb_elem = 500000;
    let dim = 25;
    // generate nb_elem colmuns vectors of dimension dim
    let mut rng = rand::rng();
    let unif = rand::distr::StandardUniform;
    let mut data = Vec::with_capacity(nb_elem);
    for _ in 0..nb_elem {
        let column = (0..dim).map(|_| rng.sample(unif)).collect::<Vec<f32>>();
        data.push(column);
    }
    // give an id to each data
    let data_with_id = data.iter().zip(0..data.len()).collect::<Vec<_>>();

    let ef_c = 200;
    let max_nb_connection = 15;
    let nb_layer = 16.min((nb_elem as f32).ln().trunc() as usize);
    let hns = Hnsw::<f32, DistL2>::new(max_nb_connection, nb_elem, nb_layer, ef_c, DistL2 {});
    let mut start = ProcessTime::now();
    let mut begin_t = SystemTime::now();
    hns.parallel_insert(&data_with_id);
    let mut cpu_time: Duration = start.elapsed();
    println!(" hnsw data insertion  cpu time {:?}", cpu_time);
    println!(
        " hnsw data insertion parallel,   system time {:?} \n",
        begin_t.elapsed().unwrap()
    );
    hns.dump_layer_info();
    println!(
        " parallel hnsw data nb point inserted {:?}",
        hns.get_nb_point()
    );
    //
    // serial insertion
    //
    let hns = Hnsw::<f32, DistL2>::new(max_nb_connection, nb_elem, nb_layer, ef_c, DistL2 {});
    start = ProcessTime::now();
    begin_t = SystemTime::now();
    for _i in 0..data_with_id.len() {
        hns.insert((data_with_id[_i].0.as_slice(), data_with_id[_i].1))
    }
    cpu_time = start.elapsed();
    println!("\n\n serial hnsw data insertion {:?}", cpu_time);
    println!(
        " hnsw data insertion serial,  system time {:?}",
        begin_t.elapsed().unwrap()
    );
    hns.dump_layer_info();
    println!(
        " serial hnsw data nb point inserted {:?}",
        hns.get_nb_point()
    );

    let ef_search = max_nb_connection * 2;
    let knbn = 10;
    //
    for _iter in 0..100 {
        let mut r_vec = Vec::<f32>::with_capacity(dim);
        let mut rng = rand::rng();
        let unif = Uniform::<f32>::new(0., 1.).unwrap();
        for _ in 0..dim {
            r_vec.push(rng.sample(unif));
        }
        //
        let _neighbours = hns.search(&r_vec, knbn, ef_search);
    }
}
