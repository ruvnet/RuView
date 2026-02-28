#![allow(clippy::range_zip_with_len)]

//! some testing utilities.  
//! run with to get output statistics : cargo test --release -- --nocapture --test test_parallel.  
//! serial test corresponds to random-10nn-euclidean(k=10)
//! parallel test corresponds to random data in 25 dimensions k = 10, dist Cosine

use rand::distr::Uniform;
use rand::prelude::*;

use skiplist::OrderedSkipList;

use anndists::dist;
use hnsw_rs::prelude::*;
use serde::{de::DeserializeOwned, Serialize};

pub fn gen_random_vector_f32(nbrow: usize) -> Vec<f32> {
    let mut rng = rand::rng();
    let unif = Uniform::<f32>::new(0., 1.).unwrap();
    (0..nbrow).map(|_| rng.sample(unif)).collect::<Vec<f32>>()
}

/// return nbcolumn vectors of dimension nbrow
pub fn gen_random_matrix_f32(nbrow: usize, nbcolumn: usize) -> Vec<Vec<f32>> {
    let mut rng = rand::rng();
    let unif = Uniform::<f32>::new(0., 1.).unwrap();
    let mut data = Vec::with_capacity(nbcolumn);
    for _ in 0..nbcolumn {
        let column = (0..nbrow).map(|_| rng.sample(unif)).collect::<Vec<f32>>();
        data.push(column);
    }
    data
}

fn brute_force_neighbours<T: Serialize + DeserializeOwned + Copy + Send + Sync>(
    nb_neighbours: usize,
    refdata: &PointIndexation<T>,
    distance: PointDistance<T>,
    data: &[T],
) -> OrderedSkipList<PointIdWithOrder> {
    let mut neighbours = OrderedSkipList::<PointIdWithOrder>::with_capacity(refdata.get_nb_point());

    let mut ptiter = refdata.into_iter();
    let mut more = true;
    while more {
        if let Some(point) = ptiter.next() {
            let dist_p = distance.eval(data, point.get_v());
            let ordered_point = PointIdWithOrder::new(point.get_point_id(), dist_p);
            //            log::debug!(" brute force inserting {:?}", ordered_point);
            if neighbours.len() < nb_neighbours {
                neighbours.insert(ordered_point);
            } else {
                neighbours.insert(ordered_point);
                neighbours.pop_back();
            }
        } else {
            more = false;
        }
    } // end while
    neighbours
} // end of brute_force_2

//================================================================================================

mod tests {
    use cpu_time::ProcessTime;
    use std::time::Duration;

    use super::*;
    use dist::l2_normalize;

