//! This module provides io dump/ reload of computed graph via the structure Hnswio.  
//! This structure stores references to data points if memory map is used.  
//!
//! A dump is constituted of 2 files.
//! One file stores just the graph (or topology) with id of points.  
//! The other file stores the ids and vector in point and can be reloaded via a mmap scheme.
//! The graph file is suffixed by "hnsw.graph" the other is suffixed by "hnsw.data"
//!
//! Examples of dump and reload of structure Hnsw is given in the tests (see test_dump_reload, reload_with_mmap)
// datafile
// MAGICDATAP : u32
// dimension : usize!!
// The for each point the triplet: (MAGICDATAP, origin_id , dimension , array of values bson encoded) ( u32, u64, ....)
//
// A point is dumped in graph file as given by its external id (type DataId i.e : a usize, possibly a hash value)
// and layer (u8) and rank_in_layer:i32.
// In the data file the point dump consist in the triplet: (MAGICDATAP, origin_id , array of values.)
//
use serde::{Serialize, de::DeserializeOwned};
use std::sync::atomic::{AtomicUsize, Ordering};
//
use std::time::SystemTime;

// io
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

// synchro
use parking_lot::RwLock;
use std::sync::Arc;

use std::collections::HashMap;

use rand::Rng;

use anyhow::*;
use std::any::type_name;

use anndists::dist::distances::*;

use self::hnsw::*;
use crate::datamap::*;
use crate::hnsw;
use log::{debug, error, info, trace};
use std::io::prelude::*;

// magic before each graph point data for each point
const MAGICPOINT: u32 = 0x000a678f;
// magic at beginning of description format v2 of dump
const MAGICDESCR_2: u32 = 0x002a677f;

// magic at beginning of description format v3 of dump
// format where we can use mmap to provide acces to data (not graph) via a memory mapping of file data ,
// useful when data vector are large and data uses more space than graph.
// differ from v2 as we do not use bincode encoding for point. We dump pure binary
// This help use mmap as we can return directly a slice.
const MAGICDESCR_3: u32 = 0x002a6771;

// magic for v4
// we dump level scale modififcation factor
const MAGICDESCR_4: u32 = 0x002a6779;

// magic at beginning of a layer dump
const MAGICLAYER: u32 = 0x000a676f;
// magic head of data file and before each data vector
pub(crate) const MAGICDATAP: u32 = 0xa67f0000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DumpMode {
    Light,
    Full,
}

/// The main interface for dumping struct Hnsw.
pub(crate) trait HnswIoT {
    fn dump(&self, mode: DumpMode, dumpinit: &mut DumpInit) -> anyhow::Result<i32>;
}

/// Describe options accessible for reload
///
///  - datamap : a bool for mmap usage.  
///    The data point can be reloaded via mmap of data file dump.  
///    This can be useful when data points consist in large vectors (as in genomic sketching)
///    as in this case data needs more space than the graph.  
///
///  - mmap_threshold : the number of itmes above which we use mmap. Default is 0, meaning always use mmap data
///    Can be useful for search speed in hnsw if we have part of data resident in memory.
#[derive(Copy, Clone)]
pub struct ReloadOptions {
    datamap: bool,
    /// number of data items above which we use mmap.
    mmap_threshold: usize,
}

impl Default for ReloadOptions {
    /// default is no mmap
    fn default() -> Self {
        ReloadOptions {
            datamap: false,
            mmap_threshold: 0,
        }
    }
}

impl ReloadOptions {
    pub fn new(datamap: bool) -> Self {
        ReloadOptions {
            datamap,
            mmap_threshold: 0,
        }
    }

    /// set mmap uasge to true
    pub fn set_mmap(&mut self, val: bool) -> Self {
        self.datamap = val;
        *self
    }

    /// set mmap threshold i.e : The maximum number of data that will be reloaded in memory by reading file dump, the other points will be mmapped.  
    /// As the upper layers are the most frequently used, these points will be loaded in memory during reading, the others will be mmaped.
    /// See test *reload_with_mmap()*
    pub fn set_mmap_threshold(&mut self, threshold: usize) -> Self {
        if threshold > 0 {
            self.datamap = true;
            self.mmap_threshold = threshold;
        }
        *self
    }

    /// return a 2-uple, (datamap, threshold)
    pub fn use_mmap(&self) -> (bool, usize) {
        (self.datamap, self.mmap_threshold)
    }
} // end of ReloadOptions

//===============================================================================================

// initialize datafile and graphfile for io ops
// This structure will check existence of dumps of same name and generate a unique filename if necessary according to overwrite flag
#[allow(unused)]
pub struct DumpInit {
    // basename dump
    basename: String,
    // to dump data
    pub(crate) data_out: BufWriter<File>,
    // to dump graph
    pub(crate) graph_out: BufWriter<File>,
} // end of

impl DumpInit {
    // This structure will check existence of dumps of same name and generate a unique filename if necessary according to overwrite flag
    pub fn new(dir: &Path, basename_default: &str, overwrite: bool) -> Self {
        // if we cannot overwrite data files (in case of mmap in particular)
        // we will ensure we have a unique basename
        let basename = match overwrite {
            true => basename_default.to_string(),
            false => {
                // we check
                let mut dataname = basename_default.to_string();
                dataname.push_str(".hnsw.data");
                let mut datapath = PathBuf::from(dir);
                datapath.push(dataname);
                let exist_res = std::fs::metadata(datapath.as_os_str());
                if exist_res.is_ok() {
                    let unique_basename = loop {
                        let mut unique_basename;
                        let mut dataname: String;
                        let id: usize = rand::thread_rng().gen_range(0..10000);
                        let strid: String = id.to_string();
                        unique_basename = basename_default.to_string();
                        unique_basename.push('-');
                        unique_basename.push_str(&strid);
                        dataname = unique_basename.clone();
                        dataname.push_str(".hnsw.data");
                        let mut datapath = PathBuf::from(dir);
                        datapath.push(dataname);
                        let exist_res = std::fs::metadata(datapath.as_os_str());
                        if exist_res.is_err() {
                            break unique_basename;
                        }
                    };
                    unique_basename
                } else {
                    basename_default.to_string()
                }
            }
        };
        //
        info!("Dumping with (unique) basename : {}", basename);
        //
        let mut graphname = basename.clone();
        graphname.push_str(".hnsw.graph");
        let mut graphpath = PathBuf::from(dir);
        graphpath.push(graphname);
        let graphfileres = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&graphpath);
        if graphfileres.is_err() {
            println!(
                "HnswIo::reload_hnsw : could not open file {:?}",
                graphpath.as_os_str()
            );
            std::panic::panic_any("HnswIo::init : could not open file".to_string());
        }
        let graphfile = graphfileres.unwrap();
        //  same thing for data file
        let mut dataname = basename.clone();
        dataname.push_str(".hnsw.data");
        let mut datapath = PathBuf::from(dir);
        datapath.push(dataname);
        let datafileres = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&datapath);
        if datafileres.is_err() {
            println!(
                "HnswIo::init : could not open file {:?}",
                datapath.as_os_str()
            );
            std::panic::panic_any("HnswIo::init : could not open file".to_string());
        }
        let datafile = datafileres.unwrap();
        //
        let graph_out = BufWriter::new(graphfile);
        let data_out = BufWriter::new(datafile);
        //
        DumpInit {
            basename,
            data_out,
            graph_out,
        }
    }

    /// returns the basename used for the dump. May be it has been made unique to void overwriting a previous or mmapped dump
    pub fn get_basename(&self) -> &String {
        &self.basename
    }

    pub fn flush(&mut self) -> Result<()> {
        self.data_out.flush()?;
        self.graph_out.flush()?;
        Ok(())
    }
} // end impl for DumpInit

