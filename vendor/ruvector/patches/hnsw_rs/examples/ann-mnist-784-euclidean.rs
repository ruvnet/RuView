#![allow(clippy::needless_range_loop)]

use cpu_time::ProcessTime;
use std::time::{Duration, SystemTime};

// search in serial mode i7-core @2.7Ghz for 10 fist neighbours
//  max_nb_conn   ef_cons    ef_search   scale_factor    extend  keep pruned  recall        req/s      last ratio
//
//     12           400         12           1              0          0        0.917        6486       1.005
//     24           400         24           1              1          0        0.9779       3456       1.001

// parallel mode 4 i7-core @2.7Ghz
//  max_nb_conn   ef_cons    ef_search   scale_factor    extend  keep pruned  recall        req/s      last ratio
//     24           400         24           1              0          0        0.977        12566       1.001
//     24           400         12           1              0          0        0.947        18425       1.003

// 8 hyperthreaded i7-core @ 2.3 Ghz
//     24           400         24           1              0          0        0.977        22197        1.001

// 24 core Core(TM) i9-13900HX simdeez
//     24           400         24           1              0          0        0.977        62000        1.001

// 24 core Core(TM) i9-13900HX simdeez with modify_level_scale at 0.5
//     24           400         24           0.5              0          0      0.990        58722        1.000

use anndists::dist::*;
use hnsw_rs::prelude::*;
use log::info;

mod utils;
use utils::*;

pub fn main() {
    let mut parallel = true;
    //
    let fname = String::from("/home/jpboth/Data/ANN/fashion-mnist-784-euclidean.hdf5");
    println!("\n\n test_load_hdf5 {:?}", fname);
    // now recall that data are stored in row order.
    let anndata = annhdf5::AnnBenchmarkData::new(fname).unwrap();
    let knbn_max = anndata.test_distances.dim().1;
    let nb_elem = anndata.train_data.len();
    info!(
        "Train size : {}, test size : {}",
        nb_elem,
        anndata.test_data.len()
    );
    info!("Nb neighbours answers for test data : {}", knbn_max);
    //
    let max_nb_connection = 24;
    let nb_layer = 16.min((nb_elem as f32).ln().trunc() as usize);
    let ef_c = 400;
    println!(
        " number of elements to insert {:?} , setting max nb layer to {:?} ef_construction {:?}",
        nb_elem, nb_layer, ef_c
    );
    println!(
        " ====================================================================================="
    );
    let nb_search = anndata.test_data.len();
    println!(" number of search {:?}", nb_search);

    let mut hnsw = Hnsw::<f32, DistL2>::new(max_nb_connection, nb_elem, nb_layer, ef_c, DistL2 {});
    hnsw.set_extend_candidates(false);
    //
    hnsw.modify_level_scale(0.25);
    // parallel insertion
    let mut start = ProcessTime::now();
    let mut now = SystemTime::now();
    let data_for_par_insertion = anndata
        .train_data
        .iter()
        .map(|x| (x.0.as_slice(), x.1))
        .collect();
    if parallel {
        println!(" \n parallel insertion");
        hnsw.parallel_insert_slice(&data_for_par_insertion);
    } else {
        println!(" \n serial insertion");
        for d in data_for_par_insertion {
            hnsw.insert_slice(d);
        }
    }
    let mut cpu_time: Duration = start.elapsed();
    //
    println!(
        "\n hnsw data insertion cpu time  {:?}  system time {:?} ",
        cpu_time,
        now.elapsed()
    );
    hnsw.dump_layer_info();
    println!(" hnsw data nb point inserted {:?}", hnsw.get_nb_point());
    //
    //  Now the bench with 10 neighbours
    //
    let mut recalls = Vec::<usize>::with_capacity(nb_elem);
    let mut nb_returned = Vec::<usize>::with_capacity(nb_elem);
    let mut last_distances_ratio = Vec::<f32>::with_capacity(nb_elem);
    let mut knn_neighbours_for_tests = Vec::<Vec<Neighbour>>::with_capacity(nb_elem);
    hnsw.set_searching_mode(true);
    let knbn = 10;
    let ef_c = max_nb_connection;
    println!("\n searching with ef : {:?}", ef_c);
    start = ProcessTime::now();
    now = SystemTime::now();
    // search
    parallel = true;
    if parallel {
        println!(" \n parallel search");
        knn_neighbours_for_tests = hnsw.parallel_search(&anndata.test_data, knbn, ef_c);
    } else {
        println!(" \n serial search");
        for i in 0..anndata.test_data.len() {
            let knn_neighbours: Vec<Neighbour> = hnsw.search(&anndata.test_data[i], knbn, ef_c);
            knn_neighbours_for_tests.push(knn_neighbours);
        }
    }
    cpu_time = start.elapsed();
    let search_sys_time = now.elapsed().unwrap().as_micros() as f32;
    let search_cpu_time = cpu_time.as_micros() as f32;
    println!(
        "total cpu time for search requests {:?} , system time {:?} ",
        search_cpu_time, search_sys_time
    );
    // now compute recall rate
    for i in 0..anndata.test_data.len() {
        let true_distances = anndata.test_distances.row(i);
        let max_dist = true_distances[knbn - 1];
        let mut _knn_neighbours_id: Vec<usize> =
            knn_neighbours_for_tests[i].iter().map(|p| p.d_id).collect();
        let knn_neighbours_dist: Vec<f32> = knn_neighbours_for_tests[i]
            .iter()
            .map(|p| p.distance)
            .collect();
        nb_returned.push(knn_neighbours_dist.len());
        // count how many distances of knn_neighbours_dist are less than
        let recall = knn_neighbours_dist
            .iter()
            .filter(|x| *x <= &max_dist)
            .count();
        recalls.push(recall);
        let mut ratio = 0.;
        if !knn_neighbours_dist.is_empty() {
            ratio = knn_neighbours_dist[knn_neighbours_dist.len() - 1] / max_dist;
        }
        last_distances_ratio.push(ratio);
    }
    let mean_recall = (recalls.iter().sum::<usize>() as f32) / ((knbn * recalls.len()) as f32);
    println!(
        "\n mean fraction nb returned by search {:?} ",
        (nb_returned.iter().sum::<usize>() as f32) / ((nb_returned.len() * knbn) as f32)
    );
    println!(
        "\n last distances ratio {:?} ",
        last_distances_ratio.iter().sum::<f32>() / last_distances_ratio.len() as f32
    );
    println!(
        "\n recall rate for {:?} is {:?} , nb req /s {:?}",
        anndata.fname,
        mean_recall,
        (nb_search as f32) * 1.0e+6_f32 / search_sys_time
    );
}
