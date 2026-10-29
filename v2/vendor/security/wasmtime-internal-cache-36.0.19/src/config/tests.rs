use super::CacheConfig;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

// note: config loading during validation creates cache directory to canonicalize its path,
//       that's why these function and macro always use custom cache directory
// note: tempdir removes directory when being dropped, so we need to return it to the caller,
//       so the paths are valid
pub fn test_prolog() -> (TempDir, PathBuf, PathBuf) {
    let _ = env_logger::try_init();
    let temp_dir = tempfile::tempdir().expect("Can't create temporary directory");
    let cache_dir = temp_dir.path().join("cache-dir");
    let config_path = temp_dir.path().join("cache-config.toml");
    (temp_dir, cache_dir, config_path)
}

macro_rules! load_config {
    ($config_path:ident, $content_fmt:expr, $cache_dir:ident) => {{
        let config_path = &$config_path;
        let content = format!($content_fmt, cache_dir = $cache_dir.display());
        fs::write(config_path, content).expect("Failed to write test config file");
        CacheConfig::from_file(Some(config_path)).unwrap()
    }};
}

macro_rules! bad_config {
    ($config_path:ident, $content_fmt:expr, $cache_dir:ident) => {{
        let config_path = &$config_path;
        let content = format!($content_fmt, cache_dir = $cache_dir.display());
        fs::write(config_path, content).expect("Failed to write test config file");
        assert!(CacheConfig::from_file(Some(config_path)).is_err());
    }};
}

#[test]
fn test_unrecognized_settings() {
    let (_td, cd, cp) = test_prolog();
    bad_config!(
        cp,
        "unrecognized-setting = 42\n\
         [cache]\n\
         directory = '{cache_dir}'",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         unrecognized-setting = 42",
        cd
    );
}

#[test]
fn test_all_settings() {
    let (_td, cd, cp) = test_prolog();
    let conf = load_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size = '16'\n\
         baseline-compression-level = 3\n\
         optimized-compression-level = 20\n\
         optimized-compression-usage-counter-threshold = '256'\n\
         cleanup-interval = '1h'\n\
         optimizing-compression-task-timeout = '30m'\n\
         allowed-clock-drift-for-files-from-future = '1d'\n\
         file-count-soft-limit = '65536'\n\
         files-total-size-soft-limit = '512Mi'\n\
         file-count-limit-percent-if-deleting = '70%'\n\
         files-total-size-limit-percent-if-deleting = '70%'",
        cd
    );
    check_conf(&conf, &cd);

    let conf = load_config!(
        cp,
        // added some white spaces
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size  = ' 16\t'\n\
         baseline-compression-level = 3\n\
         optimized-compression-level =\t 20\n\
         optimized-compression-usage-counter-threshold = '256'\n\
         cleanup-interval = ' 1h'\n\
         optimizing-compression-task-timeout = '30  m'\n\
         allowed-clock-drift-for-files-from-future = '1\td'\n\
         file-count-soft-limit = '\t \t65536\t'\n\
         files-total-size-soft-limit = '512\t\t Mi '\n\
         file-count-limit-percent-if-deleting = '70\t%'\n\
         files-total-size-limit-percent-if-deleting = ' 70 %'",
        cd
    );
    check_conf(&conf, &cd);

    fn check_conf(conf: &CacheConfig, cd: &PathBuf) {
        assert_eq!(
            conf.directory(),
            Some(&fs::canonicalize(cd).expect("canonicalize failed"))
        );
        assert_eq!(conf.worker_event_queue_size(), 0x10);
        assert_eq!(conf.baseline_compression_level(), 3);
        assert_eq!(conf.optimized_compression_level(), 20);
        assert_eq!(conf.optimized_compression_usage_counter_threshold(), 0x100);
        assert_eq!(conf.cleanup_interval(), Duration::from_secs(60 * 60));
        assert_eq!(
            conf.optimizing_compression_task_timeout(),
            Duration::from_secs(30 * 60)
        );
        assert_eq!(
            conf.allowed_clock_drift_for_files_from_future(),
            Duration::from_secs(60 * 60 * 24)
        );
        assert_eq!(conf.file_count_soft_limit(), 0x10_000);
        assert_eq!(conf.files_total_size_soft_limit(), 512 * (1u64 << 20));
        assert_eq!(conf.file_count_limit_percent_if_deleting(), 70);
        assert_eq!(conf.files_total_size_limit_percent_if_deleting(), 70);
    }
}