//====================================================
// basic block used to provide arguments to load_hnsw and load_hnsw_with_dist
struct LoadInit {
    descr: Description,
    //
    graphfile: BufReader<File>,
    //
    datafile: BufReader<File>,
} // end of LoadInit

/// a structure to provide simplified methods for reloading a previous dump.  
///  
/// The data point can be reloaded via mmap of data file dump.  
/// This can be useful when data points consist in large vectors (as in genomic sketching)
/// as in this case data needs more space than the graph.  
/// Note : **As this structure potentially contains the mmap data used in hnsw after reload it must not be dropped
/// before the reloaded hnsw.**
/// Example:
///
/// See example in  tests::reload_with_mmap
/// ```text
///     let directory = Path::new(".");
///     let mut reloader = HnswIo::new(directory, "mmapreloadtest");
///     let options = ReloadOptions::default().set_mmap(true);
///     reloader.set_options(options);
///     let hnsw_loaded : Hnsw<f32,DistL1>= reloader.load_hnsw::<f32, DistL1>().unwrap();
/// ```  
///   
/// In some cases we need a hnsw variable that can come from a reload **OR** a direct initialization.  
///   
/// Hnswio must be defined before Hnsw as drop is done in reverse order of definition, and the function [load_hnsw](Self::load_hnsw())
/// borrows Hnswio. (Hnswio stores the mmap address Hnsw can refer to if mmap is used)
/// It is also possible to preinitialize a Hnswio with the default() function which leaves all the fields with blank values and use
/// the function [set_values](Self::set_values()) after.  
/// We get something like:
///
/// ```text
///     let need_reload : bool;
///     ....................
///     let mut hnswio : Hnswio::default();
///     let hnsw : Hnsw<>;
///     if need_reload {
///         hnswio.set_values(...);
///         hnsw = hnswio.reload_hnsw(...)
///     }
///     else {
///         hnsw = Hnsw::new(...)
///     }
/// ````
#[derive(Default)]
pub struct HnswIo {
    dir: PathBuf,
    /// basename is used to build $basename.hnsw.data and $basename.hnsw.graph
    basename: String,
    /// options
    options: ReloadOptions,
    datamap: Option<DataMap>,
    /// for Hnswio to be async
    nb_point_loaded: Arc<AtomicUsize>,
    initialized: bool,
} // end of struct ReloadOptions

impl HnswIo {
    /// - directory is directory containing the dumped files,
    /// - basename is used to build $basename.hnsw.data and $basename.hnsw.graph
    ///
    ///  default is to use default ReloadOptions.
    pub fn new(directory: &Path, basename: &str) -> Self {
        HnswIo {
            dir: directory.to_path_buf(),
            basename: basename.to_string(),
            options: ReloadOptions::default(),
            datamap: None,
            nb_point_loaded: Arc::new(AtomicUsize::new(0)),
            initialized: true,
        }
    }

    /// same as preceding, avoids the call to [set_options](Self::set_options())
    pub fn new_with_options(directory: &Path, basename: &str, options: ReloadOptions) -> Self {
        HnswIo {
            dir: directory.to_path_buf(),
            basename: basename.to_string(),
            options,
            datamap: None,
            nb_point_loaded: Arc::new(AtomicUsize::new(0)),
            initialized: true,
        }
    }

    /// return basename of dump
    pub fn get_basename(&self) -> &str {
        &self.basename
    }
    /// this method enables effective initialization after default allocation.
    /// It is an error to call set_values on an already defined Hswnio by any function other than [default](Self::default())
    pub fn set_values(
        &mut self,
        directory: &Path,
        basename: String,
        options: ReloadOptions,
    ) -> Result<()> {
        if self.initialized {
            return Err(anyhow!("Hnswio already initialized"));
        };
        //
        self.dir = directory.to_path_buf();
        self.basename = basename;
        self.options = options;
        self.datamap = None;
        //
        self.initialized = true;
        //
        Ok(())
    } // end of set_values

    //
    fn init(&self) -> Result<LoadInit> {
        //
        info!("reloading from basename : {}", &self.basename);
        //
        let mut graphname = self.basename.clone();
        graphname.push_str(".hnsw.graph");
        let mut graphpath = self.dir.clone();
        graphpath.push(graphname);
        let graphfileres = OpenOptions::new().read(true).open(&graphpath);
        if graphfileres.is_err() {
            println!(
                "HnswIo::reload_hnsw : could not open file {:?}",
                graphpath.as_os_str()
            );
            error!(
                "HnswIo::reload_hnsw : could not open file {:?}",
                graphpath.as_os_str()
            );
            return Err(anyhow!(
                "HnswIo::reload_hnsw : could not open file {:?}",
                graphpath.as_os_str()
            ));
        }
        let graphfile = graphfileres.unwrap();
        //  same thing for data file
        let mut dataname = self.basename.clone();
        dataname.push_str(".hnsw.data");
        let mut datapath = self.dir.clone();
        datapath.push(dataname);
        let datafileres = OpenOptions::new().read(true).open(&datapath);
        if datafileres.is_err() {
            println!(
                "HnswIo::init : could not open file {:?}",
                datapath.as_os_str()
            );
            error!(
                "HnswIo::init : could not open file {:?}",
                datapath.as_os_str()
            );
            return Err(anyhow!(
                "HnswIo::reload_hnsw : could not open file {:?}",
                datapath.as_os_str()
            ));
        }
        let datafile = datafileres.unwrap();
        //
        let mut graph_in = BufReader::new(graphfile);
        let data_in = BufReader::new(datafile);
        // we need to call load_description first to get distance name
        let hnsw_description = load_description(&mut graph_in).unwrap();
        //
        Ok(LoadInit {
            descr: hnsw_description,
            graphfile: graph_in,
            datafile: data_in,
        })
    }

    /// to set non default options, in particular to ask for mmap of data file
    pub fn set_options(&mut self, options: ReloadOptions) {
        self.options = options;
    }

