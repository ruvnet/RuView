use super::*;
use crate::config::tests::test_prolog;
use std::iter::repeat;
use std::process;
// load_config! comes from crate::cache(::config::tests);

// when doing anything with the tests, make sure they are DETERMINISTIC
// -- the result shouldn't rely on system time!
pub mod system_time_stub;

#[test]
fn test_on_get_create_stats_file() {
    let (_tempdir, cache_dir, config_path) = test_prolog();
    let cache_config = load_config!(
        config_path,
        "[cache]\n\
         directory = '{cache_dir}'",
        cache_dir
    );
    let worker = Worker::start_new(&cache_config);

    let mod_file = cache_dir.join("some-mod");
    worker.on_cache_get_async(mod_file);
    worker.wait_for_all_events_handled();
    assert_eq!(worker.events_dropped(), 0);

    let stats_file = cache_dir.join("some-mod.stats");
    let stats = read_stats_file(&stats_file).expect("Failed to read stats file");
    assert_eq!(stats.usages, 1);
    assert_eq!(
        stats.compression_level,
        cache_config.baseline_compression_level()
    );
}

#[test]
fn test_on_get_update_usage_counter() {
    let (_tempdir, cache_dir, config_path) = test_prolog();
    let cache_config = load_config!(
        config_path,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size = '16'",
        cache_dir
    );
    let worker = Worker::start_new(&cache_config);

    let mod_file = cache_dir.join("some-mod");
    let stats_file = cache_dir.join("some-mod.stats");
    let default_stats = ModuleCacheStatistics::default(&cache_config);
    assert!(write_stats_file(&stats_file, &default_stats));

    let mut usages = 0;
    for times_used in &[4, 7, 2] {
        for _ in 0..*times_used {
            worker.on_cache_get_async(mod_file.clone());
            usages += 1;
        }

        worker.wait_for_all_events_handled();
        assert_eq!(worker.events_dropped(), 0);

        let stats = read_stats_file(&stats_file).expect("Failed to read stats file");
        assert_eq!(stats.usages, usages);
    }
}

#[test]
fn test_on_get_recompress_no_mod_file() {
    let (_tempdir, cache_dir, config_path) = test_prolog();
    let cache_config = load_config!(
        config_path,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size = '16'\n\
         baseline-compression-level = 3\n\
         optimized-compression-level = 7\n\
         optimized-compression-usage-counter-threshold = '256'",
        cache_dir
    );
    let worker = Worker::start_new(&cache_config);

    let mod_file = cache_dir.join("some-mod");
    let stats_file = cache_dir.join("some-mod.stats");
    let mut start_stats = ModuleCacheStatistics::default(&cache_config);
    start_stats.usages = 250;
    assert!(write_stats_file(&stats_file, &start_stats));

    let mut usages = start_stats.usages;
    for times_used in &[4, 7, 2] {
        for _ in 0..*times_used {
            worker.on_cache_get_async(mod_file.clone());
            usages += 1;
        }

        worker.wait_for_all_events_handled();
        assert_eq!(worker.events_dropped(), 0);

        let stats = read_stats_file(&stats_file).expect("Failed to read stats file");
        assert_eq!(stats.usages, usages);
        assert_eq!(
            stats.compression_level,
            cache_config.baseline_compression_level()
        );
    }
}