#[test]
fn test_compression_level_settings() {
    let (_td, cd, cp) = test_prolog();
    let conf = load_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         baseline-compression-level = 1\n\
         optimized-compression-level = 21",
        cd
    );
    assert_eq!(conf.baseline_compression_level(), 1);
    assert_eq!(conf.optimized_compression_level(), 21);

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         baseline-compression-level = -1\n\
         optimized-compression-level = 21",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         baseline-compression-level = 15\n\
         optimized-compression-level = 10",
        cd
    );
}

#[test]
fn test_si_prefix_settings() {
    let (_td, cd, cp) = test_prolog();
    let conf = load_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size = '42'\n\
         optimized-compression-usage-counter-threshold = '4K'\n\
         file-count-soft-limit = '3M'",
        cd
    );
    assert_eq!(conf.worker_event_queue_size(), 42);
    assert_eq!(conf.optimized_compression_usage_counter_threshold(), 4_000);
    assert_eq!(conf.file_count_soft_limit(), 3_000_000);

    let conf = load_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size = '2K'\n\
         optimized-compression-usage-counter-threshold = '4444T'\n\
         file-count-soft-limit = '1P'",
        cd
    );
    assert_eq!(conf.worker_event_queue_size(), 2_000);
    assert_eq!(
        conf.optimized_compression_usage_counter_threshold(),
        4_444_000_000_000_000
    );
    assert_eq!(conf.file_count_soft_limit(), 1_000_000_000_000_000);

    // different errors
    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size = '2g'",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         file-count-soft-limit = 1",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         file-count-soft-limit = '-31337'",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         file-count-soft-limit = '3.14M'",
        cd
    );
}

#[test]
fn test_disk_space_settings() {
    let (_td, cd, cp) = test_prolog();
    let conf = load_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-soft-limit = '76'",
        cd
    );
    assert_eq!(conf.files_total_size_soft_limit(), 76);

    let conf = load_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-soft-limit = '42 Mi'",
        cd
    );
    assert_eq!(conf.files_total_size_soft_limit(), 42 * (1u64 << 20));

    let conf = load_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-soft-limit = '2 Gi'",
        cd
    );
    assert_eq!(conf.files_total_size_soft_limit(), 2 * (1u64 << 30));

    let conf = load_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-soft-limit = '31337 Ti'",
        cd
    );
    assert_eq!(conf.files_total_size_soft_limit(), 31337 * (1u64 << 40));

    let conf = load_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-soft-limit = '7 Pi'",
        cd
    );
    assert_eq!(conf.files_total_size_soft_limit(), 7 * (1u64 << 50));

    let conf = load_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-soft-limit = '7M'",
        cd
    );
    assert_eq!(conf.files_total_size_soft_limit(), 7_000_000);

    // different errors
    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-soft-limit = '7 mi'",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-soft-limit = 1",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-soft-limit = '-31337'",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-soft-limit = '3.14Ki'",
        cd
    );
}

#[test]
fn test_duration_settings() {
    let (_td, cd, cp) = test_prolog();
    let conf = load_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         cleanup-interval = '100s'\n\
         optimizing-compression-task-timeout = '3m'\n\
         allowed-clock-drift-for-files-from-future = '4h'",
        cd
    );
    assert_eq!(conf.cleanup_interval(), Duration::from_secs(100));
    assert_eq!(
        conf.optimizing_compression_task_timeout(),
        Duration::from_secs(3 * 60)
    );
    assert_eq!(
        conf.allowed_clock_drift_for_files_from_future(),
        Duration::from_secs(4 * 60 * 60)
    );

    let conf = load_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         cleanup-interval = '2d'\n\
         optimizing-compression-task-timeout = '333 m'",
        cd
    );
    assert_eq!(
        conf.cleanup_interval(),
        Duration::from_secs(2 * 24 * 60 * 60)
    );
    assert_eq!(
        conf.optimizing_compression_task_timeout(),
        Duration::from_secs(333 * 60)
    );

    // different errors
    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         optimizing-compression-task-timeout = '333'",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         optimizing-compression-task-timeout = 333",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         optimizing-compression-task-timeout = '10 M'",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         optimizing-compression-task-timeout = '10 min'",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         optimizing-compression-task-timeout = '-10s'",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         optimizing-compression-task-timeout = '1.5m'",
        cd
    );
}

