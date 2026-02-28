#![allow(clippy::needless_range_loop)]

use cpu_time::ProcessTime;
use std::time::{Duration, SystemTime};

//    glove 25 // 2.7 Ghz 4 cores 8Mb L3  k = 10
//   ============================================
//
// max_nb_conn   ef_cons    ef_search   scale_factor    extend  keep pruned  recall        req/s      last ratio
//  24            800         64            1.           1          0          0.928        4090        1.003
//  24            800         64            1.           1          1          0.927        4594        1.003
//  24            400,        48            1.           1          0          0.919        6349        1.0044
//  24            800         48            1            1          1          0.918        5785        1.005
//  24            400         32            1.           0          0          0.898        8662
//  24            400         64            1.           1          0          0.930        4711        1.0027
//  24            400         64            1.           1          1          0.921        4550        1.0039
//  24           1600         48            1            1          0          0.924        5380        1.0034

//  32            400         48            1            1          0          0.93         4706        1.0026
//  32            800         64            1            1          0          0.94         3780.       1.0015
//  32            1600        48            1            1          0          0.934        4455        1.0023
//  48            1600        48            1            1          0          0.945        3253        1.00098

//  24            400         48            1            1          0          0.92         6036.       1.0038
//  48            800         48            1            1          0          0.935        4018        1.002
//  48            800         64            1            1          0          0.942        3091        1.0014
//  48            800         64            1            1          1          0.9435       2640        1.00126

// k = 100

//  24            800         48            1            1          0          0.96         2432      1.004
//  48            800        128            1            1          0          0.979        1626      1.001

// glove 25 // 8 cores i7 2.3 Ghz  8Mb L3  knbn = 100
// ==================================================

//  48            800         48            1            1          0          0.935        13400      1.002
//  48            800        128            1            1          0          0.979         5227      1.002

// 24 core Core(TM) i9-13900HX simdeez     knbn = 10
// ==================================================
//  48            800         48            1            1          0          0.936        30748     1.002

// 24 core Core(TM) i9-13900HX simdeez     knbn = 100
// ==================================================
//  48            800        128            1            1          0          0.979        12000     1.002

// results with scale modification 0.5
//====================================

// 24 core Core(TM) i9-13900HX simdeez     knbn = 10
// ==================================================
//  24            800         48            0.5            1          0          0.931        40700     1.002
//  48            800         48            0.5            1          0          0.941        30001     1.001

// 24 core Core(TM) i9-13900HX simdeez     knbn = 100
// ==================================================
//  24            800        128            0.5            1          0          0.974        16521     1.002
//  48            800        128            0.5            1          0          0.985        11484     1.001

use anndists::dist::*;
use hnsw_rs::prelude::*;
use log::info;

mod utils;

use utils::*;

pub fn main() {
    let _ = env_logger::builder().is_test(true).try_init().unwrap();
    let parallel = true;
    //
    let fname = String::from("/home/jpboth/Data/ANN/glove-25-angular.hdf5");
    println!("\n\n test_load_hdf5 {:?}", fname);
    // now recall that data are stored in row order.
    let mut anndata = annhdf5::AnnBenchmarkData::new(fname).unwrap();
    // pre normalisation to use Dot computations instead of Cosine
    anndata.do_l2_normalization();
    // run bench
    let nb_elem = anndata.train_data.len();
    let knbn_max = anndata.test_distances.dim().1;
    info!(
        "Train size : {}, test size : {}",
        nb_elem,
        anndata.test_data.len()
    );
    info!("Nb neighbours answers for test data : {} \n\n", knbn_max);
    //
    let max_nb_connection = 24;
    let ef_c = 800;
    println!(
        " max_nb_conn : {:?}, ef_construction : {:?} ",
        max_nb_connection, ef_c
    );
    let nb_layer = 16.min((nb_elem as f32).ln().trunc() as usize);
    println!(
        " number of elements to insert {:?} , setting max nb layer to {:?} ef_construction {:?}",
        nb_elem, nb_layer, ef_c
    );
    let nb_search = anndata.test_data.len();
    println!(" number of search {:?}", nb_search);
    // Hnsw allocation
    let mut hnsw =
        Hnsw::<f32, DistDot>::new(max_nb_connection, nb_elem, nb_layer, ef_c, DistDot {});
    //
    hnsw.set_extend_candidates(true);
    hnsw.modify_level_scale(0.5);
    //
    // parallel insertion
    let start = ProcessTime::now();
    let now = SystemTime::now();
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
    let cpu_time: Duration = start.elapsed();
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
    let knbn = 10;
    let ef_search = 48;
    search(&mut hnsw, knbn, ef_search, &anndata);

    let knbn = 100;
    let ef_search = 128;
    search(&mut hnsw, knbn, ef_search, &anndata);
}

pub fn search<Dist>(
    hnsw: &mut Hnsw<f32, Dist>,
    knbn: usize,
    ef_search: usize,
    anndata: &annhdf5::AnnBenchmarkData,
) where
    Dist: Distance<f32> + Send + Sync,
{
    println!("\n\n ef_search : {:?} knbn : {:?} ", ef_search, knbn);
    let parallel = true;
    //
    let nb_elem = anndata.train_data.len();
    let nb_search = anndata.test_data.len();
    //
    let mut recalls = Vec::<usize>::with_capacity(nb_elem);
    let mut nb_returned = Vec::<usize>::with_capacity(nb_elem);
    let mut last_distances_ratio = Vec::<f32>::with_capacity(nb_elem);
    let mut knn_neighbours_for_tests = Vec::<Vec<Neighbour>>::with_capacity(nb_elem);
    hnsw.set_searching_mode(true);
    println!("searching with ef : {:?}", ef_search);
    let start = ProcessTime::now();
    let now = SystemTime::now();
    // search
    if parallel {
        println!(" \n parallel search");
        knn_neighbours_for_tests = hnsw.parallel_search(&anndata.test_data, knbn, ef_search);
    } else {
        println!(" \n serial search");
        for i in 0..anndata.test_data.len() {
            let knn_neighbours: Vec<Neighbour> =
                hnsw.search(&anndata.test_data[i], knbn, ef_search);
            knn_neighbours_for_tests.push(knn_neighbours);
        }
    }
    let cpu_time = start.elapsed();
    let search_cpu_time = cpu_time.as_micros() as f32;
    let search_sys_time = now.elapsed().unwrap().as_micros() as f32;
    println!(
        "total cpu time for search requests {:?} , system time {:?} ",
        search_cpu_time,
        now.elapsed()
    );
    // now compute recall rate
    for i in 0..anndata.test_data.len() {
        let max_dist = anndata.test_distances.row(i)[knbn - 1];
        let knn_neighbours_d: Vec<f32> = knn_neighbours_for_tests[i]
            .iter()
            .map(|p| p.distance)
            .collect();
        nb_returned.push(knn_neighbours_d.len());
        let recall = knn_neighbours_d.iter().filter(|d| *d <= &max_dist).count();
        recalls.push(recall);
        let mut ratio = 0.;
        if !knn_neighbours_d.is_empty() {
            ratio = knn_neighbours_d[knn_neighbours_d.len() - 1] / max_dist;
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
