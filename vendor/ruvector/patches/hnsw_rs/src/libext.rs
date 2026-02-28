//! This file contains lib to call hnsw from julia (or any language providing a C api)
//! The AnnT trait is implemented with macros for u32, u16, u8, f32, f64 and i32.  
//! The macro declare_myapi_type!  produces struct HnswApif32 and so on.
//!

#![allow(non_camel_case_types)]

use core::ffi::c_ulonglong;
use std::fs::OpenOptions;
use std::io::BufReader;
use std::path::PathBuf;
use std::ptr;

use anndists::dist::distances::*;
use log::{debug, error, info, trace, warn};

use crate::api::*;
use crate::hnsw::*;
use crate::hnswio::*;

//========== Hnswio

/// returns a pointer to a Hnswio
/// args corresponds to string giving base filename of dump, supposed to be in current directory
/// # Safety
/// pointer must be char* pointer to the string
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_hnswio(flen: u64, name: *const u8) -> *const HnswIo {
    let slice = unsafe { std::slice::from_raw_parts(name, flen as usize) };
    let filename = String::from_utf8_lossy(slice).into_owned();
    let hnswio = HnswIo::new(std::path::Path::new("."), &filename);
    Box::into_raw(Box::new(hnswio))
}

//=================
// the export macro makes the macro global in crate and accecssible via crate::declare_myapi_type!
#[macro_export]
macro_rules! declare_myapi_type(
    ($name:ident, $ty:ty) => (
        pub struct $name {
#[allow(dead_code)]
        pub(crate) opaque: Box<dyn AnnT<Val=$ty>>,
        } // end struct
        impl $name {
            pub fn new(arg: Box<dyn AnnT<Val=$ty>>) -> Self {
                $name{ opaque:arg}
            } // end new
        } // end impl
    )
);

declare_myapi_type!(HnswApiNodata, NoData);

// declare_myapi_type!(HnswApif64, f64);
// declare_myapi_type!(HnswApif32, f32);

/// to be able to return a vector from rust in a julia struct before converting to a julia Vector
#[repr(C)]
pub struct Vec_api<T> {
    len: i64,
    ptr: *const T,
} // end struct

#[repr(C)]
/// The basic Neighbour info returned by api
pub struct Neighbour_api {
    /// id of neighbour
    pub id: usize,
    /// distance of data sent in request to this neighbour
    pub d: f32,
}

impl From<&Neighbour> for Neighbour_api {
    fn from(neighbour: &Neighbour) -> Self {
        Neighbour_api {
            id: neighbour.d_id,
            d: neighbour.distance,
        }
    }
}

#[repr(C)]
/// The response to a neighbour search requests
pub struct Neighbourhood_api {
    pub nbgh: i64,
    pub neighbours: *const Neighbour_api,
}

#[repr(C)]
pub struct Neighbour_api_parsearch_answer {
    /// The number of answers (o request), i.e size of both vectors nbgh and neighbours
    pub nb_answer: usize,
    /// for each request, we get a Neighbourhood_api
    pub neighbourhoods: *const Neighbourhood_api,
}

//===================================== f32  type =====================================

// macros have been exported to the root of the crate so we do not refer to them via api::
super::declare_myapi_type!(HnswApif32, f32);
super::declare_myapi_type!(HnswApif64, f64);

//===================================================================================================
//  These are the macros to generate trait implementation for useful numeric types
#[allow(unused_macros)]
macro_rules! generate_insert(
($function_name:ident, $api_name:ty, $type_val:ty) => (
        /// # Safety
        /// The function is unsafe because it dereferences a raw pointer
        ///
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $function_name(hnsw_api : *mut $api_name, len:usize, data : *const $type_val, id : usize) {
        trace!("entering insert, type {:?} vec len is {:?}, id : {:?} ", stringify!($type_val), len, id);
        //  construct vector: Rust clones and take ownership.
        let data_v : Vec<$type_val>;
        unsafe {
            let slice = std::slice::from_raw_parts(data, len);
            data_v = Vec::from(slice);
            trace!("calling insert data");
            (*hnsw_api).opaque.insert_data(&data_v, id);
        }
        trace!("exiting insert for type {:?}", stringify!($type_val));
        }  // end of insert
    )
);

macro_rules! generate_parallel_insert(
($function_name:ident, $api_name:ty, $type_val:ty) => (
        /// # Safety
        /// The function is unsafe because it dereferences a raw pointer
        ///
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $function_name(hnsw_api : *mut $api_name, nb_vec: usize, vec_len : usize,
                        datas : *mut *const $type_val, ids : *const usize) {
            //
            trace!("entering parallel_insert type {:?}  , vec len is {:?}, nb_vec : {:?}", stringify!($type_val), vec_len, nb_vec);
            let data_ids : Vec<usize>;
            let data_ptrs : Vec<*const $type_val>;
            unsafe {
                let slice = std::slice::from_raw_parts(ids, nb_vec);
                data_ids = Vec::from(slice);
            }
            // debug!("got ids");
            unsafe {
                let slice = std::slice::from_raw_parts(datas, nb_vec);
                data_ptrs = Vec::from(slice);
            }
            // debug!("got data ptrs");
            let mut data_v = Vec::<Vec<$type_val>>::with_capacity(nb_vec);
            for i in 0..nb_vec {
                unsafe {
                    let slice = std::slice::from_raw_parts(data_ptrs[i], vec_len);
                    let v = Vec::from(slice);
                    data_v.push(v);
                }
            }
            // debug!("sending request");
            let mut request : Vec<(&Vec<$type_val>, usize)> = Vec::with_capacity(nb_vec);
            for i in 0..nb_vec {
                request.push((&data_v[i], data_ids[i]));
            }
            //
            unsafe { (*hnsw_api).opaque.parallel_insert_data(&request); };
            trace!("exiting parallel_insert");
        } // end of parallel_insert
    )
);