#[test]
fn test_percent_settings() {
    let (_td, cd, cp) = test_prolog();
    let conf = load_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         file-count-limit-percent-if-deleting = '62%'\n\
         files-total-size-limit-percent-if-deleting = '23 %'",
        cd
    );
    assert_eq!(conf.file_count_limit_percent_if_deleting(), 62);
    assert_eq!(conf.files_total_size_limit_percent_if_deleting(), 23);

    // different errors
    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-limit-percent-if-deleting = '23'",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-limit-percent-if-deleting = '22.5%'",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-limit-percent-if-deleting = '0.5'",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-limit-percent-if-deleting = '-1%'",
        cd
    );

    bad_config!(
        cp,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         files-total-size-limit-percent-if-deleting = '101%'",
        cd
    );
}

/// Default builder produces a disabled cache configuration with the same defaults.
#[test]
fn test_builder_default() {
    let (_td, _cd, cp) = test_prolog();
    let config_content = "[cache]\n";
    fs::write(&cp, config_content).expect("Failed to write test config file");
    let expected_config = CacheConfig::from_file(Some(&cp)).unwrap();

    let mut config = CacheConfig::new();
    config
        .validate()
        .expect("Failed to validate default config");

    assert_eq!(config.directory, expected_config.directory);
    assert_eq!(
        config.worker_event_queue_size,
        expected_config.worker_event_queue_size
    );
    assert_eq!(
        config.baseline_compression_level,
        expected_config.baseline_compression_level
    );
    assert_eq!(
        config.optimized_compression_level,
        expected_config.optimized_compression_level
    );
    assert_eq!(
        config.optimized_compression_usage_counter_threshold,
        expected_config.optimized_compression_usage_counter_threshold
    );
    assert_eq!(config.cleanup_interval, expected_config.cleanup_interval);
    assert_eq!(
        config.optimizing_compression_task_timeout,
        expected_config.optimizing_compression_task_timeout
    );
    assert_eq!(
        config.allowed_clock_drift_for_files_from_future,
        expected_config.allowed_clock_drift_for_files_from_future
    );
    assert_eq!(
        config.file_count_soft_limit,
        expected_config.file_count_soft_limit
    );
    assert_eq!(
        config.files_total_size_soft_limit,
        expected_config.files_total_size_soft_limit
    );
    assert_eq!(
        config.file_count_limit_percent_if_deleting,
        expected_config.file_count_limit_percent_if_deleting
    );
    assert_eq!(
        config.files_total_size_limit_percent_if_deleting,
        expected_config.files_total_size_limit_percent_if_deleting
    );
}

#[test]
fn test_builder_all_settings() {
    let (_td, cd, _cp) = test_prolog();

    let mut conf = CacheConfig::new();
    conf.with_directory(&cd)
        .with_worker_event_queue_size(0x10)
        .with_baseline_compression_level(3)
        .with_optimized_compression_level(20)
        .with_optimized_compression_usage_counter_threshold(0x100)
        .with_cleanup_interval(Duration::from_secs(60 * 60))
        .with_optimizing_compression_task_timeout(Duration::from_secs(30 * 60))
        .with_allowed_clock_drift_for_files_from_future(Duration::from_secs(60 * 60 * 24))
        .with_file_count_soft_limit(0x10_000)
        .with_files_total_size_soft_limit(512 * (1u64 << 20))
        .with_file_count_limit_percent_if_deleting(70)
        .with_files_total_size_limit_percent_if_deleting(70);
    conf.validate().expect("validation failed");
    check_conf(&conf, &cd);

    fn check_conf(conf: &CacheConfig, cd: &PathBuf) {
        assert_eq!(
            conf.directory(),
            Some(&fs::canonicalize(cd).expect("canonicalize failed"))
        );
        assert_eq!(conf.worker_event_queue_size(), 0x10);
        assert_eq!(conf.baseline_compression_level(), 3);
        assert_eq!(conf.optimized_compression_level(), 20);
        assert_eq!(conf.optimized_compression_usage_counter_threshold(), 0x100);
        assert_eq!(conf.cleanup_interval(), Duration::from_secs(60 * 60));
        assert_eq!(
            conf.optimizing_compression_task_timeout(),
            Duration::from_secs(30 * 60)
        );
        assert_eq!(
            conf.allowed_clock_drift_for_files_from_future(),
            Duration::from_secs(60 * 60 * 24)
        );
        assert_eq!(conf.file_count_soft_limit(), 0x10_000);
        assert_eq!(conf.files_total_size_soft_limit(), 512 * (1u64 << 20));
        assert_eq!(conf.file_count_limit_percent_if_deleting(), 70);
        assert_eq!(conf.files_total_size_limit_percent_if_deleting(), 70);
    }
}
