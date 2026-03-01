//! This module provides conversion of a Point structure to a FlatPoint containing just the Id of a point
//! and those of its neighbours.
//! The whole Hnsw structure is then flattened into a Hashtable associating the data ID of a point to
//! its corresponding FlatPoint.   
//! It can be used, for example, when reloading only the graph part of the data to have knowledge
//! of relative proximity of points as described just by their DataId
//!

use hashbrown::HashMap;
use std::cmp::Ordering;

use crate::hnsw;
use anndists::dist::distances::Distance;
use hnsw::*;
use log::error;

// an ordering of Neighbour of a Point

impl PartialEq for Neighbour {
    fn eq(&self, other: &Neighbour) -> bool {
        self.distance == other.distance
    } // end eq
}

impl Eq for Neighbour {}

// order points by distance to self.
#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for Neighbour {
    fn partial_cmp(&self, other: &Neighbour) -> Option<Ordering> {
        self.distance.partial_cmp(&other.distance)
    } // end cmp
} // end impl PartialOrd

impl Ord for Neighbour {
    fn cmp(&self, other: &Neighbour) -> Ordering {
        if !self.distance.is_nan() && !other.distance.is_nan() {
            self.distance.partial_cmp(&other.distance).unwrap()
        } else {
            panic!("got a NaN in a distance");
        }
    } // end cmp
}

/// a reduced version of point inserted in the Hnsw structure.
/// It contains original id of point as submitted to the struct Hnsw
/// an ordered (by distance) list of neighbours to the point
/// and it position in layers.
#[derive(Clone)]
pub struct FlatPoint {
    /// an id coming from client using hnsw, should identify point uniquely
    origin_id: DataId,
    /// a point id identifying point as stored in our structure
    p_id: PointId,
    /// neighbours info
    neighbours: Vec<Neighbour>,
}

impl FlatPoint {
    /// returns the neighbours orderded by distance.
    pub fn get_neighbours(&self) -> &Vec<Neighbour> {
        &self.neighbours
    }
    /// returns the origin id of the point
    pub fn get_id(&self) -> DataId {
        self.origin_id
    }
    //
    pub fn get_p_id(&self) -> PointId {
        self.p_id
    }
} // end impl block for FlatPoint

fn flatten_point<T: Clone + Send + Sync>(point: &Point<T>) -> FlatPoint {
    let neighbours = point.get_neighborhood_id();
    // now we flatten neighbours
    let mut flat_neighbours = Vec::<Neighbour>::new();
    for layer in neighbours {
        for neighbour in layer {
            flat_neighbours.push(neighbour);
        }
    }
    flat_neighbours.sort_unstable();
    FlatPoint {
        origin_id: point.get_origin_id(),
        p_id: point.get_point_id(),
        neighbours: flat_neighbours,
    }
} // end of flatten_point

/// A structure providing neighbourhood information of a point stored in the Hnsw structure given its DataId.  
/// The structure uses the [FlatPoint] structure.  
/// This structure can be obtained by FlatNeighborhood::from<&Hnsw<T,D>>
pub struct FlatNeighborhood {
    hash_t: HashMap<DataId, FlatPoint>,
}

impl FlatNeighborhood {
    /// get neighbour of a point given its id.  
    /// The neighbours are sorted in increasing distance from data_id.
    pub fn get_neighbours(&self, p_id: DataId) -> Option<Vec<Neighbour>> {
        self.hash_t
            .get(&p_id)
            .map(|point| point.get_neighbours().clone())
    }
} // end impl block for FlatNeighborhood

impl<T: Clone + Send + Sync, D: Distance<T> + Send + Sync> From<&Hnsw<'_, T, D>>
    for FlatNeighborhood
{
    /// extract from the Hnsw strucure a hashtable mapping original DataId into a FlatPoint structure gathering its neighbourhood information.  
    /// Useful after reloading from a dump with T=NoData and D = NoDist as points are then reloaded with neighbourhood information only.
    fn from(hnsw: &Hnsw<T, D>) -> Self {
        let mut hash_t = HashMap::new();
        let pt_iter = hnsw.get_point_indexation().into_iter();
        //
        for point in pt_iter {
            //    println!("point : {:?}", _point.p_id);
            let res_insert = hash_t.insert(point.get_origin_id(), flatten_point(&point));
            if let Some(old_point) = res_insert {
                error!("2 points with same origin id {:?}", old_point.origin_id);
            }
        }
        FlatNeighborhood { hash_t }
    }
} // e,d of Fom implementation

#[cfg(test)]

mod tests {

    use super::*;
    use anndists::dist::distances::*;
    use log::debug;

    use crate::api::AnnT;
    use crate::hnswio::*;

    use rand::distr::{Distribution, Uniform};

    fn log_init_test() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn test_dump_reload_graph_flatten() {
        println!("\n\n test_dump_reload_graph_flatten");
        log_init_test();
        // generate a random test
        let mut rng = rand::rng();
        let unif = Uniform::<f32>::new(0., 1.).unwrap();
        // 1000 vectors of size 10 f32
        let nbcolumn = 1000;
        let nbrow = 10;
        let mut xsi;
        let mut data = Vec::with_capacity(nbcolumn);
        for j in 0..nbcolumn {
            data.push(Vec::with_capacity(nbrow));
            for _ in 0..nbrow {
                xsi = unif.sample(&mut rng);
                data[j].push(xsi);
            }
        }
        // define hnsw
        let ef_construct = 25;
        let nb_connection = 10;
        let hnsw = Hnsw::<f32, DistL1>::new(nb_connection, nbcolumn, 16, ef_construct, DistL1 {});
        for (i, d) in data.iter().enumerate() {
            hnsw.insert((d, i));
        }
        // some loggin info
        hnsw.dump_layer_info();
        // get flat neighbours of point 3
        let neighborhood_before_dump = FlatNeighborhood::from(&hnsw);
        let nbg_2_before = neighborhood_before_dump.get_neighbours(2).unwrap();
        println!("voisins du point 2 {:?}", nbg_2_before);
        // dump in a file. Must take care of name as tests runs in // !!!
        let fname = "dumpreloadtestflat";
        let directory = tempfile::tempdir().unwrap();
        let _res = hnsw.file_dump(directory.path(), fname);
        // This will dump in 2 files named dumpreloadtest.hnsw.graph and dumpreloadtest.hnsw.data
        //
        // reload
        debug!("HNSW reload");
        // we will need a procedural macro to get from distance name to its instantiation.
        // from now on we test with DistL1
        let mut reloader = HnswIo::new(directory.path(), fname);
        let hnsw_loaded: Hnsw<NoData, NoDist> = reloader.load_hnsw().unwrap();
        let neighborhood_after_dump = FlatNeighborhood::from(&hnsw_loaded);
        let nbg_2_after = neighborhood_after_dump.get_neighbours(2).unwrap();
        println!("Neighbors of point 2 {:?}", nbg_2_after);
        // test equality of neighborhood
        assert_eq!(nbg_2_after.len(), nbg_2_before.len());
        for i in 0..nbg_2_before.len() {
            assert_eq!(nbg_2_before[i].p_id, nbg_2_after[i].p_id);
            assert_eq!(nbg_2_before[i].distance, nbg_2_after[i].distance);
        }
        check_graph_equality(&hnsw_loaded, &hnsw);
    } // end of test_dump_reload
} // end module test