#[test]
fn test_on_get_recompress_with_mod_file() {
    let (_tempdir, cache_dir, config_path) = test_prolog();
    let cache_config = load_config!(
        config_path,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size = '16'\n\
         baseline-compression-level = 3\n\
         optimized-compression-level = 7\n\
         optimized-compression-usage-counter-threshold = '256'",
        cache_dir
    );
    let worker = Worker::start_new(&cache_config);

    let mod_file = cache_dir.join("some-mod");
    let mod_data = "some test data to be compressed";
    let data = zstd::encode_all(
        mod_data.as_bytes(),
        cache_config.baseline_compression_level(),
    )
    .expect("Failed to compress sample mod file");
    fs::write(&mod_file, &data).expect("Failed to write sample mod file");

    let stats_file = cache_dir.join("some-mod.stats");
    let mut start_stats = ModuleCacheStatistics::default(&cache_config);
    start_stats.usages = 250;
    assert!(write_stats_file(&stats_file, &start_stats));

    // scenarios:
    // 1. Shouldn't be recompressed
    // 2. Should be recompressed
    // 3. After lowering compression level, should be recompressed
    let scenarios = [(4, false), (7, true), (2, false)];

    let mut usages = start_stats.usages;
    assert!(usages < cache_config.optimized_compression_usage_counter_threshold());
    let mut tested_higher_opt_compr_lvl = false;
    for (times_used, lower_compr_lvl) in &scenarios {
        for _ in 0..*times_used {
            worker.on_cache_get_async(mod_file.clone());
            usages += 1;
        }

        worker.wait_for_all_events_handled();
        assert_eq!(worker.events_dropped(), 0);

        let mut stats = read_stats_file(&stats_file).expect("Failed to read stats file");
        assert_eq!(stats.usages, usages);
        assert_eq!(
            stats.compression_level,
            if usages < cache_config.optimized_compression_usage_counter_threshold() {
                cache_config.baseline_compression_level()
            } else {
                cache_config.optimized_compression_level()
            }
        );
        let compressed_data = fs::read(&mod_file).expect("Failed to read mod file");
        let decoded_data =
            zstd::decode_all(&compressed_data[..]).expect("Failed to decompress mod file");
        assert_eq!(decoded_data, mod_data.as_bytes());

        if *lower_compr_lvl {
            assert!(usages >= cache_config.optimized_compression_usage_counter_threshold());
            tested_higher_opt_compr_lvl = true;
            stats.compression_level -= 1;
            assert!(write_stats_file(&stats_file, &stats));
        }
    }
    assert!(usages >= cache_config.optimized_compression_usage_counter_threshold());
    assert!(tested_higher_opt_compr_lvl);
}

#[test]
fn test_on_get_recompress_lock() {
    let (_tempdir, cache_dir, config_path) = test_prolog();
    let cache_config = load_config!(
        config_path,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size = '16'\n\
         baseline-compression-level = 3\n\
         optimized-compression-level = 7\n\
         optimized-compression-usage-counter-threshold = '256'\n\
         optimizing-compression-task-timeout = '30m'\n\
         allowed-clock-drift-for-files-from-future = '1d'",
        cache_dir
    );
    let worker = Worker::start_new(&cache_config);

    let mod_file = cache_dir.join("some-mod");
    let mod_data = "some test data to be compressed";
    let data = zstd::encode_all(
        mod_data.as_bytes(),
        cache_config.baseline_compression_level(),
    )
    .expect("Failed to compress sample mod file");
    fs::write(&mod_file, &data).expect("Failed to write sample mod file");

    let stats_file = cache_dir.join("some-mod.stats");
    let mut start_stats = ModuleCacheStatistics::default(&cache_config);
    start_stats.usages = 255;

    let lock_file = cache_dir.join("some-mod.wip-lock");

    let scenarios = [
        // valid lock
        (true, "past", Duration::from_secs(30 * 60 - 1)),
        // valid future lock
        (true, "future", Duration::from_secs(24 * 60 * 60)),
        // expired lock
        (false, "past", Duration::from_secs(30 * 60)),
        // expired future lock
        (false, "future", Duration::from_secs(24 * 60 * 60 + 1)),
    ];

    for (lock_valid, duration_sign, duration) in &scenarios {
        assert!(write_stats_file(&stats_file, &start_stats)); // restore usage & compression level
        create_file_with_mtime(&lock_file, "", duration_sign, &duration);

        worker.on_cache_get_async(mod_file.clone());
        worker.wait_for_all_events_handled();
        assert_eq!(worker.events_dropped(), 0);

        let stats = read_stats_file(&stats_file).expect("Failed to read stats file");
        assert_eq!(stats.usages, start_stats.usages + 1);
        assert_eq!(
            stats.compression_level,
            if *lock_valid {
                cache_config.baseline_compression_level()
            } else {
                cache_config.optimized_compression_level()
            }
        );
        let compressed_data = fs::read(&mod_file).expect("Failed to read mod file");
        let decoded_data =
            zstd::decode_all(&compressed_data[..]).expect("Failed to decompress mod file");
        assert_eq!(decoded_data, mod_data.as_bytes());
    }
}

