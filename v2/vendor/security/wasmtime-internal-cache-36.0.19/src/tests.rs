use super::config::tests::test_prolog;
use super::*;

// Since cache system is a global thing, each test needs to be run in separate process.
// So, init() tests are run as integration tests.
// However, caching is a private thing, an implementation detail, and needs to be tested
// from the inside of the module.
// We test init() in exactly one test, rest of the tests doesn't rely on it.

#[test]
fn test_cache_init() {
    let (_tempdir, cache_dir, config_path) = test_prolog();
    let baseline_compression_level = 4;
    let config_content = format!(
        "[cache]\n\
         directory = '{}'\n\
         baseline-compression-level = {}\n",
        cache_dir.display(),
        baseline_compression_level,
    );
    fs::write(&config_path, config_content).expect("Failed to write test config file");

    let cache_config = CacheConfig::from_file(Some(&config_path)).unwrap();

    // assumption: config init creates cache directory and returns canonicalized path
    assert_eq!(
        cache_config.directory(),
        Some(&fs::canonicalize(cache_dir).unwrap())
    );
    assert_eq!(
        cache_config.baseline_compression_level(),
        baseline_compression_level
    );

    // test if we can use worker
    Cache::new(cache_config)
        .unwrap()
        .worker()
        .on_cache_update_async(config_path);
}

#[test]
fn test_write_read_cache() {
    let (_tempdir, cache_dir, config_path) = test_prolog();
    let cache_config = load_config!(
        config_path,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         baseline-compression-level = 3\n",
        cache_dir
    );
    let cache = Cache::new(cache_config.clone()).unwrap();

    // assumption: config load creates cache directory and returns canonicalized path
    assert_eq!(
        cache_config.directory(),
        Some(&fs::canonicalize(cache_dir).unwrap())
    );

    let compiler1 = "test-1";
    let compiler2 = "test-2";

    let entry1 = ModuleCacheEntry::from_inner(ModuleCacheEntryInner::new(compiler1, &cache));
    let entry2 = ModuleCacheEntry::from_inner(ModuleCacheEntryInner::new(compiler2, &cache));

    entry1.get_data::<_, i32, i32>(1, |_| Ok(100)).unwrap();
    entry1.get_data::<_, i32, i32>(1, |_| panic!()).unwrap();

    entry1.get_data::<_, i32, i32>(2, |_| Ok(100)).unwrap();
    entry1.get_data::<_, i32, i32>(1, |_| panic!()).unwrap();
    entry1.get_data::<_, i32, i32>(2, |_| panic!()).unwrap();

    entry1.get_data::<_, i32, i32>(3, |_| Ok(100)).unwrap();
    entry1.get_data::<_, i32, i32>(1, |_| panic!()).unwrap();
    entry1.get_data::<_, i32, i32>(2, |_| panic!()).unwrap();
    entry1.get_data::<_, i32, i32>(3, |_| panic!()).unwrap();

    entry1.get_data::<_, i32, i32>(4, |_| Ok(100)).unwrap();
    entry1.get_data::<_, i32, i32>(1, |_| panic!()).unwrap();
    entry1.get_data::<_, i32, i32>(2, |_| panic!()).unwrap();
    entry1.get_data::<_, i32, i32>(3, |_| panic!()).unwrap();
    entry1.get_data::<_, i32, i32>(4, |_| panic!()).unwrap();

    entry2.get_data::<_, i32, i32>(1, |_| Ok(100)).unwrap();
    entry1.get_data::<_, i32, i32>(1, |_| panic!()).unwrap();
    entry1.get_data::<_, i32, i32>(2, |_| panic!()).unwrap();
    entry1.get_data::<_, i32, i32>(3, |_| panic!()).unwrap();
    entry1.get_data::<_, i32, i32>(4, |_| panic!()).unwrap();
    entry2.get_data::<_, i32, i32>(1, |_| panic!()).unwrap();
}