macro_rules! generate_search_neighbours(
($function_name:ident, $api_name:ty, $type_val:ty) => (
        /// # Safety
        /// The function is unsafe because it dereferences a raw pointer
        ///
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $function_name(hnsw_api : *const $api_name, len:usize, data : *const $type_val,
                                knbn : usize, ef_search : usize) ->  *const Neighbourhood_api {
            //
            trace!("entering search_neighbours type {:?}, vec len is {:?}, id : {:?} ef_search {:?}", stringify!($type_val), len, knbn, ef_search);
            let data_v : Vec<$type_val>;
            let neighbours : Vec<Neighbour>;
            unsafe {
                let slice = std::slice::from_raw_parts(data, len);
                data_v = Vec::from(slice);
                trace!("calling search neighbours {:?}", data_v);
                neighbours =  (*hnsw_api).opaque.search_neighbours(&data_v, knbn, ef_search);
            }
            let neighbours_api : Vec<Neighbour_api> = neighbours.iter().map(|n| Neighbour_api::from(n)).collect();
            trace!(" got nb neighbours {:?}", neighbours_api.len());
            // for i in 0..neighbours_api.len() {
            //    println!(" id {:?}  dist : {:?} ", neighbours_api[i].id, neighbours_api[i].d);
            // }
            let nbgh_i = neighbours.len() as i64;
            let neighbours_ptr = neighbours_api.as_ptr();
            std::mem::forget(neighbours_api);
            let answer = Neighbourhood_api {
                    nbgh : nbgh_i,
                    neighbours : neighbours_ptr,
            };
            trace!("search_neighbours returning nb neighbours {:?} id ptr {:?} ", nbgh_i, neighbours_ptr);
            Box::into_raw(Box::new(answer))
        }
    )
);

macro_rules! generate_parallel_search_neighbours(
($function_name:ident, $api_name:ty, $type_val:ty) => (
        #[unsafe(no_mangle)]
        /// search nb_vec of size vec_len. The the searches will be done in // as far as possible.
        /// # Safety
        /// The function is unsafe because it dereferences a raw pointer
        ///
        pub unsafe extern "C" fn $function_name(hnsw_api : *const $api_name, nb_vec : usize, vec_len :i64,
                            data : *mut *const $type_val, knbn : usize, ef_search : usize) ->  *const Vec_api<Neighbourhood_api> {
            //
            // must build a Vec<Vec<f32> to build request
            trace!("recieving // search request for type: {:?} with {:?} vectors", stringify!($type_val), nb_vec);
            let neighbours : Vec<Vec<Neighbour> >;
            let mut data_v = Vec::<Vec<$type_val>>::with_capacity(nb_vec);
            unsafe {
                let slice = std::slice::from_raw_parts(data, nb_vec);
                let ptr_list : Vec<*const $type_val> = Vec::from(slice);
                for i in 0..nb_vec {
                    let slice_i = std::slice::from_raw_parts(ptr_list[i], vec_len as usize);
                    let v = Vec::from(slice_i);
                    data_v.push(v);
                }
            // debug!(" reconstructed input vectors");
            neighbours =  (*hnsw_api).opaque.parallel_search_neighbours(&data_v, knbn, ef_search);
            }
            // construct a vector of Neighbourhood_api
            // reverse work, construct 2 arrays, one vector of Neighbours, and one vectors of number of returned neigbours by input a vector.
            let mut neighbour_lists = Vec::<Neighbourhood_api>::with_capacity(nb_vec);
            for v in neighbours {
                let neighbours_api : Vec<Neighbour_api> = v.iter().map(|n| Neighbour_api::from(n)).collect();
                let nbgh = neighbours_api.len();
                let neighbours_api_ptr = neighbours_api.as_ptr();
                std::mem::forget(neighbours_api);
                let v_answer = Neighbourhood_api {
                        nbgh : nbgh as i64,
                        neighbours: neighbours_api_ptr,
                    };
                neighbour_lists.push(v_answer);
            }
            trace!(" reconstructed output  pointers to vectors");
            let neighbour_lists_ptr = neighbour_lists.as_ptr();
            std::mem::forget(neighbour_lists);
            let answer = Vec_api::<Neighbourhood_api> {
                len : nb_vec as i64,
                ptr : neighbour_lists_ptr,
            };
            Box::into_raw(Box::new(answer))
        }  // end of parallel_search_neighbours_f32 for HnswApif32
    )
);