#[test]
fn test_on_update_fresh_stats_file() {
    let (_tempdir, cache_dir, config_path) = test_prolog();
    let cache_config = load_config!(
        config_path,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size = '16'\n\
         baseline-compression-level = 3\n\
         optimized-compression-level = 7\n\
         cleanup-interval = '1h'",
        cache_dir
    );
    let worker = Worker::start_new(&cache_config);

    let mod_file = cache_dir.join("some-mod");
    let stats_file = cache_dir.join("some-mod.stats");
    let cleanup_certificate = cache_dir.join(".cleanup.wip-done");
    create_file_with_mtime(&cleanup_certificate, "", "future", &Duration::from_secs(0));
    // the below created by the worker if it cleans up
    let worker_lock_file = cache_dir.join(format!(".cleanup.wip-{}", process::id()));

    // scenarios:
    // 1. Create new stats file
    // 2. Overwrite existing file
    for update_file in &[true, false] {
        worker.on_cache_update_async(mod_file.clone());
        worker.wait_for_all_events_handled();
        assert_eq!(worker.events_dropped(), 0);

        let mut stats = read_stats_file(&stats_file).expect("Failed to read stats file");
        assert_eq!(stats.usages, 1);
        assert_eq!(
            stats.compression_level,
            cache_config.baseline_compression_level()
        );

        if *update_file {
            stats.usages += 42;
            stats.compression_level += 1;
            assert!(write_stats_file(&stats_file, &stats));
        }

        assert!(!worker_lock_file.exists());
    }
}

#[test]
fn test_on_update_cleanup_limits_trash_locks() {
    let (_tempdir, cache_dir, config_path) = test_prolog();
    let cache_config = load_config!(
        config_path,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size = '16'\n\
         cleanup-interval = '30m'\n\
         optimizing-compression-task-timeout = '30m'\n\
         allowed-clock-drift-for-files-from-future = '1d'\n\
         file-count-soft-limit = '5'\n\
         files-total-size-soft-limit = '30K'\n\
         file-count-limit-percent-if-deleting = '70%'\n\
         files-total-size-limit-percent-if-deleting = '70%'
         ",
        cache_dir
    );
    let worker = Worker::start_new(&cache_config);
    let content_1k = "a".repeat(1_000);
    let content_10k = "a".repeat(10_000);

    let mods_files_dir = cache_dir.join("target-triple").join("compiler-version");
    let mod_with_stats = mods_files_dir.join("mod-with-stats");
    let trash_dirs = [
        mods_files_dir.join("trash"),
        mods_files_dir.join("trash").join("trash"),
    ];
    let trash_files = [
        cache_dir.join("trash-file"),
        cache_dir.join("trash-file.wip-lock"),
        cache_dir.join("target-triple").join("trash.txt"),
        cache_dir.join("target-triple").join("trash.txt.wip-lock"),
        mods_files_dir.join("trash.ogg"),
        mods_files_dir.join("trash").join("trash.doc"),
        mods_files_dir.join("trash").join("trash.doc.wip-lock"),
        mods_files_dir.join("trash").join("trash").join("trash.xls"),
        mods_files_dir
            .join("trash")
            .join("trash")
            .join("trash.xls.wip-lock"),
    ];
    let mod_locks = [
        // valid lock
        (
            mods_files_dir.join("mod0.wip-lock"),
            true,
            "past",
            Duration::from_secs(30 * 60 - 1),
        ),
        // valid future lock
        (
            mods_files_dir.join("mod1.wip-lock"),
            true,
            "future",
            Duration::from_secs(24 * 60 * 60),
        ),
        // expired lock
        (
            mods_files_dir.join("mod2.wip-lock"),
            false,
            "past",
            Duration::from_secs(30 * 60),
        ),
        // expired future lock
        (
            mods_files_dir.join("mod3.wip-lock"),
            false,
            "future",
            Duration::from_secs(24 * 60 * 60 + 1),
        ),
    ];
    // the below created by the worker if it cleans up
    let worker_lock_file = cache_dir.join(format!(".cleanup.wip-{}", process::id()));

    let scenarios = [
        // Close to limits, but not reached, only trash deleted
        (2, 2, 4),
        // File count limit exceeded
        (1, 10, 3),
        // Total size limit exceeded
        (4, 0, 2),
        // Both limits exceeded
        (3, 5, 3),
    ];

    for (files_10k, files_1k, remaining_files) in &scenarios {
        let mut secs_ago = 100;

        for d in &trash_dirs {
            fs::create_dir_all(d).expect("Failed to create directories");
        }
        for f in &trash_files {
            create_file_with_mtime(f, "", "past", &Duration::from_secs(0));
        }
        for (f, _, sign, duration) in &mod_locks {
            create_file_with_mtime(f, "", sign, &duration);
        }

        let mut mods_paths = vec![];
        for content in repeat(&content_10k)
            .take(*files_10k)
            .chain(repeat(&content_1k).take(*files_1k))
        {
            mods_paths.push(mods_files_dir.join(format!("test-mod-{}", mods_paths.len())));
            create_file_with_mtime(
                mods_paths.last().unwrap(),
                content,
                "past",
                &Duration::from_secs(secs_ago),
            );
            assert!(secs_ago > 0);
            secs_ago -= 1;
        }

        // creating .stats file updates mtime what affects test results
        // so we use a separate nonexistent module here (orphaned .stats will be removed anyway)
        worker.on_cache_update_async(mod_with_stats.clone());
        worker.wait_for_all_events_handled();
        assert_eq!(worker.events_dropped(), 0);

        for ent in trash_dirs.iter().chain(trash_files.iter()) {
            assert!(!ent.exists());
        }
        for (f, valid, ..) in &mod_locks {
            assert_eq!(f.exists(), *valid);
        }
        for (idx, path) in mods_paths.iter().enumerate() {
            let should_exist = idx >= mods_paths.len() - *remaining_files;
            assert_eq!(path.exists(), should_exist);
            if should_exist {
                // cleanup before next iteration
                fs::remove_file(path).expect("Failed to remove a file");
            }
        }
        fs::remove_file(&worker_lock_file).expect("Failed to remove lock file");
    }
}