    #[test]
    fn test_serial() {
        //
        //
        let nb_elem = 1000;
        let dim = 10;
        let knbn = 10;
        let ef = 20;
        let parallel = true;
        //
        println!("\n\n test_serial nb_elem {:?}", nb_elem);
        //
        let data = gen_random_matrix_f32(dim, nb_elem);
        let data_with_id = data.iter().zip(0..data.len()).collect::<Vec<_>>();

        let ef_c = 400;
        let max_nb_connection = 32;
        let nb_layer = 16.min((nb_elem as f32).ln().trunc() as usize);
        let mut hns = Hnsw::<f32, dist::DistL1>::new(
            max_nb_connection,
            nb_elem,
            nb_layer,
            ef_c,
            dist::DistL1 {},
        );
        hns.set_extend_candidates(true);
        hns.set_keeping_pruned(true);
        let mut start = ProcessTime::now();
        if parallel {
            println!("parallel insertion");
            hns.parallel_insert(&data_with_id);
        } else {
            println!("serial insertion");
            for (i, d) in data.iter().enumerate() {
                hns.insert((d, i));
            }
        }
        let mut cpu_time: Duration = start.elapsed();
        println!(" hnsw serial data insertion {:?}", cpu_time);
        hns.dump_layer_info();
        println!(" hnsw data nb point inserted {:?}", hns.get_nb_point());
        //

        let nbtest = 300;
        let mut recalls = Vec::<usize>::with_capacity(nbtest);
        let mut nb_returned = Vec::<usize>::with_capacity(nb_elem);
        let mut search_times = Vec::<f32>::with_capacity(nbtest);
        for _itest in 0..nbtest {
            //
            let mut r_vec = Vec::<f32>::with_capacity(dim);
            let mut rng = rand::rng();
            let unif = Uniform::<f32>::new(0., 1.).unwrap();
            for _ in 0..dim {
                r_vec.push(rng.sample(unif));
            }
            start = ProcessTime::now();
            let brute_neighbours = brute_force_neighbours(
                knbn,
                hns.get_point_indexation(),
                Box::new(dist::DistL1 {}),
                &r_vec,
            );
            cpu_time = start.elapsed();
            if nbtest <= 100 {
                println!("\n\n  **************** test {:?}", _itest);
                println!("\n brute force neighbours :");
                println!("======================");
                println!(" brute force computing {:?} \n ", cpu_time);
                for i in 0..brute_neighbours.len() {
                    let p = brute_neighbours[i].point_id;
                    println!(" {:?} {:?} ", p, brute_neighbours[i].dist_to_ref);
                }
            }
            //
            hns.set_searching_mode(true);
            start = ProcessTime::now();
            let knn_neighbours = hns.search(&r_vec, knbn, ef);
            cpu_time = start.elapsed();
            search_times.push(cpu_time.as_micros() as f32);
            if nbtest <= 100 {
                println!("\n\n hnsw searching  {:?} \n", cpu_time);
                println!("\n knn neighbours");
                println!("======================");
                for n in &knn_neighbours {
                    println!("  {:?} {:?}  {:?} ", n.d_id, n.p_id, n.distance);
                }
            }
            // compute recall
            let knn_neighbours_dist: Vec<f32> = knn_neighbours.iter().map(|p| p.distance).collect();
            let max_dist = brute_neighbours[knbn - 1].dist_to_ref;
            let recall = knn_neighbours_dist
                .iter()
                .filter(|d| *d <= &max_dist)
                .count();
            if nbtest <= 100 {
                println!("recall   {:?}", (recall as f32) / (knbn as f32));
            }
            recalls.push(recall);
            nb_returned.push(knn_neighbours.len());
        } // end on nbtest
          //
          // compute recall
          //

        let mean_recall = (recalls.iter().sum::<usize>() as f32) / ((knbn * recalls.len()) as f32);
        let mean_search_time = (search_times.iter().sum::<f32>()) / (search_times.len() as f32);
        println!(
            "\n mean fraction (of knbn) returned by search {:?} ",
            (nb_returned.iter().sum::<usize>() as f32) / ((nb_returned.len() * knbn) as f32)
        );
        println!(
            "\n nb element {:?} nb search : {:?} recall rate  is {:?} search time inverse {:?} ",
            nb_elem,
            nbtest,
            mean_recall,
            1.0e+6_f32 / mean_search_time
        );
    } // end test1