#[allow(unused_macros)]
macro_rules! generate_file_dump(
    ($function_name:ident, $api_name:ty, $type_val:ty) => (
        /// dump the graph to a file
        /// # Safety
        /// The function is unsafe because it dereferences a raw pointer
        ///
    #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $function_name(hnsw_api : *const $api_name, namelen : usize, filename :*const u8) -> i64 {
            log::info!("receiving request for file dump");
            let slice = unsafe { std::slice::from_raw_parts(filename, namelen) } ;
            let fstring  = String::from_utf8_lossy(slice).into_owned();
            let res =  unsafe { (*hnsw_api).opaque.file_dump(&PathBuf::from("."), &fstring) } ;
            if res.is_ok() {
                return 1;
            }
            else { return -1; }
        } // end of function_name
    )
);

//======= Reload stuff

#[allow(unused_macros)]
macro_rules! generate_loadhnsw(
    ($function_name:ident, $api_name:ty, $type_val:ty, $type_dist : ty) => (
        /// function to reload from a previous dump (knowing data type and distance used).
        /// This function takes as argument a pointer to Hnswio_api that drives the reloading.
        /// The pointer is provided by the function [get_hnswio()](get_hnswio).
        /// # Safety
        /// The function is unsafe because it dereferences a raw pointer
        ///
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $function_name(hnswio_c : *mut HnswIo)  -> *const $api_name {
            //
            unsafe {
            let hnsw_loaded_res = (*hnswio_c).load_hnsw::<$type_val, $type_dist>();

            if let Ok(hnsw_loaded) = hnsw_loaded_res {
                let api = <$api_name>::new(Box::new(hnsw_loaded));
                return Box::into_raw(Box::new(api));
            }
            else {
                warn!("an error occured, could not reload data from {:?}", (*hnswio_c).get_basename());
                return ptr::null();
            }
        }
        }  // end of load_hnswdump_
     )
);

// here we must generate as many function as there are couples (type, distance) to be accessed from our needs in Julia

// f32
generate_loadhnsw!(
    load_hnswdump_f32_DistL1,
    HnswApif32,
    f32,
    anndists::dist::distances::DistL1
);
generate_loadhnsw!(
    load_hnswdump_f32_DistL2,
    HnswApif32,
    f32,
    anndists::dist::distances::DistL2
);
generate_loadhnsw!(
    load_hnswdump_f32_DistCosine,
    HnswApif32,
    f32,
    anndists::dist::distances::DistCosine
);
generate_loadhnsw!(
    load_hnswdump_f32_DistDot,
    HnswApif32,
    f32,
    anndists::dist::distances::DistDot
);
generate_loadhnsw!(
    load_hnswdump_f32_DistJensenShannon,
    HnswApif32,
    f32,
    anndists::dist::distances::DistJensenShannon
);
generate_loadhnsw!(
    load_hnswdump_f32_DistJeffreys,
    HnswApif32,
    f32,
    anndists::dist::distances::DistJeffreys
);

// i32
generate_loadhnsw!(
    load_hnswdump_i32_DistL1,
    HnswApii32,
    i32,
    anndists::dist::distances::DistL1
);
generate_loadhnsw!(
    load_hnswdump_i32_DistL2,
    HnswApii32,
    i32,
    anndists::dist::distances::DistL2
);
generate_loadhnsw!(
    load_hnswdump_i32_DistHamming,
    HnswApii32,
    i32,
    anndists::dist::distances::DistHamming
);

// u32
generate_loadhnsw!(
    load_hnswdump_u32_DistL1,
    HnswApiu32,
    u32,
    anndists::dist::distances::DistL1
);
generate_loadhnsw!(
    load_hnswdump_u32_DistL2,
    HnswApiu32,
    u32,
    anndists::dist::distances::DistL2
);
generate_loadhnsw!(
    load_hnswdump_u32_DistHamming,
    HnswApiu32,
    u32,
    anndists::dist::distances::DistHamming
);
generate_loadhnsw!(
    load_hnswdump_u32_DistJaccard,
    HnswApiu32,
    u32,
    anndists::dist::distances::DistJaccard
);

// u16
generate_loadhnsw!(
    load_hnswdump_u16_DistL1,
    HnswApiu16,
    u16,
    anndists::dist::distances::DistL1
);
generate_loadhnsw!(
    load_hnswdump_u16_DistL2,
    HnswApiu16,
    u16,
    anndists::dist::distances::DistL2
);
generate_loadhnsw!(
    load_hnswdump_u16_DistHamming,
    HnswApiu16,
    u16,
    anndists::dist::distances::DistHamming
);
generate_loadhnsw!(
    load_hnswdump_u16_DistLevenshtein,
    HnswApiu16,
    u16,
    anndists::dist::distances::DistLevenshtein
);

// u8
generate_loadhnsw!(
    load_hnswdump_u8_DistL1,
    HnswApiu8,
    u8,
    anndists::dist::distances::DistL1
);
generate_loadhnsw!(
    load_hnswdump_u8_DistL2,
    HnswApiu8,
    u8,
    anndists::dist::distances::DistL2
);
generate_loadhnsw!(
    load_hnswdump_u8_DistHamming,
    HnswApiu8,
    u8,
    anndists::dist::distances::DistHamming
);
generate_loadhnsw!(
    load_hnswdump_u8_DistJaccard,
    HnswApiu8,
    u8,
    anndists::dist::distances::DistJaccard
);