#[test]
fn test_on_update_cleanup_lru_policy() {
    let (_tempdir, cache_dir, config_path) = test_prolog();
    let cache_config = load_config!(
        config_path,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size = '16'\n\
         file-count-soft-limit = '5'\n\
         files-total-size-soft-limit = '30K'\n\
         file-count-limit-percent-if-deleting = '80%'\n\
         files-total-size-limit-percent-if-deleting = '70%'",
        cache_dir
    );
    let worker = Worker::start_new(&cache_config);
    let content_1k = "a".repeat(1_000);
    let content_5k = "a".repeat(5_000);
    let content_10k = "a".repeat(10_000);

    let mods_files_dir = cache_dir.join("target-triple").join("compiler-version");
    fs::create_dir_all(&mods_files_dir).expect("Failed to create directories");
    let nonexistent_mod_file = cache_dir.join("nonexistent-mod");
    let orphaned_stats_file = cache_dir.join("orphaned-mod.stats");
    let worker_lock_file = cache_dir.join(format!(".cleanup.wip-{}", process::id()));

    // content, how long ago created, how long ago stats created (if created), should be alive
    let scenarios = [
        &[
            (&content_10k, 29, None, false),
            (&content_10k, 28, None, false),
            (&content_10k, 27, None, false),
            (&content_1k, 26, None, true),
            (&content_10k, 25, None, true),
            (&content_1k, 24, None, true),
        ],
        &[
            (&content_10k, 29, None, false),
            (&content_10k, 28, None, false),
            (&content_10k, 27, None, true),
            (&content_1k, 26, None, true),
            (&content_5k, 25, None, true),
            (&content_1k, 24, None, true),
        ],
        &[
            (&content_10k, 29, Some(19), true),
            (&content_10k, 28, None, false),
            (&content_10k, 27, None, false),
            (&content_1k, 26, Some(18), true),
            (&content_5k, 25, None, true),
            (&content_1k, 24, None, true),
        ],
        &[
            (&content_10k, 29, Some(19), true),
            (&content_10k, 28, Some(18), true),
            (&content_10k, 27, None, false),
            (&content_1k, 26, Some(17), true),
            (&content_5k, 25, None, false),
            (&content_1k, 24, None, false),
        ],
        &[
            (&content_10k, 29, Some(19), true),
            (&content_10k, 28, None, false),
            (&content_1k, 27, None, false),
            (&content_5k, 26, Some(18), true),
            (&content_1k, 25, None, false),
            (&content_10k, 24, None, false),
        ],
    ];

    for mods in &scenarios {
        let filenames = (0..mods.len())
            .map(|i| {
                (
                    mods_files_dir.join(format!("mod-{i}")),
                    mods_files_dir.join(format!("mod-{i}.stats")),
                )
            })
            .collect::<Vec<_>>();

        for ((content, mod_secs_ago, create_stats, _), (mod_filename, stats_filename)) in
            mods.iter().zip(filenames.iter())
        {
            create_file_with_mtime(
                mod_filename,
                content,
                "past",
                &Duration::from_secs(*mod_secs_ago),
            );
            if let Some(stats_secs_ago) = create_stats {
                create_file_with_mtime(
                    stats_filename,
                    "cleanup doesn't care",
                    "past",
                    &Duration::from_secs(*stats_secs_ago),
                );
            }
        }
        create_file_with_mtime(
            &orphaned_stats_file,
            "cleanup doesn't care",
            "past",
            &Duration::from_secs(0),
        );

        worker.on_cache_update_async(nonexistent_mod_file.clone());
        worker.wait_for_all_events_handled();
        assert_eq!(worker.events_dropped(), 0);

        assert!(!orphaned_stats_file.exists());
        for ((_, _, create_stats, alive), (mod_filename, stats_filename)) in
            mods.iter().zip(filenames.iter())
        {
            assert_eq!(mod_filename.exists(), *alive);
            assert_eq!(stats_filename.exists(), *alive && create_stats.is_some());

            // cleanup for next iteration
            if *alive {
                fs::remove_file(&mod_filename).expect("Failed to remove a file");
                if create_stats.is_some() {
                    fs::remove_file(&stats_filename).expect("Failed to remove a file");
                }
            }
        }

        fs::remove_file(&worker_lock_file).expect("Failed to remove lock file");
    }
}

