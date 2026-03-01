- version 0.3.3
  small fix on filter (thanks to VillSnow). include ndarray 0.17 as possible dep. fixed compiler warning on elided lifetimes

- version 0.3.2
  update dependencies to ndarray 0.16 , rand 0.9  indexmap 2.9, hdf5. edition=2024

- version 0.3.1
 
  Possibility to reduce the number of levels used Hnsw structure with the function hnsw::modify_level_scale.
  This often increases significantly recall while incurring a moderate cpu cost. It is also possible
  to have same recall with smaller *max_nb_conn* parameters so reducing memory usage.  
  See README.md at [bigann](https://github.com/jean-pierreBoth/bigann).  
  Modification inspired by the article by [Munyampirwa](https://arxiv.org/abs/2412.01940)

  Clippy cleaning and minor arguments change (PathBuf to Path String to &str) in dump/reload
  with the help of bwsw (https://github.com/bwsw)


- **version 0.3.0**:
 
   The distances implementation is now in a separate crate [anndsits](https://crates.io/crates/anndists). Using hnsw_rs::prelude:::*   should make the change transparent. 
   
   The mmap implementation makes it possible to use the [coreset](https://github.com/jean-pierreBoth/coreset) crate to compute coreset and clusters of data stored in hnsw dumps.

- version 0.2.1:
  
  when using mmap, the points less frequently used (points in lower layers) are preferentially mmap-ed while upper layers are preferentially 
  explcitly read from file.

  Hnswio is now Sync.

  feature stdsimd, based on std::simd, runs with nightly on Hamming with u32,u64 and DisL1,DistL2, DistDot with f32
  
- The **version 0.2** introduces 
    1. possibility to use mmap on the data file storing  the vectors represented in the hnsw structure. This is mostly usefule for
    large vectors, where data needs more space than the graph part.
    As a consequence the format of this file changed. Old format can be read but new dumps will be in the new format.  
    In case of mmap usage, a dump after inserting new elements must ensure that the old file is not overwritten, so a unique file name is
  generated if necessary. See documentation of module Hnswio

    1. the filtering trait
  
   
-  Upgrade of many dependencies. Change from simple_logger to env_logger. The logger is initialized one for all in file src/lib.rs and cannot be intialized twice. The level of log can be modulated by the RUST_LOG env variable on a module basis or switched off. See the *env_logger* crate doc.
  
- A rust crate *edlib_rs* provides an interface to the *excellent* edlib C++ library  [(Cf edlib)](https://github.com/Martinsos/edlib) can be found at [edlib_rs](https://github.com/jean-pierreBoth/edlib-rs) or on crate.io. It can be used to define a user adhoc distance on &[u8] with normal, prefix or infix mode (which is useful in genomics alignment).
  
- The library do not depend anymore on hdf5 and ndarray. They are dev-dependancies needed for examples, this simplify compatibility issues.
- Added insertion methods for slices for easier use with the ndarray crate.
  
- simd/avx2 requires now the feature "simdeez_f". So by default the crate can compile on M1 chip and transitions to std::simd.
  
- Added DistPtr and possiblity to dump/reload with this distance type. (See *load_hnsw_with_dist* function)
  
- Implementation of Hamming for f64 exclusively in the context SuperMinHash in crate [probminhash](https://crates.io/crates/probminhash)