// Reload only graph
generate_loadhnsw!(
    load_hnswdump_NoData_DistNoDist,
    HnswApiNodata,
    NoData,
    anndists::dist::NoDist
);

//=============== implementation for i32
/// # Safety
/// The function is unsafe because it dereferences a raw pointer
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn init_hnsw_f32(
    max_nb_conn: usize,
    ef_const: usize,
    namelen: usize,
    cdistname: *const u8,
) -> *const HnswApif32 {
    info!("entering init_hnsw_f32");
    let slice = unsafe { std::slice::from_raw_parts(cdistname, namelen) };
    let dname = String::from_utf8_lossy(slice).into_owned();
    // map distname to sthg. This whole block will go to a macro
    match dname.as_str() {
        "DistL1" => {
            info!(" received DistL1");
            let h = Hnsw::<f32, DistL1>::new(max_nb_conn, 10000, 16, ef_const, DistL1 {});
            let api = HnswApif32 {
                opaque: Box::new(h),
            };
            Box::into_raw(Box::new(api))
        }
        "DistL2" => {
            let h = Hnsw::<f32, DistL2>::new(max_nb_conn, 10000, 16, ef_const, DistL2 {});
            let api = HnswApif32 {
                opaque: Box::new(h),
            };
            Box::into_raw(Box::new(api))
        }
        "DistDot" => {
            let h = Hnsw::<f32, DistDot>::new(max_nb_conn, 10000, 16, ef_const, DistDot {});
            let api = HnswApif32 {
                opaque: Box::new(h),
            };
            Box::into_raw(Box::new(api))
        }
        "DistHellinger" => {
            let h =
                Hnsw::<f32, DistHellinger>::new(max_nb_conn, 10000, 16, ef_const, DistHellinger {});
            let api = HnswApif32 {
                opaque: Box::new(h),
            };
            Box::into_raw(Box::new(api))
        }
        "DistJeffreys" => {
            let h =
                Hnsw::<f32, DistJeffreys>::new(max_nb_conn, 10000, 16, ef_const, DistJeffreys {});
            let api = HnswApif32 {
                opaque: Box::new(h),
            };
            Box::into_raw(Box::new(api))
        }
        "DistJensenShannon" => {
            let h = Hnsw::<f32, DistJensenShannon>::new(
                max_nb_conn,
                10000,
                16,
                ef_const,
                DistJensenShannon {},
            );
            let api = HnswApif32 {
                opaque: Box::new(h),
            };
            Box::into_raw(Box::new(api))
        }
        _ => {
            warn!("init_hnsw_f32 received unknow distance {:?} ", dname);
            ptr::null::<HnswApif32>()
        }
    } // znd match
} // end of init_hnsw_f32

/// same as max_layer with different arguments, we pass max_elements and max_layer
/// # Safety
/// This function is unsafe because it dereferences raw pointers.
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn new_hnsw_f32(
    max_nb_conn: usize,
    ef_const: usize,
    namelen: usize,
    cdistname: *const u8,
    max_elements: usize,
    max_layer: usize,
) -> *const HnswApif32 {
    debug!("entering new_hnsw_f32");
    let slice = unsafe { std::slice::from_raw_parts(cdistname, namelen) };
    let dname = String::from_utf8_lossy(slice);
    // map distname to sthg. This whole block will go to a macro
    match dname.as_ref() {
        "DistL1" => {
            info!(" received DistL1");
            let h =
                Hnsw::<f32, DistL1>::new(max_nb_conn, max_elements, max_layer, ef_const, DistL1 {});
            let api = HnswApif32 {
                opaque: Box::new(h),
            };
            Box::into_raw(Box::new(api))
        }
        "DistL2" => {
            let h =
                Hnsw::<f32, DistL2>::new(max_nb_conn, max_elements, max_layer, ef_const, DistL2 {});
            let api = HnswApif32 {
                opaque: Box::new(h),
            };
            Box::into_raw(Box::new(api))
        }
        "DistDot" => {
            let h = Hnsw::<f32, DistDot>::new(
                max_nb_conn,
                max_elements,
                max_layer,
                ef_const,
                DistDot {},
            );
            let api = HnswApif32 {
                opaque: Box::new(h),
            };
            Box::into_raw(Box::new(api))
        }
        "DistHellinger" => {
            let h = Hnsw::<f32, DistHellinger>::new(
                max_nb_conn,
                max_elements,
                max_layer,
                ef_const,
                DistHellinger {},
            );
            let api = HnswApif32 {
                opaque: Box::new(h),
            };
            Box::into_raw(Box::new(api))
        }
        "DistJeffreys" => {
            let h = Hnsw::<f32, DistJeffreys>::new(
                max_nb_conn,
                max_elements,
                max_layer,
                ef_const,
                DistJeffreys {},
            );
            let api = HnswApif32 {
                opaque: Box::new(h),
            };
            Box::into_raw(Box::new(api))
        }
        "DistJensenShannon" => {
            let h = Hnsw::<f32, DistJensenShannon>::new(
                max_nb_conn,
                max_elements,
                max_layer,
                ef_const,
                DistJensenShannon {},
            );
            let api = HnswApif32 {
                opaque: Box::new(h),
            };
            Box::into_raw(Box::new(api))
        }
        _ => {
            warn!("init_hnsw_f32 received unknow distance {:?} ", dname);
            ptr::null::<HnswApif32>()
        }
    } // znd match
    //
} // end of new_hnsw_f32