// clock drift should be applied to mod cache & stats, too
// however, postpone deleting files to as late as possible
#[test]
fn test_on_update_cleanup_future_files() {
    let (_tempdir, cache_dir, config_path) = test_prolog();
    let cache_config = load_config!(
        config_path,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size = '16'\n\
         allowed-clock-drift-for-files-from-future = '1d'\n\
         file-count-soft-limit = '3'\n\
         files-total-size-soft-limit = '1M'\n\
         file-count-limit-percent-if-deleting = '70%'\n\
         files-total-size-limit-percent-if-deleting = '70%'",
        cache_dir
    );
    let worker = Worker::start_new(&cache_config);
    let content_1k = "a".repeat(1_000);

    let mods_files_dir = cache_dir.join("target-triple").join("compiler-version");
    fs::create_dir_all(&mods_files_dir).expect("Failed to create directories");
    let nonexistent_mod_file = cache_dir.join("nonexistent-mod");
    // the below created by the worker if it cleans up
    let worker_lock_file = cache_dir.join(format!(".cleanup.wip-{}", process::id()));

    let scenarios: [&[_]; 5] = [
        // NOT cleaning up, everything is ok
        &[
            (Duration::from_secs(0), None, true),
            (Duration::from_secs(24 * 60 * 60), None, true),
        ],
        // NOT cleaning up, everything is ok
        &[
            (Duration::from_secs(0), None, true),
            (Duration::from_secs(24 * 60 * 60 + 1), None, true),
        ],
        // cleaning up, removing files from oldest
        &[
            (Duration::from_secs(0), None, false),
            (Duration::from_secs(24 * 60 * 60), None, true),
            (Duration::from_secs(1), None, false),
            (Duration::from_secs(2), None, true),
        ],
        // cleaning up, removing files from oldest; deleting file from far future
        &[
            (Duration::from_secs(0), None, false),
            (Duration::from_secs(1), None, true),
            (Duration::from_secs(24 * 60 * 60 + 1), None, false),
            (Duration::from_secs(2), None, true),
        ],
        // cleaning up, removing files from oldest; file from far future should have .stats from +-now => it's a legitimate file
        &[
            (Duration::from_secs(0), None, false),
            (Duration::from_secs(1), None, false),
            (
                Duration::from_secs(24 * 60 * 60 + 1),
                Some(Duration::from_secs(3)),
                true,
            ),
            (Duration::from_secs(2), None, true),
        ],
    ];

    for mods in &scenarios {
        let filenames = (0..mods.len())
            .map(|i| {
                (
                    mods_files_dir.join(format!("mod-{i}")),
                    mods_files_dir.join(format!("mod-{i}.stats")),
                )
            })
            .collect::<Vec<_>>();

        for ((duration, opt_stats_duration, _), (mod_filename, stats_filename)) in
            mods.iter().zip(filenames.iter())
        {
            create_file_with_mtime(mod_filename, &content_1k, "future", duration);
            if let Some(stats_duration) = opt_stats_duration {
                create_file_with_mtime(stats_filename, "", "future", stats_duration);
            }
        }

        worker.on_cache_update_async(nonexistent_mod_file.clone());
        worker.wait_for_all_events_handled();
        assert_eq!(worker.events_dropped(), 0);

        for ((_, opt_stats_duration, alive), (mod_filename, stats_filename)) in
            mods.iter().zip(filenames.iter())
        {
            assert_eq!(mod_filename.exists(), *alive);
            assert_eq!(
                stats_filename.exists(),
                *alive && opt_stats_duration.is_some()
            );
            if *alive {
                fs::remove_file(mod_filename).expect("Failed to remove a file");
                if opt_stats_duration.is_some() {
                    fs::remove_file(stats_filename).expect("Failed to remove a file");
                }
            }
        }

        fs::remove_file(&worker_lock_file).expect("Failed to remove lock file");
    }
}

