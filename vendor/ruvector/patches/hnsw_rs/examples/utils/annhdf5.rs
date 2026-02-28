//! This file provides hdf5 utilities to load ann-benchmarks hdf5 data files
//! As the libray does not depend on hdf5 nor on ndarray, it is nearly the same for both
//! ann benchmarks.  

use ndarray::Array2;

use ::hdf5::*;
use log::debug;

// datasets
//   . distances (nbojects, dim)   f32 matrix    for tests objects
//   . neighbors (nbobjects, nbnearest) int32 matrix giving the num of nearest neighbors in train data
//   . test      (nbobjects, dim)   f32 matrix  test data
//   . train     (nbobjects, dim)   f32 matrix  train data

/// a structure to load  hdf5 data file benchmarks from https://github.com/erikbern/ann-benchmarks
pub struct AnnBenchmarkData {
    pub fname: String,
    /// distances from each test object to its nearest neighbours.
    pub test_distances: Array2<f32>,
    // for each test data , id of its nearest neighbours
    #[allow(unused)]
    pub test_neighbours: Array2<i32>,
    /// list of vectors for which we will search ann.
    pub test_data: Vec<Vec<f32>>,
    /// list of data vectors and id
    pub train_data: Vec<(Vec<f32>, usize)>,
    /// searched results. first neighbours for each test data.
    #[allow(unused)]
    pub searched_neighbours: Vec<Vec<i32>>,
    /// distances of neighbours obtained of each test
    #[allow(unused)]
    pub searched_distances: Vec<Vec<f32>>,
}