/// # Safety
/// This function is unsafe because it dereferences raw pointers.
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn drop_hnsw_f32(p: *const HnswApif32) {
    unsafe {
        let _raw = Box::from_raw(p as *mut HnswApif32);
    }
}

/// # Safety
/// This function is unsafe because it dereferences raw pointers.
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn drop_hnsw_u16(p: *const HnswApiu16) {
    unsafe {
        let _raw = Box::from_raw(p as *mut HnswApiu16);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn init_hnsw_ptrdist_f32(
    max_nb_conn: usize,
    ef_const: usize,
    c_func: extern "C" fn(*const f32, *const f32, c_ulonglong) -> f32,
) -> *const HnswApif32 {
    info!("init_ hnsw_ptrdist: ptr  {:?}", c_func);
    let c_dist = DistCFFI::<f32>::new(c_func);
    let h = Hnsw::<f32, DistCFFI<f32>>::new(max_nb_conn, 10000, 16, ef_const, c_dist);
    let api = HnswApif32 {
        opaque: Box::new(h),
    };
    Box::into_raw(Box::new(api))
}

/// # Safety
/// This function is unsafe because it dereferences raw pointers.
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn insert_f32(
    hnsw_api: *mut HnswApif32,
    len: usize,
    data: *const f32,
    id: usize,
) {
    trace!("entering insert_f32, vec len is {:?}, id : {:?} ", len, id);
    //  construct vector: Rust clones and take ownership.
    let data_v: Vec<f32>;
    unsafe {
        let slice = std::slice::from_raw_parts(data, len);
        data_v = Vec::from(slice);
        //    debug!("calling insert data");
        (*hnsw_api).opaque.insert_data(&data_v, id);
    }
    // trace!("exiting insert_f32");
} // end of insert_f32

/// # Safety
/// This function is unsafe because it dereferences raw pointers.
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn parallel_insert_f32(
    hnsw_api: *mut HnswApif32,
    nb_vec: usize,
    vec_len: usize,
    datas: *mut *const f32,
    ids: *const usize,
) {
    //
    // debug!("entering parallel_insert_f32 , vec len is {:?}, nb_vec : {:?}", vec_len, nb_vec);
    let data_ids: Vec<usize>;
    let data_ptrs: Vec<*const f32>;
    unsafe {
        let slice = std::slice::from_raw_parts(ids, nb_vec);
        data_ids = Vec::from(slice);
    }
    unsafe {
        let slice = std::slice::from_raw_parts(datas, nb_vec);
        data_ptrs = Vec::from(slice);
    }
    // debug!("got data ptrs");
    let mut data_v = Vec::<Vec<f32>>::with_capacity(nb_vec);
    #[allow(clippy::needless_range_loop)]
    for i in 0..nb_vec {
        unsafe {
            let slice = std::slice::from_raw_parts(data_ptrs[i], vec_len);
            let v = Vec::from(slice);
            data_v.push(v);
        }
    }
    // debug!("sending request");
    let mut request: Vec<(&Vec<f32>, usize)> = Vec::with_capacity(nb_vec);
    for i in 0..nb_vec {
        request.push((&data_v[i], data_ids[i]));
    }
    //
    unsafe {
        (*hnsw_api).opaque.parallel_insert_data(&request);
    };
    trace!("exiting parallel_insert");
} // end of parallel_insert_f32

/// # Safety
/// This function is unsafe because it dereferences raw pointers.
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn search_neighbours_f32(
    hnsw_api: *const HnswApif32,
    len: usize,
    data: *const f32,
    knbn: usize,
    ef_search: usize,
) -> *const Neighbourhood_api {
    //
    trace!(
        "entering search_neighbours , vec len is {:?}, id : {:?} ef_search {:?}",
        len, knbn, ef_search
    );
    let data_v: Vec<f32>;
    let neighbours: Vec<Neighbour>;
    unsafe {
        let slice = std::slice::from_raw_parts(data, len);
        data_v = Vec::from(slice);
        trace!("calling search neighbours {:?}", data_v);
        neighbours = (*hnsw_api)
            .opaque
            .search_neighbours(&data_v, knbn, ef_search);
    }
    let neighbours_api: Vec<Neighbour_api> = neighbours.iter().map(Neighbour_api::from).collect();
    trace!(" got nb neighbours {:?}", neighbours_api.len());
    // for i in 0..neighbours_api.len() {
    //    println!(" id {:?}  dist : {:?} ", neighbours_api[i].id, neighbours_api[i].d);
    // }
    let nbgh_i = neighbours.len() as i64;
    let neighbours_ptr = neighbours_api.as_ptr();
    std::mem::forget(neighbours_api);
    let answer = Neighbourhood_api {
        nbgh: nbgh_i,
        neighbours: neighbours_ptr,
    };
    trace!(
        "search_neighbours returning nb neighbours {:?} id ptr {:?} ",
        nbgh_i, neighbours_ptr
    );
    Box::into_raw(Box::new(answer))
}
// end of search_neighbours for HnswApif32

generate_parallel_search_neighbours!(parallel_search_neighbours_f32, HnswApif32, f32);
generate_file_dump!(file_dump_f32, HnswApif32, f32);

//=============== implementation for i32

/// # Safety
/// This function is unsafe because it dereferences raw pointers.
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn init_hnsw_i32(
    max_nb_conn: usize,
    ef_const: usize,
    namelen: usize,
    cdistname: *const u8,
) -> *const HnswApii32 {
    info!("entering init_hnsw_i32");
    let slice = unsafe { std::slice::from_raw_parts(cdistname, namelen) };
    let dname = String::from_utf8_lossy(slice);
    // map distname to sthing. This whole block will go to a macro
    if dname == "DistL1" {
        info!(" received DistL1");
        let h = Hnsw::<i32, DistL1>::new(max_nb_conn, 10000, 16, ef_const, DistL1 {});
        let api = HnswApii32 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistL2" {
        let h = Hnsw::<i32, DistL2>::new(max_nb_conn, 10000, 16, ef_const, DistL2 {});
        let api = HnswApii32 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistHamming" {
        let h = Hnsw::<i32, DistHamming>::new(max_nb_conn, 10000, 16, ef_const, DistHamming {});
        let api = HnswApii32 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    }
    ptr::null::<HnswApii32>()
} // end of init_hnsw_i32

#[unsafe(no_mangle)]
pub extern "C" fn init_hnsw_ptrdist_i32(
    max_nb_conn: usize,
    ef_const: usize,
    c_func: extern "C" fn(*const i32, *const i32, c_ulonglong) -> f32,
) -> *const HnswApii32 {
    debug!("init_ hnsw_ptrdist: ptr  {:?}", c_func);
    let c_dist = DistCFFI::<i32>::new(c_func);
    let h = Hnsw::<i32, DistCFFI<i32>>::new(max_nb_conn, 10000, 16, ef_const, c_dist);
    let api = HnswApii32 {
        opaque: Box::new(h),
    };
    Box::into_raw(Box::new(api))
}

//==generation of function for i32

super::declare_myapi_type!(HnswApii32, i32);

generate_insert!(insert_i32, HnswApii32, i32);
generate_parallel_insert!(parallel_insert_i32, HnswApii32, i32);
generate_search_neighbours!(search_neighbours_i32, HnswApii32, i32);
generate_parallel_search_neighbours!(parallel_search_neighbours_i32, HnswApii32, i32);
generate_file_dump!(file_dump_i32, HnswApii32, i32);

//========== generation for u32

/// # Safety
/// This function is unsafe because it dereferences raw pointers.
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn init_hnsw_u32(
    max_nb_conn: usize,
    ef_const: usize,
    namelen: usize,
    cdistname: *const u8,
) -> *const HnswApiu32 {
    debug!("Entering init_hnsw_u32");
    let slice = unsafe { std::slice::from_raw_parts(cdistname, namelen) };
    let dname = String::from_utf8_lossy(slice);
    // map distname to sthg. This whole block will go to a macro
    if dname == "DistL1" {
        debug!("Received DistL1");
        let h = Hnsw::<u32, DistL1>::new(max_nb_conn, 10000, 16, ef_const, DistL1 {});
        let api = HnswApiu32 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistL2" {
        let h = Hnsw::<u32, DistL2>::new(max_nb_conn, 10000, 16, ef_const, DistL2 {});
        let api = HnswApiu32 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistJaccard" {
        let h = Hnsw::<u32, DistJaccard>::new(max_nb_conn, 10000, 16, ef_const, DistJaccard {});
        let api = HnswApiu32 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistHamming" {
        let h = Hnsw::<u32, DistHamming>::new(max_nb_conn, 10000, 16, ef_const, DistHamming {});
        let api = HnswApiu32 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    }
    //
    ptr::null::<HnswApiu32>()
} // end of init_hnsw_i32

#[unsafe(no_mangle)]
pub extern "C" fn init_hnsw_ptrdist_u32(
    max_nb_conn: usize,
    ef_const: usize,
    c_func: extern "C" fn(*const u32, *const u32, c_ulonglong) -> f32,
) -> *const HnswApiu32 {
    info!("init_ hnsw_ptrdist: ptr  {:?}", c_func);
    let c_dist = DistCFFI::<u32>::new(c_func);
    let h = Hnsw::<u32, DistCFFI<u32>>::new(max_nb_conn, 10000, 16, ef_const, c_dist);
    let api = HnswApiu32 {
        opaque: Box::new(h),
    };
    Box::into_raw(Box::new(api))
}

super::declare_myapi_type!(HnswApiu32, u32);

generate_insert!(insert_u32, HnswApiu32, u32);
generate_parallel_insert!(parallel_insert_u32, HnswApiu32, u32);
generate_search_neighbours!(search_neighbours_u32, HnswApiu32, u32);
generate_parallel_search_neighbours!(parallel_search_neighbours_u32, HnswApiu32, u32);
generate_file_dump!(file_dump_u32, HnswApiu32, u32);

//============== generation of function for u16 =====================

super::declare_myapi_type!(HnswApiu16, u16);

/// # Safety
/// This function is unsafe because it dereferences raw pointers.
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn init_hnsw_u16(
    max_nb_conn: usize,
    ef_const: usize,
    namelen: usize,
    cdistname: *const u8,
) -> *const HnswApiu16 {
    info!("entering init_hnsw_u16");
    let slice = unsafe { std::slice::from_raw_parts(cdistname, namelen) };
    let dname = String::from_utf8_lossy(slice);
    // map distname to sthg. This whole block will go to a macro
    if dname == "DistL1" {
        info!(" received DistL1");
        let h = Hnsw::<u16, DistL1>::new(max_nb_conn, 10000, 16, ef_const, DistL1 {});
        let api = HnswApiu16 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistL2" {
        let h = Hnsw::<u16, DistL2>::new(max_nb_conn, 10000, 16, ef_const, DistL2 {});
        let api = HnswApiu16 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistHamming" {
        let h = Hnsw::<u16, DistHamming>::new(max_nb_conn, 10000, 16, ef_const, DistHamming {});
        let api = HnswApiu16 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistJaccard" {
        let h = Hnsw::<u16, DistJaccard>::new(max_nb_conn, 10000, 16, ef_const, DistJaccard {});
        let api = HnswApiu16 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistLevenshtein" {
        let h =
            Hnsw::<u16, DistLevenshtein>::new(max_nb_conn, 10000, 16, ef_const, DistLevenshtein {});
        let api = HnswApiu16 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    }
    ptr::null::<HnswApiu16>()
} // end of init_hnsw_u16

/// # Safety
/// This function is unsafe because it dereferences raw pointers.
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn new_hnsw_u16(
    max_nb_conn: usize,
    ef_const: usize,
    namelen: usize,
    cdistname: *const u8,
    max_elements: usize,
    max_layer: usize,
) -> *const HnswApiu16 {
    info!("entering init_hnsw_u16");
    let slice = unsafe { std::slice::from_raw_parts(cdistname, namelen) };
    let dname = String::from_utf8_lossy(slice);
    // map distname to sthg. This whole block will go to a macro
    if dname == "DistL1" {
        info!(" received DistL1");
        let h = Hnsw::<u16, DistL1>::new(max_nb_conn, max_elements, max_layer, ef_const, DistL1 {});
        let api = HnswApiu16 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistL2" {
        let h = Hnsw::<u16, DistL2>::new(max_nb_conn, max_elements, max_layer, ef_const, DistL2 {});
        let api = HnswApiu16 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistHamming" {
        let h = Hnsw::<u16, DistHamming>::new(
            max_nb_conn,
            max_elements,
            max_layer,
            ef_const,
            DistHamming {},
        );
        let api = HnswApiu16 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistJaccard" {
        let h = Hnsw::<u16, DistJaccard>::new(
            max_nb_conn,
            max_elements,
            max_layer,
            ef_const,
            DistJaccard {},
        );
        let api = HnswApiu16 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistLevenshtein" {
        let h = Hnsw::<u16, DistLevenshtein>::new(
            max_nb_conn,
            max_elements,
            max_layer,
            ef_const,
            DistLevenshtein {},
        );
        let api = HnswApiu16 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    }
    ptr::null::<HnswApiu16>()
} // end of init_hnsw_u16

#[unsafe(no_mangle)]
pub extern "C" fn init_hnsw_ptrdist_u16(
    max_nb_conn: usize,
    ef_const: usize,
    c_func: extern "C" fn(*const u16, *const u16, c_ulonglong) -> f32,
) -> *const HnswApiu16 {
    info!("init_ hnsw_ptrdist: ptr  {:?}", c_func);
    let c_dist = DistCFFI::<u16>::new(c_func);
    let h = Hnsw::<u16, DistCFFI<u16>>::new(max_nb_conn, 10000, 16, ef_const, c_dist);
    let api = HnswApiu16 {
        opaque: Box::new(h),
    };
    Box::into_raw(Box::new(api))
}

generate_insert!(insert_u16, HnswApiu16, u16);
generate_parallel_insert!(parallel_insert_u16, HnswApiu16, u16);
generate_search_neighbours!(search_neighbours_u16, HnswApiu16, u16);
generate_parallel_search_neighbours!(parallel_search_neighbours_u16, HnswApiu16, u16);
generate_file_dump!(file_dump_u16, HnswApiu16, u16);

//============== generation of function for u8 =====================

super::declare_myapi_type!(HnswApiu8, u8);

/// # Safety
/// This function is unsafe because it dereferences raw pointers.
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn init_hnsw_u8(
    max_nb_conn: usize,
    ef_const: usize,
    namelen: usize,
    cdistname: *const u8,
) -> *const HnswApiu8 {
    debug!("entering init_hnsw_u8");
    let slice = unsafe { std::slice::from_raw_parts(cdistname, namelen) };
    let dname = String::from_utf8_lossy(slice);
    // map distname to sthg. This whole block will go to a macro
    if dname == "DistL1" {
        info!(" received DistL1");
        let h = Hnsw::<u8, DistL1>::new(max_nb_conn, 10000, 16, ef_const, DistL1 {});
        let api = HnswApiu8 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistL2" {
        let h = Hnsw::<u8, DistL2>::new(max_nb_conn, 10000, 16, ef_const, DistL2 {});
        let api = HnswApiu8 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistHamming" {
        let h = Hnsw::<u8, DistHamming>::new(max_nb_conn, 10000, 16, ef_const, DistHamming {});
        let api = HnswApiu8 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    } else if dname == "DistJaccard" {
        let h = Hnsw::<u8, DistJaccard>::new(max_nb_conn, 10000, 16, ef_const, DistJaccard {});
        let api = HnswApiu8 {
            opaque: Box::new(h),
        };
        return Box::into_raw(Box::new(api));
    }
    ptr::null::<HnswApiu8>()
} // end of init_hnsw_u16

#[unsafe(no_mangle)]
pub extern "C" fn init_hnsw_ptrdist_u8(
    max_nb_conn: usize,
    ef_const: usize,
    c_func: extern "C" fn(*const u8, *const u8, c_ulonglong) -> f32,
) -> *const HnswApiu8 {
    info!("init_ hnsw_ptrdist: ptr  {:?}", c_func);
    let c_dist = DistCFFI::<u8>::new(c_func);
    let h = Hnsw::<u8, DistCFFI<u8>>::new(max_nb_conn, 10000, 16, ef_const, c_dist);
    let api = HnswApiu8 {
        opaque: Box::new(h),
    };
    Box::into_raw(Box::new(api))
}

generate_insert!(insert_u8, HnswApiu8, u8);
generate_parallel_insert!(parallel_insert_u8, HnswApiu8, u8);
generate_search_neighbours!(search_neighbours_u8, HnswApiu8, u8);
generate_parallel_search_neighbours!(parallel_search_neighbours_u8, HnswApiu8, u8);
generate_file_dump!(file_dump_u8, HnswApiu8, u8);

//=========================== dump restore functions

/// This structure provides a light description of the graph to be passed to C compatible languages.
#[repr(C)]
pub struct DescriptionFFI {
    ///  value is 1 for Full 0 for Light
    pub dumpmode: u8,
    /// max number of connections in layers != 0
    pub max_nb_connection: u8,
    /// number of observed layers
    pub nb_layer: u8,
    /// search parameter
    pub ef: usize,
    /// total number of points
    pub nb_point: usize,
    /// dimension of data vector
    pub data_dimension: usize,
    /// length and pointer on dist name
    pub distname_len: usize,
    pub distname: *const u8,
    /// T typename
    pub t_name_len: usize,
    pub t_name: *const u8,
}

impl Default for DescriptionFFI {
    fn default() -> Self {
        Self::new()
    }
}

impl DescriptionFFI {
    pub fn new() -> Self {
        DescriptionFFI {
            dumpmode: 0,
            max_nb_connection: 0,
            nb_layer: 0,
            ef: 0,
            nb_point: 0,
            data_dimension: 0,
            distname_len: 0,
            distname: ptr::null(),
            t_name_len: 0,
            t_name: ptr::null(),
        }
    } // end of new
}

/// returns a const pointer to a DescriptionFFI from a dump file, given filename length and pointer (*const u8)
/// # Safety
/// This function is unsafe because it dereferences raw pointers.
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn load_hnsw_description(
    flen: usize,
    name: *const u8,
) -> *const DescriptionFFI {
    // opens file
    let slice = unsafe { std::slice::from_raw_parts(name, flen) };
    let filename = String::from_utf8_lossy(slice).into_owned();
    let fpath = PathBuf::from(filename);
    let fileres = OpenOptions::new().read(true).open(&fpath);
    //
    let mut ffi_description = DescriptionFFI::new();
    match fileres {
        Ok(file) => {
            //
            let mut bufr = BufReader::with_capacity(10000000, file);
            let res = load_description(&mut bufr);
            if let Ok(description) = res {
                let distname = String::clone(&description.distname);
                let distname_ptr = distname.as_ptr();
                let distname_len = distname.len();
                std::mem::forget(distname);

                let t_name = String::clone(&description.t_name);
                let t_name_ptr = t_name.as_ptr();
                let t_name_len = t_name.len();
                std::mem::forget(t_name);

                ffi_description.dumpmode = 1; // CAVEAT
                ffi_description.max_nb_connection = description.max_nb_connection;
                ffi_description.nb_layer = description.nb_layer;
                ffi_description.ef = description.ef;
                ffi_description.data_dimension = description.dimension;
                ffi_description.distname_len = distname_len;
                ffi_description.distname = distname_ptr;
                ffi_description.t_name_len = t_name_len;
                ffi_description.t_name = t_name_ptr;
                Box::into_raw(Box::new(ffi_description))
            } else {
                error!(
                    "could not get descrption of hnsw from file {:?}",
                    fpath.as_os_str()
                );
                println!(
                    "could not get descrption of hnsw from file {:?} ",
                    fpath.as_os_str()
                );
                ptr::null()
            }
        }
        Err(_e) => {
            error!(
                "no such file, load_hnsw_description: could not open file {:?}",
                fpath.as_os_str()
            );
            println!(
                "no such file, load_hnsw_description: could not open file {:?}",
                fpath.as_os_str()
            );
            ptr::null()
        }
    }
} // end of load_hnsw_description

//============ log initialization ============//

/// to initialize rust logging from Julia
#[unsafe(no_mangle)]
pub extern "C" fn init_rust_log() {
    let _res = env_logger::Builder::from_default_env().try_init();
}