// this tests if worker triggered cleanup or not when some cleanup lock/certificate was out there
#[test]
fn test_on_update_cleanup_self_lock() {
    let (_tempdir, cache_dir, config_path) = test_prolog();
    let cache_config = load_config!(
        config_path,
        "[cache]\n\
         directory = '{cache_dir}'\n\
         worker-event-queue-size = '16'\n\
         cleanup-interval = '30m'\n\
         allowed-clock-drift-for-files-from-future = '1d'",
        cache_dir
    );
    let worker = Worker::start_new(&cache_config);

    let mod_file = cache_dir.join("some-mod");
    let trash_file = cache_dir.join("trash-file.txt");

    let lock_file = cache_dir.join(".cleanup.wip-lock");
    // the below created by the worker if it cleans up
    let worker_lock_file = cache_dir.join(format!(".cleanup.wip-{}", process::id()));

    let scenarios = [
        // valid lock
        (true, "past", Duration::from_secs(30 * 60 - 1)),
        // valid future lock
        (true, "future", Duration::from_secs(24 * 60 * 60)),
        // expired lock
        (false, "past", Duration::from_secs(30 * 60)),
        // expired future lock
        (false, "future", Duration::from_secs(24 * 60 * 60 + 1)),
    ];

    for (lock_valid, duration_sign, duration) in &scenarios {
        create_file_with_mtime(
            &trash_file,
            "with trash content",
            "future",
            &Duration::from_secs(0),
        );
        create_file_with_mtime(&lock_file, "", duration_sign, &duration);

        worker.on_cache_update_async(mod_file.clone());
        worker.wait_for_all_events_handled();
        assert_eq!(worker.events_dropped(), 0);

        assert_eq!(trash_file.exists(), *lock_valid);
        assert_eq!(lock_file.exists(), *lock_valid);
        if *lock_valid {
            assert!(!worker_lock_file.exists());
        } else {
            fs::remove_file(&worker_lock_file).expect("Failed to remove lock file");
        }
    }
}

fn create_file_with_mtime(filename: &Path, contents: &str, offset_sign: &str, offset: &Duration) {
    fs::write(filename, contents).expect("Failed to create a file");
    let mtime = match offset_sign {
        "past" => system_time_stub::NOW
            .checked_sub(*offset)
            .expect("Failed to calculate new mtime"),
        "future" => system_time_stub::NOW
            .checked_add(*offset)
            .expect("Failed to calculate new mtime"),
        _ => unreachable!(),
    };
    filetime::set_file_mtime(filename, mtime.into()).expect("Failed to set mtime");
}