impl AnnBenchmarkData {
    pub fn new(fname: String) -> Result<AnnBenchmarkData> {
        let res = hdf5::File::open(fname.clone());
        if res.is_err() {
            println!("you must download file {:?}", fname);
            panic!(
                "download benchmark file some where and modify examples source file accordingly"
            );
        }
        let file = res.ok().unwrap();
        //
        // get test distances
        //
        let res_distances = file.dataset("distances");
        if res_distances.is_err() {
            //   let reader = hdf5::Reader::<f32>::new(&test_distance);
            panic!("error getting distances dataset");
        }
        let distances = res_distances.unwrap();
        let shape = distances.shape();
        assert_eq!(shape.len(), 2);
        let dataf32 = distances.dtype().unwrap().is::<f32>();
        if !dataf32 {
            // error
            panic!("error getting type distances dataset");
        }
        // read really data
        let res = distances.read_2d::<f32>();
        if res.is_err() {
            // some error
            panic!("error reading distances dataset");
        }
        let test_distances = res.unwrap();
        // a check for row order
        debug!(
            "First 2 distances for first test {:?} {:?}  ",
            test_distances.get((0, 0)).unwrap(),
            test_distances.get((0, 1)).unwrap()
        );
        //
        // read neighbours
        //
        let res_neighbours = file.dataset("neighbors");
        if res_neighbours.is_err() {
            //   let reader = hdf5::Reader::<f32>::new(&test_distance);
            panic!("error getting neighbours");
        }
        let neighbours = res_neighbours.unwrap();
        let shape = neighbours.shape();
        assert_eq!(shape.len(), 2);
        println!("neighbours shape : {:?}", shape);
        let datai32 = neighbours.dtype().unwrap().is::<i32>();
        if !datai32 {
            // error
            panic!("error getting type  neighbours");
        }
        // read really data
        let res = neighbours.read_2d::<i32>();
        if res.is_err() {
            // some error
            panic!("error reading neighbours dataset");
        }
        let test_neighbours = res.unwrap();
        debug!(
            "First 2 neighbours  for first test {:?} {:?}  ",
            test_neighbours.get((0, 0)).unwrap(),
            test_neighbours.get((0, 1)).unwrap()
        );
        println!("\n 10 first neighbours for first vector : ");
        for i in 0..10 {
            print!(" {:?} ", test_neighbours.get((0, i)).unwrap());
        }
        println!("\n 10 first neighbours for second vector : ");
        for i in 0..10 {
            print!(" {:?} ", test_neighbours.get((1, i)).unwrap());
        }
        //
        // read test data
        // ===============
        //
        let res_testdata = file.dataset("test");
        if res_testdata.is_err() {
            panic!("error getting test de notataset");
        }
        let test_data = res_testdata.unwrap();
        let shape = test_data.shape(); // nota shape returns a slice, dim returns a t-uple
        assert_eq!(shape.len(), 2);
        let dataf32 = test_data.dtype().unwrap().is::<f32>();
        if !dataf32 {
            panic!("error getting type de notistances dataset");
        }
        // read really datae not
        let res = test_data.read_2d::<f32>();
        if res.is_err() {
            // some error
            panic!("error reading distances dataset");
        }
        let test_data_2d = res.unwrap();
        let mut test_data = Vec::<Vec<f32>>::with_capacity(shape[1]);
        let (nbrow, nbcolumn) = test_data_2d.dim();
        println!(" test data, nb element {:?},  dim : {:?}", nbrow, nbcolumn);
        for i in 0..nbrow {
            let mut vec = Vec::with_capacity(nbcolumn);
            for j in 0..nbcolumn {
                vec.push(*test_data_2d.get((i, j)).unwrap());
            }
            test_data.push(vec);
        }
        //
        //  loaf train data
        //
        let res_traindata = file.dataset("train");
        if res_traindata.is_err() {
            panic!("error getting distances dataset");
        }
        let train_data = res_traindata.unwrap();
        let train_shape = train_data.shape();
        assert_eq!(shape.len(), 2);
        if test_data_2d.dim().1 != train_shape[1] {
            println!("test and train have not the same dimension");
            panic!();
        }
        println!(
            "\n train data shape : {:?}, nbvector {:?} ",
            train_shape, train_shape[0]
        );
        let dataf32 = train_data.dtype().unwrap().is::<f32>();
        if !dataf32 {
            // error
            panic!("error getting type distances dataset");
        }
        // read really data
        let res = train_data.read_2d::<f32>();
        if res.is_err() {
            // some error
            panic!("error reading distances dataset");
        }
        let train_data_2d = res.unwrap();
        let mut train_data = Vec::<(Vec<f32>, usize)>::with_capacity(shape[1]);
        let (nbrow, nbcolumn) = train_data_2d.dim();
        for i in 0..nbrow {
            let mut vec = Vec::with_capacity(nbcolumn);
            for j in 0..nbcolumn {
                vec.push(*train_data_2d.get((i, j)).unwrap());
            }
            train_data.push((vec, i));
        }
        //
        // now allocate array's for result
        //
        println!(
            " allocating vector for search neighbours answer : {:?}",
            test_data.len()
        );
        let searched_neighbours = Vec::<Vec<i32>>::with_capacity(test_data.len());
        let searched_distances = Vec::<Vec<f32>>::with_capacity(test_data.len());
        // searched_distances
        Ok(AnnBenchmarkData {
            fname: fname.clone(),
            test_distances,
            test_neighbours,
            test_data,
            train_data,
            searched_neighbours,
            searched_distances,
        })
    } // end new

    /// do l2 normalisation of test and train vector to use DistDot metrinc instead DistCosine to spare cpu
    #[allow(unused)]
    pub fn do_l2_normalization(&mut self) {
        for i in 0..self.test_data.len() {
            anndists::dist::l2_normalize(&mut self.test_data[i]);
        }
        for i in 0..self.train_data.len() {
            anndists::dist::l2_normalize(&mut self.train_data[i].0);
        }
    } // end of do_l2_normalization
} // end of impl block

#[cfg(test)]

mod tests {

    use super::*;

    #[test]

    fn test_load_hdf5() {
        env_logger::Builder::from_default_env().init();
        //
        let fname = String::from("/home.2/Data/ANN/glove-25-angular.hdf5");
        println!("\n\n test_load_hdf5 {:?}", fname);
        // now recall that data are stored in row order.
        let _anndata = AnnBenchmarkData::new(fname).unwrap();
        //
    } // end of test_load_hdf5
} // end of module test