    #[test]
    fn test_parallel() {
        //
        let nb_elem = 1000;
        let dim = 25;
        let knbn = 10;
        let ef_c = 800;
        let max_nb_connection = 48;
        let ef = 20;
        //
        //
        let mut data = gen_random_matrix_f32(dim, nb_elem);
        for v in &mut data {
            l2_normalize(v);
        }
        let data_with_id = data.iter().zip(0..data.len()).collect::<Vec<_>>();
        let nb_layer = 16.min((nb_elem as f32).ln().trunc() as usize);
        let mut hns = Hnsw::<f32, dist::DistDot>::new(
            max_nb_connection,
            nb_elem,
            nb_layer,
            ef_c,
            dist::DistDot {},
        );
        // !
        //   hns.set_extend_candidates(true);
        let mut start = ProcessTime::now();
        let now = std::time::SystemTime::now();
        // parallel insertion
        hns.parallel_insert(&data_with_id);
        let mut cpu_time: Duration = start.elapsed();
        println!(
            "\n hnsw data parallel insertion cpu time {:?} , system time {:?}",
            cpu_time,
            now.elapsed()
        );
        // one serial more to check
        let mut v = gen_random_vector_f32(dim);
        l2_normalize(&mut v);
        hns.insert((&v, hns.get_nb_point() + 1));
        //
        hns.dump_layer_info();
        println!(" hnsw data nb point inserted {:?}", hns.get_nb_point());
        //
        println!("\n hnsw testing requests ...");
        let nbtest = 100;
        let mut recalls = Vec::<usize>::with_capacity(nbtest);
        let mut recalls_id = Vec::<usize>::with_capacity(nbtest);

        let mut search_times = Vec::<f32>::with_capacity(nbtest);
        for _itest in 0..nbtest {
            let mut r_vec = Vec::<f32>::with_capacity(dim);
            let mut rng = rand::rng();
            let unif = Uniform::<f32>::new(0., 1.).unwrap();
            for _ in 0..dim {
                r_vec.push(rng.sample(unif));
            }
            l2_normalize(&mut r_vec);

            start = ProcessTime::now();
            let brute_neighbours = brute_force_neighbours(
                knbn,
                hns.get_point_indexation(),
                Box::new(dist::DistDot),
                &r_vec,
            );
            cpu_time = start.elapsed();
            if nbtest <= 100 {
                println!("\n\n test_par nb_elem {:?}", nb_elem);
                println!("\n brute force neighbours :");
                println!("======================");
                println!(" brute force computing {:?} \n", cpu_time);
                for i in 0..brute_neighbours.len() {
                    println!(
                        " {:?}  {:?} ",
                        brute_neighbours[i].point_id, brute_neighbours[i].dist_to_ref
                    );
                }
            }
            //
            let knbn = 10;
            hns.set_searching_mode(true);
            start = ProcessTime::now();
            let knn_neighbours = hns.search(&r_vec, knbn, ef);
            cpu_time = start.elapsed();
            search_times.push(cpu_time.as_micros() as f32);
            if nbtest <= 100 {
                println!("\n knn neighbours");
                println!("======================");
                println!(" hnsw searching  {:?} \n", cpu_time);
                for n in &knn_neighbours {
                    println!("  {:?} \t {:?}  \t {:?}", n.d_id, n.p_id, n.distance);
                }
            }
            // compute recall with balls
            let knn_neighbours_dist: Vec<f32> = knn_neighbours.iter().map(|p| p.distance).collect();
            let max_dist = brute_neighbours[knbn - 1].dist_to_ref;
            let recall = knn_neighbours_dist
                .iter()
                .filter(|d| *d <= &max_dist)
                .count();
            if nbtest <= 100 {
                println!("recall   {:?}", (recall as f32) / (knbn as f32));
            }
            recalls.push(recall);
            // compute recall with id
            let mut recall_id = 0;
            let mut knn_neighbours_id: Vec<PointId> =
                knn_neighbours.iter().map(|p| p.p_id).collect();
            knn_neighbours_id.sort_unstable();
            let snbn = knbn.min(brute_neighbours.len());
            for j in 0..snbn {
                let to_search = brute_neighbours[j].point_id;
                if knn_neighbours_id.binary_search(&to_search).is_ok() {
                    recall_id += 1;
                }
            }
            recalls_id.push(recall_id);
        } // end on nbtest
          //
          // compute recall
          //

        let mean_recall = (recalls.iter().sum::<usize>() as f32) / ((knbn * recalls.len()) as f32);
        let mean_search_time = (search_times.iter().sum::<f32>()) / (search_times.len() as f32);
        println!(
            "\n nb search {:?} recall rate  is {:?} search time inverse {:?} ",
            nbtest,
            mean_recall,
            1.0e+6_f32 / mean_search_time
        );
        let mean_recall_id =
            (recalls.iter().sum::<usize>() as f32) / ((knbn * recalls.len()) as f32);
        println!("mean recall rate with point ids {:?}", mean_recall_id);
        //
        //  assert!(1==0);
    } // end test_par
}