    /// reload a previously dumped hnsw structure
    pub fn load_hnsw<'b, 'a, T, D>(&'a mut self) -> Result<Hnsw<'b, T, D>>
    where
        T: 'static + Serialize + DeserializeOwned + Clone + Sized + Send + Sync + std::fmt::Debug,
        D: Distance<T> + Default + Send + Sync,
        'a: 'b,
    {
        //
        debug!("HnswIo::load_hnsw ");
        let start_t = SystemTime::now();
        //
        let init = self.init();
        if init.is_err() {
            return Err(anyhow!("could not reload HNSW structure"));
        }
        let mut init = init.unwrap();
        let data_in = &mut init.datafile;
        let graph_in = &mut init.graphfile;
        let description = init.descr;
        info!("format version : {}", description.format_version);
        //  In datafile , we must read MAGICDATAP and dimension and check
        let mut it_slice = [0u8; std::mem::size_of::<u32>()];
        data_in.read_exact(&mut it_slice)?;
        let magic = u32::from_ne_bytes(it_slice);
        assert_eq!(
            magic, MAGICDATAP,
            "magic not equal to MAGICDATAP in load_point"
        );
        //
        let mut it_slice = [0u8; std::mem::size_of::<usize>()];
        data_in.read_exact(&mut it_slice)?;
        let dimension = usize::from_ne_bytes(it_slice);
        assert_eq!(
            dimension, description.dimension,
            "data dimension incoherent {:?} {:?} ",
            dimension, description.dimension
        );
        //
        let _mode = description.dumpmode;
        let distname = description.distname.clone();
        // We must ensure that the distance stored matches the one asked for in loading hnsw
        // for that we check for short names equality stripping
        debug!("distance in description = {:?}", distname);
        let d_type_name = type_name::<D>().to_string();
        let d_type_name_split: Vec<&str> = d_type_name.rsplit_terminator("::").collect();
        for s in &d_type_name_split {
            info!(" distname in generic type argument {:?}", s);
        }
        let distname_split: Vec<&str> = distname.rsplit_terminator("::").collect();
        if (std::any::TypeId::of::<T>() != std::any::TypeId::of::<NoData>())
            && (d_type_name_split[0] != distname_split[0])
        {
            // for all types except NoData , distance asked in reload declaration and distance in dump must be equal!
            let mut errmsg = String::from("error in distances : dumped distance is : ");
            errmsg.push_str(&distname);
            errmsg.push_str(" asked distance in loading is : ");
            errmsg.push_str(&d_type_name);
            error!(" distance in type argument : {:?}", d_type_name);
            error!("error , dump is for distance = {:?}", distname);
            return Err(anyhow!(errmsg));
        }
        let t_type = description.t_name.clone();
        debug!("T type name in dump = {:?}", t_type);
        // Do we use mmap at reload
        if self.options.use_mmap().0 {
            let datamap_res = DataMap::from_hnswdump::<T>(self.dir.as_path(), &self.basename);
            if datamap_res.is_err() {
                error!("load_hnsw could not initialize mmap")
            } else {
                info!("reload using mmap");
                self.datamap = Some(datamap_res.unwrap());
            }
        }
        // reloader can use datamap
        let layer_point_indexation = self.load_point_indexation(graph_in, &description, data_in)?;
        let data_dim = layer_point_indexation.get_data_dimension();
        //
        let hnsw: Hnsw<T, D> = Hnsw {
            max_nb_connection: description.max_nb_connection as usize,
            ef_construction: description.ef,
            extend_candidates: true,
            keep_pruned: false,
            max_layer: description.nb_layer as usize,
            layer_indexed_points: layer_point_indexation,
            data_dimension: data_dim,
            dist_f: D::default(),
            searching: false,
            datamap_opt: true, // set datamap_opt to true
        };
        //
        debug!("load_hnsw completed");
        let elapsed_t = start_t.elapsed().unwrap().as_secs() as f32;
        info!("reload_hnsw : elapsed system time(s) {}", elapsed_t);
        Ok(hnsw)
    } // end of load_hnsw

    /// reload a previously dumped hnsw structure
    /// This function makes reload of a Hnsw dump with a given Dist.  
    /// It is dedicated to distance of type DistPtr (see crate [anndist](https://crates.io/crates/anndists)) that cannot implement Default.  
    /// **It is the user responsability to reload with the same function as used in the dump**
    ///
    pub fn load_hnsw_with_dist<'b, 'a, T, D>(&'a self, f: D) -> anyhow::Result<Hnsw<'b, T, D>>
    where
        T: 'static + Serialize + DeserializeOwned + Clone + Sized + Send + Sync + std::fmt::Debug,
        D: Distance<T> + Send + Sync,
        'a: 'b,
    {
        //
        debug!("HnswIo::load_hnsw_with_dist");
        //
        let init = self.init();
        if init.is_err() {
            return Err(anyhow!("Could not reload hnsw structure"));
        }
        let mut init = init.unwrap();
        //
        let data_in = &mut init.datafile;
        let graph_in = &mut init.graphfile;
        let description = init.descr;
        //  In datafile , we must read MAGICDATAP and dimension and check
        let mut it_slice = [0u8; std::mem::size_of::<u32>()];
        data_in.read_exact(&mut it_slice)?;
        let magic = u32::from_ne_bytes(it_slice);
        assert_eq!(
            magic, MAGICDATAP,
            "magic not equal to MAGICDATAP in load_point"
        );
        //
        let mut it_slice = [0u8; std::mem::size_of::<usize>()];
        data_in.read_exact(&mut it_slice)?;
        let dimension = usize::from_ne_bytes(it_slice);
        assert_eq!(
            dimension, description.dimension,
            "data dimension incoherent {:?} {:?} ",
            dimension, description.dimension
        );
        //
        let _mode = description.dumpmode;
        let distname = description.distname.clone();
        // We must ensure that the distance stored matches the one asked for in loading hnsw
        // for that we check for short names equality stripping
        info!("distance in description = {:?}", distname);
        let d_type_name = type_name::<D>().to_string();
        let v: Vec<&str> = d_type_name.rsplit_terminator("::").collect();
        for s in v {
            info!(" distname in generic type argument {:?}", s);
        }
        if (std::any::TypeId::of::<T>() != std::any::TypeId::of::<NoData>())
            && (d_type_name != distname)
        {
            // for all types except NoData , distance asked in reload declaration and distance in dump must be equal!
            let mut errmsg = String::from("error in distances : dumped distance is : ");
            errmsg.push_str(&distname);
            errmsg.push_str(" asked distance in loading is : ");
            errmsg.push_str(&d_type_name);
            error!(" distance in type argument : {:?}", d_type_name);
            error!("error , dump is for distance = {:?}", distname);
            return Err(anyhow!(errmsg));
        }
        let t_type = description.t_name.clone();
        info!("T type name in dump = {:?}", t_type);
        //
        //
        let layer_point_indexation = self.load_point_indexation(graph_in, &description, data_in)?;
        let data_dim = layer_point_indexation.get_data_dimension();
        //
        let hnsw: Hnsw<T, D> = Hnsw {
            max_nb_connection: description.max_nb_connection as usize,
            ef_construction: description.ef,
            extend_candidates: true,
            keep_pruned: false,
            max_layer: description.nb_layer as usize,
            layer_indexed_points: layer_point_indexation,
            data_dimension: data_dim,
            dist_f: f,
            searching: false,
            datamap_opt: false,
        };
        //
        debug!("load_hnsw_with_dist completed");
        // We cannot check that the pointer function was the same as the dump
        //
        Ok(hnsw)
    } // end of load_hnsw_with_dist

    fn load_point_indexation<'b, 'a, T>(
        &'a self,
        graph_in: &mut dyn Read,
        descr: &Description,
        data_in: &mut dyn Read,
    ) -> anyhow::Result<PointIndexation<'b, T>>
    where
        T: 'static + Serialize + DeserializeOwned + Clone + Sized + Send + Sync + std::fmt::Debug,
        'a: 'b,
    {
        //
        debug!(" in load_point_indexation");
        //
        // now we check that except for the case NoData, the typename are the sames.
        if std::any::TypeId::of::<T>() != std::any::TypeId::of::<NoData>()
            && std::any::type_name::<T>() != descr.t_name
        {
            error!(
                "typename loaded in  description {:?} do not correspond to instanciation type {:?}",
                descr.t_name,
                std::any::type_name::<T>()
            );
            panic!("incohrent size of T in description");
        }
        //
        let mut points_by_layer: Vec<Vec<Arc<Point<T>>>> =
            Vec::with_capacity(NB_LAYER_MAX as usize);
        let mut neighbourhood_map: HashMap<PointId, Vec<Vec<Neighbour>>> = HashMap::new();
        // load max layer
        let mut it_slice = [0u8; ::std::mem::size_of::<u8>()];
        graph_in.read_exact(&mut it_slice)?;
        let nb_layer = u8::from_ne_bytes(it_slice);
        debug!("nb layer {:?}", nb_layer);
        if nb_layer > NB_LAYER_MAX {
            return Err(anyhow!("inconsistent number of layErrers"));
        }
        //
        let mut nb_points_loaded: usize = 0;
        let mut nb_still_to_load = descr.nb_point as i64;
        let (use_mmap, max_nbpoint_in_memory) = self.options.use_mmap();
        //
        for l in 0..nb_layer as usize {
            // read and check magic
            debug!("loading layer {:?}", l);
            let mut it_slice = [0u8; ::std::mem::size_of::<u32>()];
            graph_in.read_exact(&mut it_slice)?;
            let magic = u32::from_ne_bytes(it_slice);
            if magic != MAGICLAYER {
                return Err(anyhow!("bad magic at layer beginning"));
            }
            let mut it_slice = [0u8; ::std::mem::size_of::<usize>()];
            graph_in.read_exact(&mut it_slice)?;
            let nbpoints = usize::from_ne_bytes(it_slice);
            debug!(" layer {:?} , nb points {:?}", l, nbpoints);
            let mut vlayer: Vec<Arc<Point<T>>> = Vec::with_capacity(nbpoints);
            // load graph and data part of point. Points are dumped in the same order.
            for r in 0..nbpoints {
                // do we use mmap? for this point. We must load into memory up to threshold points, and we also want the  most
                // frequently accessed points, i.e those in upper layers! to be physically loaded.
                // So we do use mmap from the moment the number of points yet to be loaded is less than threshold.
                let point_use_mmap = match use_mmap {
                    false => false,
                    true => {
                        if nb_still_to_load <= max_nbpoint_in_memory as i64 {
                            if log::log_enabled!(log::Level::Info)
                                && nb_still_to_load == max_nbpoint_in_memory as i64
                            {
                                info!(
                                    "Switching to points in memory. nb points stiil to load {:?}",
                                    nb_still_to_load
                                );
                            }
                            false
                        } else {
                            true
                        }
                    }
                };
                let load_point_res = self.load_point(graph_in, descr, data_in, point_use_mmap);
                if let Err(other) = load_point_res {
                    error!("in load_point_indexation, loading of point {} failed", r);
                    return Err(anyhow!(other));
                }

                let load_point_res = load_point_res.unwrap();
                let point = load_point_res.0;
                let p_id = point.get_point_id();
                // some checks
                assert_eq!(l, p_id.0 as usize);
                if r != p_id.1 as usize {
                    debug!("Origin= {:?},  p_id = {:?}", point.get_origin_id(), p_id);
                    debug!("Storing at l {:?}, r {:?}", l, r);
                }
                assert_eq!(r, p_id.1 as usize);
                // store neoghbour info of this point
                neighbourhood_map.insert(p_id, load_point_res.1);
                vlayer.push(point);
                nb_points_loaded += 1;
                nb_still_to_load -= 1;
                assert!(nb_still_to_load >= 0);
            }
            points_by_layer.push(vlayer);
        }
        // at this step all points are loaded , but without their neighbours fileds are not yet initialized
        let mut nbp: usize = 0;
        for (p_id, neighbours) in &neighbourhood_map {
            let point = &points_by_layer[p_id.0 as usize][p_id.1 as usize];
            for (l, neighbours) in neighbours.iter().enumerate() {
                for n in neighbours {
                    let n_point = &points_by_layer[n.p_id.0 as usize][n.p_id.1 as usize];
                    // now n_point is the Arc<Point> corresponding to neighbour n of point,
                    // construct a corresponding PointWithOrder
                    let n_pwo = PointWithOrder::<T>::new(n_point, n.distance);
                    point.neighbours.write()[l].push(Arc::new(n_pwo));
                } // end of for n
                //  must sort
                point.neighbours.write()[l].sort_unstable();
            } // end of for l
            nbp += 1;
            if nbp % 500_000 == 0 {
                debug!("reloading nb_points neighbourhood completed : {}", nbp);
            }
        } // end loop in neighbourhood_map
        //
        // get id of entry_point
        // load entry point
        info!(
            "end of layer loading, allocating PointIndexation, nb points loaded {:?}",
            nb_points_loaded
        );
        //
        let mut it_slice = [0u8; std::mem::size_of::<DataId>()];
        graph_in.read_exact(&mut it_slice)?;
        let origin_id = DataId::from_ne_bytes(it_slice);
        //
        let mut it_slice = [0u8; ::std::mem::size_of::<u8>()];
        graph_in.read_exact(&mut it_slice)?;
        let layer = u8::from_ne_bytes(it_slice);
        //
        let mut it_slice = [0u8; std::mem::size_of::<i32>()];
        graph_in.read_exact(&mut it_slice)?;
        let rank_in_l = i32::from_ne_bytes(it_slice);
        //
        info!(
            "found entry point, origin_id {:?} , layer {:?}, rank in layer {:?} ",
            origin_id, layer, rank_in_l
        );
        let entry_point = Arc::clone(&points_by_layer[layer as usize][rank_in_l as usize]);
        info!(
            " loaded entry point, origin_id {:} p_id {:?}",
            entry_point.get_origin_id(),
            entry_point.get_point_id()
        );
        //
        let point_indexation = PointIndexation {
            max_nb_connection: descr.max_nb_connection as usize,
            max_layer: NB_LAYER_MAX as usize,
            points_by_layer: Arc::new(RwLock::new(points_by_layer)),
            layer_g: LayerGenerator::new_with_scale(
                descr.max_nb_connection as usize,
                descr.level_scale,
                NB_LAYER_MAX as usize,
            ),
            nb_point: Arc::new(RwLock::new(nb_points_loaded)), // CAVEAT , we should increase , the whole thing is to be able to increment graph ?
            entry_point: Arc::new(RwLock::new(Some(entry_point))),
        };
        //
        debug!("Exiting load_pointIndexation");
        Ok(point_indexation)
    } // end of load_pointIndexation

    //
    //  Reload a point from a dump.
    //
    //  The graph part is loaded from graph_in file
    // the data vector itself is loaded from data_in
    //
    #[allow(clippy::type_complexity)]
    fn load_point<'b, 'a, T>(
        &'a self,
        graph_in: &mut dyn Read,
        descr: &Description,
        data_in: &mut dyn Read,
        point_use_mmap: bool,
    ) -> Result<(Arc<Point<'b, T>>, Vec<Vec<Neighbour>>)>
    where
        T: 'static + DeserializeOwned + Clone + Sized + Send + Sync + std::fmt::Debug,
        'a: 'b,
    {
        //
        //    debug!(" point load {:?} {:?}  ", p_id, origin_id);
        // Now  for each layer , read neighbours
        let load_res = load_point_graph(graph_in, descr);
        if load_res.is_err() {
            error!("load_point error reading graph data for point p_id");
            return Err(anyhow!("error reading graph data for point"));
        }
        let (origin_id, p_id, neighborhood) = load_res.unwrap();
        //
        let point = match point_use_mmap {
            false => {
                let v = load_point_data::<T>(origin_id, data_in, descr);
                if v.is_err() {
                    error!("loading point {:?}", origin_id);
                    std::process::exit(1);
                }
                Point::<T>::new(v.unwrap(), origin_id, p_id)
            }
            true => {
                skip_point_data(origin_id, data_in, descr)?; // keep cohrence between data file and graph file!
                debug!("constructing point from datamap, dataid : {:?}", origin_id);
                let s: Option<&'b [T]> = self.datamap.as_ref().unwrap().get_data::<T>(&origin_id);
                Point::<T>::new_from_mmap(s.unwrap(), origin_id, p_id)
            }
        };
        self.nb_point_loaded.fetch_add(1, Ordering::Relaxed);
        trace!(
            "load_point  origin {:?} allocated size {:?}, dim {:?}",
            origin_id,
            point.get_v().len(),
            descr.dimension
        );
        //
        Ok((Arc::new(point), neighborhood))
    } // end of load_point
} // end of Hnswio

/// structure describing main parameters for hnsnw data and written at the beginning of a dump file.
///
/// Name of distance and type of data must be encoded in the dump file for a coherent reload.
#[repr(C)]
pub struct Description {
    /// to keep track of format version
    pub format_version: usize,
    ///  value is 1 for Full 0 for Light
    pub dumpmode: u8,
    /// max number of connections in layers != 0
    pub max_nb_connection: u8,
    /// scale used in level sampling
    pub level_scale: f64,
    /// number of observed layers
    pub nb_layer: u8,
    /// search parameter
    pub ef: usize,
    /// total number of points
    pub nb_point: usize,
    /// data dimension
    pub dimension: usize,
    /// name of distance
    pub distname: String,
    /// T typename
    pub t_name: String,
}

impl Description {
    /// The dump of Description consists in :
    /// . The value MAGICDESCR_* as a u32 (4 u8)
    /// . The type of dump as u8
    /// . max_nb_connection as u8
    /// . ef (search parameter used in construction) as usize
    /// . nb_point (the number points dumped) as a usize
    /// . the name of distance used. (nb byes as a usize then list of bytes)
    ///
    fn dump<W: Write>(&self, argmode: DumpMode, out: &mut BufWriter<W>) -> Result<i32> {
        info!("in dump of description");
        out.write_all(&MAGICDESCR_4.to_ne_bytes())?;
        let mode: u8 = match argmode {
            DumpMode::Full => 1,
            _ => 0,
        };
        // CAVEAT should check mode == self.mode
        out.write_all(&mode.to_ne_bytes())?;
        // dump of max_nb_connection as u8!!
        out.write_all(&self.max_nb_connection.to_ne_bytes())?;
        // with MAGICDESCR_4 we must dump self.level_scale
        out.write_all(&self.level_scale.to_ne_bytes())?;
        //
        out.write_all(&self.nb_layer.to_ne_bytes())?;
        if self.nb_layer != NB_LAYER_MAX {
            println!("dump of Description, nb_layer != NB_MAX_LAYER");
            return Err(anyhow!("dump of Description, nb_layer != NB_MAX_LAYER"));
        }
        //
        info!("dumping ef {:?}", self.ef);
        out.write_all(&self.ef.to_ne_bytes())?;
        //
        info!("dumping nb point {:?}", self.nb_point);
        out.write_all(&self.nb_point.to_ne_bytes())?;
        //
        info!("dumping dimension of data {:?}", self.dimension);
        out.write_all(&self.dimension.to_ne_bytes())?;

        // dump of distance name
        let namelen: usize = self.distname.len();
        info!("distance name {:?} ", self.distname);
        out.write_all(&namelen.to_ne_bytes())?;
        out.write_all(self.distname.as_bytes())?;
        // dump of T value typename
        let namelen: usize = self.t_name.len();
        info!("T name {:?} ", self.t_name);
        out.write_all(&namelen.to_ne_bytes())?;
        out.write_all(self.t_name.as_bytes())?;
        //
        Ok(1)
    } // end fo dump

    /// return data typename
    pub fn get_typename(&self) -> String {
        self.t_name.clone()
    }

    /// returns dimension of data
    pub fn get_dimension(&self) -> usize {
        self.dimension
    }
} // end of HnswIO impl for Descr

//

/// This method is internally used by Hnswio.  
/// It is make *pub* as it can be used to retrieve the description of a dump.
/// It takes as input the graph part of the dump.
pub fn load_description(io_in: &mut dyn Read) -> Result<Description> {
    //
    let mut descr = Description {
        format_version: 0,
        dumpmode: 0,
        max_nb_connection: 0,
        level_scale: 1.0f64,
        nb_layer: 0,
        ef: 0,
        nb_point: 0,
        dimension: 0,
        distname: String::from(""),
        t_name: String::from(""),
    };
    //
    let mut it_slice = [0u8; std::mem::size_of::<u32>()];
    io_in.read_exact(&mut it_slice)?;
    let magic = u32::from_ne_bytes(it_slice);
    debug!(" magic {:X} ", magic);
    match magic {
        MAGICDESCR_2 => {
            descr.format_version = 2;
        }
        MAGICDESCR_3 => {
            descr.format_version = 3;
        }
        MAGICDESCR_4 => {
            descr.format_version = 4;
        }
        _ => {
            error!("bad magic");
            return Err(anyhow!("bad magic at descr beginning"));
        }
    }
    let mut it_slice = [0u8; std::mem::size_of::<u8>()];
    io_in.read_exact(&mut it_slice)?;
    descr.dumpmode = u8::from_ne_bytes(it_slice);
    info!(" dumpmode {:?} ", descr.dumpmode);
    //
    let mut it_slice = [0u8; std::mem::size_of::<u8>()];
    io_in.read_exact(&mut it_slice)?;
    descr.max_nb_connection = u8::from_ne_bytes(it_slice);
    info!(" max_nb_connection {:?} ", descr.max_nb_connection);
    //
    if descr.format_version == 4 {
        // we read modification for level sampling
        let mut it_slice = [0u8; std::mem::size_of::<f64>()];
        io_in.read_exact(&mut it_slice)?;
        descr.level_scale = f64::from_ne_bytes(it_slice);
        info!(" level scale : {:.2e}", descr.level_scale);
    }
    //
    let mut it_slice = [0u8; std::mem::size_of::<u8>()];
    io_in.read_exact(&mut it_slice)?;
    descr.nb_layer = u8::from_ne_bytes(it_slice);
    info!("nb_layer  {:?} ", descr.nb_layer);
    // ef
    let mut it_slice = [0u8; std::mem::size_of::<usize>()];
    io_in.read_exact(&mut it_slice)?;
    descr.ef = usize::from_ne_bytes(it_slice);
    info!("ef  {:?} ", descr.ef);
    // nb_point
    let mut it_slice = [0u8; std::mem::size_of::<usize>()];
    io_in.read_exact(&mut it_slice)?;
    descr.nb_point = usize::from_ne_bytes(it_slice);
    // read dimension
    let mut it_slice = [0u8; std::mem::size_of::<usize>()];
    io_in.read_exact(&mut it_slice)?;
    descr.dimension = usize::from_ne_bytes(it_slice);
    info!(
        "nb_point {:?} dimension {:?} ",
        descr.nb_point, descr.dimension
    );
    // distance name
    let mut it_slice = [0u8; std::mem::size_of::<usize>()];
    io_in.read_exact(&mut it_slice)?;
    let len: usize = usize::from_ne_bytes(it_slice);
    debug!("length of distance name {:?} ", len);
    if len > 256 {
        info!(" length of distance name > 256");
        println!(" length of distance name should not exceed 256");
        return Err(anyhow!("bad length for distance name"));
    }
    let mut distv = vec![0; len];
    io_in.read_exact(distv.as_mut_slice())?;
    let distname = String::from_utf8(distv).unwrap();
    debug!("distance name {:?} ", distname);
    descr.distname = distname;
    // reload of type name
    let mut it_slice = [0u8; std::mem::size_of::<usize>()];
    io_in.read_exact(&mut it_slice)?;
    let len: usize = usize::from_ne_bytes(it_slice);
    debug!("length of T  name {:?} ", len);
    if len > 256 {
        println!(" length of T name should not exceed 256");
        return Err(anyhow!("bad lenght for T name"));
    }
    let mut tnamev = vec![0; len];
    io_in.read_exact(tnamev.as_mut_slice())?;
    let t_name = String::from_utf8(tnamev).unwrap();
    debug!("T type name {:?} ", t_name);
    descr.t_name = t_name;
    debug!(" end of description load \n");
    //
    Ok(descr)
}

//
// dump and load of Point<T>
// ==========================
//

///  Graph part of point dump
/// dump of a point consist in  
///  1. The value MAGICPOINT
///  2. its identity ( a usize  rank in original data , hash value or else , and PointId)
///  3. for each layer dump of the number of neighbours followed by :
///     for each neighbour dump of its identity (: usize) and then distance (): u32) to point dumped.
///
/// identity of a point is in full mode the triplet origin_id (: usize), layer (: u8) rank_in_layer (: u32)
///                           light mode only origin_id (: usize)
///  For data dump
///  1. The value MAGICDATAP (u32)
///  2. origin_id as a u64
///  3. The vector of data (the length is known from Description)
///
fn dump_point<T: Serialize + Clone + Sized + Send + Sync, W: Write>(
    point: &Point<T>,
    mode: DumpMode,
    graphout: &mut BufWriter<W>,
    dataout: &mut BufWriter<W>,
) -> Result<i32> {
    //
    graphout.write_all(&MAGICPOINT.to_ne_bytes())?;
    // dump ext_id: usize , layer : u8 , rank in layer : i32
    graphout.write_all(&point.get_origin_id().to_ne_bytes())?;
    let p_id = point.get_point_id();
    if mode == DumpMode::Full {
        graphout.write_all(&p_id.0.to_ne_bytes())?;
        graphout.write_all(&p_id.1.to_ne_bytes())?;
    }
    trace!(" point dump {:?} {:?}  ", p_id, point.get_origin_id());
    // then dump neighborhood info : nb neighbours : u32 , then list of origin_id, layer, rank_in_layer
    let neighborhood = point.get_neighborhood_id();
    // in any case nb_layers are dumped with possibly 0 neighbours at a layer, but this does not occur by construction
    for (l, neighbours_at_l) in neighborhood.iter().enumerate() {
        // Caution : we dump number of neighbours as a usize, even if it cannot be so large!
        let nbg_l: usize = neighbours_at_l.len();
        trace!("\t dumping nbng : {} at l {}", nbg_l, l);
        graphout.write_all(&nbg_l.to_ne_bytes())?;
        for n in neighbours_at_l {
            // dump d_id : uszie , distance : f32, layer : u8, rank in layer : i32
            graphout.write_all(&n.d_id.to_ne_bytes())?;
            if mode == DumpMode::Full {
                graphout.write_all(&n.p_id.0.to_ne_bytes())?;
                graphout.write_all(&n.p_id.1.to_ne_bytes())?;
            }
            graphout.write_all(&n.distance.to_ne_bytes())?;
            //                debug!("        voisins  {:?}  {:?}  {:?}", n.p_id,  n.d_id , n.distance);
        }
    }
    // now we dump data vector!
    dataout.write_all(&MAGICDATAP.to_ne_bytes())?;
    let origin_u64 = point.get_origin_id() as u64;
    dataout.write_all(&origin_u64.to_ne_bytes())?;
    //
    let serialized = unsafe {
        std::slice::from_raw_parts(
            point.get_v().as_ptr() as *const u8,
            std::mem::size_of_val(point.get_v()),
        )
    };
    trace!("serializing len {:?}", serialized.len());
    let len_64 = serialized.len() as u64;
    dataout.write_all(&len_64.to_ne_bytes())?;
    dataout.write_all(serialized)?;
    //
    Ok(1)
} // end of dump for Point<T>

// just reload data vector for point from file where data were dumped
// used when we do not used memory map in reload
fn load_point_data<T>(
    origin_id: usize,
    data_in: &mut dyn Read,
    descr: &Description,
) -> Result<Vec<T>>
where
    T: 'static + DeserializeOwned + Clone + Sized + Send + Sync,
{
    //
    trace!("load_point_data , origin id : {}", origin_id);
    //
    // construct a point from data_in
    //
    let mut it_slice = [0u8; std::mem::size_of::<u32>()];
    data_in.read_exact(&mut it_slice)?;
    let magic = u32::from_ne_bytes(it_slice);
    assert_eq!(
        magic, MAGICDATAP,
        "magic not equal to MAGICDATAP in load_point, point_id : {:?} ",
        origin_id
    );
    // read origin id
    let mut it_slice = [0u8; std::mem::size_of::<u64>()];
    data_in.read_exact(&mut it_slice)?;
    let origin_id_data = u64::from_ne_bytes(it_slice) as usize;
    assert_eq!(
        origin_id, origin_id_data,
        "origin_id incoherent between graph and data"
    );
    // now read data. we use size_t that is in description, to take care of the casewhere we reload
    let mut it_slice = [0u8; std::mem::size_of::<u64>()];
    data_in.read_exact(&mut it_slice)?;
    let serialized_len = u64::from_ne_bytes(it_slice);
    trace!("serialized len to reload {:?}", serialized_len);
    let mut v_serialized = vec![0; serialized_len as usize];
    data_in.read_exact(&mut v_serialized)?;

    let v: Vec<T> = if std::any::TypeId::of::<T>() != std::any::TypeId::of::<NoData>() {
        match descr.format_version {
            2 => bincode::deserialize(&v_serialized).unwrap(),
            3 | 4 => {
                let slice_t = unsafe {
                    std::slice::from_raw_parts(v_serialized.as_ptr() as *const T, descr.dimension)
                };
                slice_t.to_vec()
            }
            _ => {
                error!(
                    "error in load_point, unknow format_version : {:?}",
                    descr.format_version
                );
                std::process::exit(1);
            }
        }
    } else {
        Vec::new()
    };
    //
    Ok(v)
} // end of load_point_data

// We need to maintain coherence in data and graph stream, so we read to keep in phase
fn skip_point_data(origin_id: usize, data_in: &mut dyn Read, _descr: &Description) -> Result<()> {
    //
    let mut it_slice = [0u8; std::mem::size_of::<u32>()];
    data_in.read_exact(&mut it_slice)?;
    let magic = u32::from_ne_bytes(it_slice);
    assert_eq!(
        magic, MAGICDATAP,
        "magic not equal to MAGICDATAP in load_point, point_id : {:?} ",
        origin_id
    );
    // read origin id
    let mut it_slice = [0u8; std::mem::size_of::<u64>()];
    data_in.read_exact(&mut it_slice)?;
    let origin_id_data = u64::from_ne_bytes(it_slice) as usize;
    assert_eq!(
        origin_id, origin_id_data,
        "origin_id incoherent between graph and data"
    );
    //
    // now read data. we use size_t that is in description, to take care of the casewhere we reload
    let mut it_slice = [0u8; std::mem::size_of::<u64>()];
    data_in.read_exact(&mut it_slice)?;
    let serialized_len = u64::from_ne_bytes(it_slice);
    trace!(
        "skip_point_data : serialized len to reload {:?}",
        serialized_len
    );
    let mut v_serialized = vec![0; serialized_len as usize];
    data_in.read_exact(&mut v_serialized)?;
    //
    Ok(())
} // end of skip_point_data

//==================================================================================

/// This structure gathers info loaded in dumped graph file for a point.
type PointGraphInfo = (usize, PointId, Vec<Vec<Neighbour>>);

// This function reads neighbourhood info and returns neighbourhood info.
// It suppose and requires that the file graph_in is just at beginning of info related to origin_id
fn load_point_graph(graph_in: &mut dyn Read, descr: &Description) -> Result<PointGraphInfo> {
    //
    trace!("in load_point_graph");
    // read and check magic
    let mut it_slice = [0u8; std::mem::size_of::<u32>()];
    graph_in.read_exact(&mut it_slice).unwrap();
    let magic = u32::from_ne_bytes(it_slice);
    if magic != MAGICPOINT {
        error!("got instead of MAGICPOINT {:x}", magic);
        return Err(anyhow!("bad magic at point beginning"));
    }
    let mut it_slice = [0u8; std::mem::size_of::<DataId>()];
    graph_in.read_exact(&mut it_slice).unwrap();
    let origin_id = DataId::from_ne_bytes(it_slice);
    //
    // read point_id
    let mut it_slice = [0u8; std::mem::size_of::<u8>()];
    graph_in.read_exact(&mut it_slice).unwrap();
    let layer = u8::from_ne_bytes(it_slice);
    //
    let mut it_slice = [0u8; std::mem::size_of::<i32>()];
    graph_in.read_exact(&mut it_slice).unwrap();
    let rank_in_l = i32::from_ne_bytes(it_slice);
    let p_id = PointId(layer, rank_in_l);
    debug!(
        "in load_point_graph, got origin_id : {}, p_id : {:?}",
        origin_id, p_id
    );
    //
    // Now  for each layer , read neighbours
    let nb_layer = descr.nb_layer;
    let mut neighborhood = Vec::<Vec<Neighbour>>::with_capacity(NB_LAYER_MAX as usize);
    for _l in 0..nb_layer {
        let mut neighbour: Neighbour = Default::default();
        // read nb_neighbour as usize!!! CAUTION, then nb_neighbours times identity(depends on Full or Light) distance : f32
        let mut it_slice = [0u8; std::mem::size_of::<usize>()];
        graph_in.read_exact(&mut it_slice).unwrap();
        let nb_neighbours = usize::from_ne_bytes(it_slice);
        let mut neighborhood_l: Vec<Neighbour> = Vec::with_capacity(nb_neighbours);
        for _j in 0..nb_neighbours {
            let mut it_slice = [0u8; std::mem::size_of::<DataId>()];
            graph_in.read_exact(&mut it_slice).unwrap();
            neighbour.d_id = DataId::from_ne_bytes(it_slice);
            if descr.dumpmode == 1 {
                let mut it_slice = [0u8; std::mem::size_of::<u8>()];
                graph_in.read_exact(&mut it_slice).unwrap();
                neighbour.p_id.0 = u8::from_ne_bytes(it_slice);
                //
                let mut it_slice = [0u8; std::mem::size_of::<i32>()];
                graph_in.read_exact(&mut it_slice).unwrap();
                neighbour.p_id.1 = i32::from_ne_bytes(it_slice);
            }
            let mut it_slice = [0u8; std::mem::size_of::<f32>()];
            graph_in.read_exact(&mut it_slice).unwrap();
            neighbour.distance = f32::from_ne_bytes(it_slice);
            //  debug!("        voisins  load {:?} {:?} {:?} ", neighbour.p_id, neighbour.d_id , neighbour.distance);
            // now we have a new neighbour, we must really fill neighbourhood info, so it means going from Neighbour to PointWithOrder
            neighborhood_l.push(neighbour);
        }
        neighborhood.push(neighborhood_l);
    }
    for _l in nb_layer..NB_LAYER_MAX {
        neighborhood.push(Vec::<Neighbour>::new());
    }
    //
    let point_grap_info = (origin_id, p_id, neighborhood);
    //
    Ok(point_grap_info)
} // end of load_point_graph

//
// dump and load of PointIndexation<T>
// ===================================
//
//
// nb_layer : 8
// a magick at each Layer : u32
// . number of points in layer (usize),
// . list of point of layer
// dump entry point
//
impl<T: Serialize + DeserializeOwned + Clone + Send + Sync> HnswIoT for PointIndexation<'_, T> {
    fn dump(&self, mode: DumpMode, dumpinit: &mut DumpInit) -> Result<i32> {
        let graphout = &mut dumpinit.graph_out;
        let dataout = &mut dumpinit.data_out;
        // dump max_layer
        let layers = self.points_by_layer.read();
        let nb_layer = layers.len() as u8;
        graphout.write_all(&nb_layer.to_ne_bytes())?;
        // dump layers from lower (most populatated to higher level)
        for i in 0..layers.len() {
            let nb_point = layers[i].len();
            debug!("dumping layer {:?}, nb_point {:?}", i, nb_point);
            graphout.write_all(&MAGICLAYER.to_ne_bytes())?;
            graphout.write_all(&nb_point.to_ne_bytes())?;
            for j in 0..layers[i].len() {
                assert_eq!(layers[i][j].get_point_id(), PointId(i as u8, j as i32));
                dump_point(&layers[i][j], mode, graphout, dataout)?;
            }
        }
        // dump id of entry point
        let ep_read = self.entry_point.read();
        let ep = ep_read
            .as_ref()
            .ok_or(anyhow!("entry point not initialized"))?;
        //let ep = ep_read.as_ref().unwrap();
        graphout.write_all(&ep.get_origin_id().to_ne_bytes())?;
        let p_id = ep.get_point_id();
        if mode == DumpMode::Full {
            graphout.write_all(&p_id.0.to_ne_bytes())?;
            graphout.write_all(&p_id.1.to_ne_bytes())?;
        }
        info!(
            "dumped entry_point origin_d {:?}, p_id {:?} ",
            ep.get_origin_id(),
            p_id
        );
        //
        Ok(1)
    } // end of dump for PointIndexation<T>
} // end of impl HnswIO

//
// dump and load of Hnsw<T>
// =========================
//
//

impl<T: Serialize + DeserializeOwned + Clone + Sized + Send + Sync, D: Distance<T> + Send + Sync>
    HnswIoT for Hnsw<'_, T, D>
{
    /// The dump method for hnsw.  
    /// - graphout is a BufWriter dedicated to the dump of the graph part of Hnsw
    /// - dataout is a bufWriter dedicated to the dump of the data stored in the Hnsw structure.
    fn dump(&self, mode: DumpMode, dumpinit: &mut DumpInit) -> anyhow::Result<i32> {
        //
        let graphout = &mut dumpinit.graph_out;
        let dataout = &mut dumpinit.data_out;
        // dump description , then PointIndexation
        let dumpmode: u8 = match mode {
            DumpMode::Full => 1,
            _ => 0,
        };
        let datadim: usize = self.layer_indexed_points.get_data_dimension();
        let level_scale = self.layer_indexed_points.get_level_scale();
        let description = Description {
            format_version: 3,
            //  value is 1 for Full 0 for Light
            dumpmode,
            max_nb_connection: self.get_max_nb_connection(),
            level_scale,
            nb_layer: self.get_max_level() as u8,
            ef: self.get_ef_construction(),
            nb_point: self.get_nb_point(),
            dimension: datadim,
            distname: self.get_distance_name(),
            t_name: type_name::<T>().to_string(),
        };
        debug!("dump  obtained typename {:?}", type_name::<T>());
        description.dump(mode, graphout)?;
        // We must dump a header for dataout.
        dataout.write_all(&MAGICDATAP.to_ne_bytes())?;
        dataout.write_all(&datadim.to_ne_bytes())?;
        //
        self.layer_indexed_points.dump(mode, dumpinit)?;
        Ok(1)
    }
} // end impl block for Hnsw

//===============================================================================================================

#[cfg(test)]

mod tests {
    use super::*;

    pub use crate::api::AnnT;
    use anndists::dist;
    use log::error;

    use rand::distr::{Distribution, Uniform};

    fn log_init_test() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    fn my_fn(v1: &[f32], v2: &[f32]) -> f32 {
        let norm_l1: f32 = v1.iter().zip(v2.iter()).map(|t| (*t.0 - *t.1).abs()).sum();
        norm_l1
    }

    #[test]
    fn test_dump_reload_1() {
        println!("\n\n test_dump_reload_1");
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
        let hnsw = Hnsw::<f32, dist::DistL1>::new(
            nb_connection,
            nbcolumn,
            16,
            ef_construct,
            dist::DistL1 {},
        );
        for (i, d) in data.iter().enumerate() {
            hnsw.insert((d, i));
        }
        // some loggin info
        hnsw.dump_layer_info();
        // dump in a file.  Must take care of name as tests runs in // !!!
        let fname = "dumpreloadtest1";
        let directory = tempfile::tempdir().unwrap();
        let _res = hnsw.file_dump(directory.path(), fname);
        //
        // reload
        debug!("\n\n test_dump_reload_1 hnsw reload");
        // we will need a procedural macro to get from distance name to its instanciation.
        // from now on we test with DistL1
        let mut reloader = HnswIo::new(directory.path(), fname);
        let hnsw_loaded: Hnsw<f32, DistL1> = reloader.load_hnsw::<f32, DistL1>().unwrap();
        // test equality
        check_graph_equality(&hnsw_loaded, &hnsw);
    } // end of test_dump_reload

    #[test]
    fn test_dump_reload_myfn() {
        println!("\n\n test_dump_reload_myfn");
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
        let mydist = dist::DistPtr::<f32, f32>::new(my_fn);
        let hnsw = Hnsw::<f32, dist::DistPtr<f32, f32>>::new(
            nb_connection,
            nbcolumn,
            16,
            ef_construct,
            mydist,
        );
        for (i, d) in data.iter().enumerate() {
            hnsw.insert((d, i));
        }
        // some loggin info
        hnsw.dump_layer_info();
        let fname = "dumpreloadtest_myfn";
        let directory = tempfile::tempdir().unwrap();

        let _res = hnsw.file_dump(directory.path(), fname);
        // This will dump in 2 files named dumpreloadtest.hnsw.graph and dumpreloadtest.hnsw.data
        //
        // reload
        debug!("HNSW reload");
        let reloader = HnswIo::new(directory.path(), fname);
        let mydist = dist::DistPtr::<f32, f32>::new(my_fn);
        let _hnsw_loaded: Hnsw<f32, DistPtr<f32, f32>> =
            reloader.load_hnsw_with_dist(mydist).unwrap();
    } // end of test_dump_reload_myfn

    #[test]
    fn test_dump_reload_graph_only() {
        println!("\n\n test_dump_reload_graph_only");
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
        let hnsw = Hnsw::<f32, dist::DistL1>::new(
            nb_connection,
            nbcolumn,
            16,
            ef_construct,
            dist::DistL1 {},
        );
        for (i, d) in data.iter().enumerate() {
            hnsw.insert((d, i));
        }
        // some loggin info
        hnsw.dump_layer_info();
        // dump in a file. Must take care of name as tests runs in // !!!
        let fname = "dumpreloadtestgraph";
        let directory = tempfile::tempdir().unwrap();
        let _res = hnsw.file_dump(directory.path(), fname);
        // This will dump in 2 files named dumpreloadtest.hnsw.graph and dumpreloadtest.hnsw.data
        //
        // reload
        debug!("\n\n  hnsw reload");
        let mut reloader = HnswIo::new(directory.path(), fname);
        let hnsw_loaded: Hnsw<NoData, NoDist> = reloader.load_hnsw().unwrap();
        // test equality
        check_graph_equality(&hnsw_loaded, &hnsw);
    } // end of test_dump_reload

    // this tests reloads a dump with memory mapping of data, inserts new data and redump
    #[test]
    fn reload_with_mmap() {
        println!("\n\n hnswio tests : reload_with_mmap");
        log_init_test();
        // generate a random test
        let mut rng = rand::rng();
        let unif = Uniform::<f32>::new(0., 1.).unwrap();
        // 100 vectors of size 10 f32
        let nbcolumn = 100;
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
        //
        let first: Vec<f32> = data[0].clone();
        info!("data[0] = {:?}", first);
        // define hnsw
        let ef_construct = 25;
        let nb_connection = 10;
        let hnsw = Hnsw::<f32, dist::DistL1>::new(
            nb_connection,
            nbcolumn,
            16,
            ef_construct,
            dist::DistL1 {},
        );
        for (i, d) in data.iter().enumerate() {
            hnsw.insert((d, i));
        }
        // some loggin info
        hnsw.dump_layer_info();
        // dump in a file.  Must take care of name as tests runs in // !!!
        let fname = "mmapreloadtest";
        let directory = tempfile::tempdir().unwrap();
        let dumpname = hnsw.file_dump(directory.path(), fname).unwrap();
        debug!("dump succeeded in file basename : {}", dumpname);
        //
        // reload reload_with_mmap
        debug!("HNSW reload");
        let mut reloader = HnswIo::new(directory.path(), &dumpname);
        // use mmap for points after half number of points
        let options = ReloadOptions::default().set_mmap_threshold(nbcolumn / 2);
        reloader.set_options(options);
        let hnsw_loaded: Hnsw<f32, DistL1> = reloader.load_hnsw::<f32, DistL1>().unwrap();
        // test equality
        check_graph_equality(&hnsw_loaded, &hnsw);
        // We add nbcolumn new vectors
        info!("adding points in hnsw reloaded");
        let nbcolumn = 5;
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
        let first_with_mmap: Vec<f32> = data[0].clone();
        info!(
            "first added after reloading with mmap : data[0] = {:?}",
            first_with_mmap
        );
        let nb_in = hnsw.get_nb_point();
        for (i, d) in data.iter().enumerate() {
            hnsw.insert((d, i + nb_in));
        }
        //
        let search_res = hnsw.search(&first, 5, ef_construct);
        info!("neighbours od first point inserted");
        for n in &search_res {
            info!("neighbour: {:?}", n);
        }
        assert_eq!(search_res[0].d_id, 0);
        assert_eq!(search_res[0].distance, 0.);
        let search_res = hnsw.search(&first_with_mmap, 5, ef_construct);
        info!("neighbours of first point inserted after reload with mmap");
        for n in &search_res {
            info!("neighbour {:?}", n);
        }
        if search_res[0].d_id != nb_in {
            // with very low probability it could happen that we find a very near point!
            // then distance should very small
            info!(
                "neighbour found for point id : {}, distance : {:.2e}, should have been id : {}, dist : {:.2e}",
                search_res[0].d_id, search_res[0].distance, nb_in, 0.
            );
        }
        assert_eq!(search_res[0].d_id, nb_in);
        assert_eq!(search_res[0].distance, 0.);
        //
        // TODO: redump  and care about mmapped file, so we do not overwrite
        //
        let dump_init = DumpInit::new(directory.path(), fname, false);
        info!("will use basename : {}", dump_init.get_basename());
        let res = hnsw.file_dump(directory.path(), dump_init.get_basename());
        if res.is_err() {
            error!("hnsw.file_dump failed");
            std::panic!("hnsw.file_dump failed");
        }
    } // end of reload_with_mmap

    #[test]
    fn test_bincode() {
        let mut rng = rand::rng();
        let unif = Uniform::<f32>::new(0., 1.).unwrap();
        let size = 10;
        let mut xsi;
        let mut data = Vec::with_capacity(size);
        for _ in 0..size {
            xsi = unif.sample(&mut rng);
            println!("xsi = {:?}", xsi);
            data.push(xsi);
        }
        println!("to serialized {:?}", data);

        let v_serialized: Vec<u8> = bincode::serialize(&data).unwrap();
        debug!("serializing len {:?}", v_serialized.len());
        let v_deserialized: Vec<f32> = bincode::deserialize(&v_serialized).unwrap();
        println!("deserialized {:?}", v_deserialized);
    }

    #[test]
    fn read_write_empty_db() -> Result<()> {
        log_init_test();
        let ef_construct = 25;
        let nb_connection = 10;
        let hnsw =
            Hnsw::<f32, dist::DistL1>::new(nb_connection, 0, 16, ef_construct, dist::DistL1 {});
        let fname = "empty_db";
        let directory = tempfile::tempdir()?;
        let _res = hnsw.file_dump(directory.path(), fname);
        let mut reloader = HnswIo::new(directory.path(), fname);
        let hnsw_loaded_res = reloader.load_hnsw::<f32, DistL1>();
        assert!(hnsw_loaded_res.is_err());
        Ok(())
    }
} // end module tests
