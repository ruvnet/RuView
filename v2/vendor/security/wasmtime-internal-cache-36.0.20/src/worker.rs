//! Background worker that watches over the cache.
//!
//! It cleans up old cache, updates statistics and optimizes the cache.
//! We allow losing some messages (it doesn't hurt) and some races,
//! but we guarantee eventual consistency and fault tolerancy.
//! Background tasks can be CPU intensive, but the worker thread has low priority.

#![cfg_attr(
    not(test),
    expect(
        clippy::useless_conversion,
        reason = "cfg(test) and cfg(not(test)) have a different definition \
                  of `SystemTime`, so conversions below are needed in \
                  one mode but not the other, just ignore the lint in this \
                  module in not(test) mode where the conversion isn't required",
    )
)]

use super::{CacheConfig, fs_write_atomic};
use log::{debug, info, trace, warn};
use serde_derive::{Deserialize, Serialize};
use std::cmp;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
#[cfg(test)]
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;
#[cfg(not(test))]
use std::time::SystemTime;
#[cfg(test)]
use tests::system_time_stub::SystemTimeStub as SystemTime;

#[derive(Clone)]
pub(super) struct Worker {
    sender: SyncSender<CacheEvent>,
    #[cfg(test)]
    stats: Arc<(Mutex<WorkerStats>, Condvar)>,
}

struct WorkerThread {
    receiver: Receiver<CacheEvent>,
    cache_config: CacheConfig,
    #[cfg(test)]
    stats: Arc<(Mutex<WorkerStats>, Condvar)>,
}

#[cfg(test)]
#[derive(Default)]
struct WorkerStats {
    dropped: u32,
    sent: u32,
    handled: u32,
}

#[derive(Debug, Clone)]
enum CacheEvent {
    OnCacheGet(PathBuf),
    OnCacheUpdate(PathBuf),
}

impl Worker {
    pub(super) fn start_new(cache_config: &CacheConfig) -> Self {
        let queue_size = match cache_config.worker_event_queue_size() {
            num if num <= usize::max_value() as u64 => num as usize,
            _ => usize::max_value(),
        };
        let (tx, rx) = sync_channel(queue_size);

        #[cfg(test)]
        let stats = Arc::new((Mutex::new(WorkerStats::default()), Condvar::new()));

        let worker_thread = WorkerThread {
            receiver: rx,
            cache_config: cache_config.clone(),
            #[cfg(test)]
            stats: stats.clone(),
        };

        // when self is dropped, sender will be dropped, what will cause the channel
        // to hang, and the worker thread to exit -- it happens in the tests
        // non-tests binary has only a static worker, so Rust doesn't drop it
        thread::spawn(move || worker_thread.run());

        Self {
            sender: tx,
            #[cfg(test)]
            stats,
        }
    }

    pub(super) fn on_cache_get_async(&self, path: impl AsRef<Path>) {
        let event = CacheEvent::OnCacheGet(path.as_ref().to_path_buf());
        self.send_cache_event(event);
    }

    pub(super) fn on_cache_update_async(&self, path: impl AsRef<Path>) {
        let event = CacheEvent::OnCacheUpdate(path.as_ref().to_path_buf());
        self.send_cache_event(event);
    }

    #[inline]
    fn send_cache_event(&self, event: CacheEvent) {
        let sent_event = self.sender.try_send(event.clone());

        if let Err(ref err) = sent_event {
            info!(
                "Failed to send asynchronously message to worker thread, \
                 event: {event:?}, error: {err}"
            );
        }

        #[cfg(test)]
        {
            let mut stats = self
                .stats
                .0
                .lock()
                .expect("Failed to acquire worker stats lock");

            if sent_event.is_ok() {
                stats.sent += 1;
            } else {
                stats.dropped += 1;
            }
        }
    }

    #[cfg(test)]
    pub(super) fn events_dropped(&self) -> u32 {
        let stats = self
            .stats
            .0
            .lock()
            .expect("Failed to acquire worker stats lock");
        stats.dropped
    }

    #[cfg(test)]
    pub(super) fn wait_for_all_events_handled(&self) {
        let (stats, condvar) = &*self.stats;
        let mut stats = stats.lock().expect("Failed to acquire worker stats lock");
        while stats.handled != stats.sent {
            stats = condvar
                .wait(stats)
                .expect("Failed to reacquire worker stats lock");
        }
    }
}

impl fmt::Debug for Worker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Worker").finish()
    }
}

#[derive(Serialize, Deserialize)]
struct ModuleCacheStatistics {
    pub usages: u64,
    #[serde(rename = "optimized-compression")]
    pub compression_level: i32,
}

impl ModuleCacheStatistics {
    fn default(cache_config: &CacheConfig) -> Self {
        Self {
            usages: 0,
            compression_level: cache_config.baseline_compression_level(),
        }
    }
}

enum CacheEntry {
    Recognized {
        path: PathBuf,
        mtime: SystemTime,
        size: u64,
    },
    Unrecognized {
        path: PathBuf,
        is_dir: bool,
    },
}

macro_rules! unwrap_or_warn {
    ($result:expr, $cont:stmt, $err_msg:expr, $path:expr) => {
        match $result {
            Ok(val) => val,
            Err(err) => {
                warn!("{}, path: {}, msg: {}", $err_msg, $path.display(), err);
                $cont
            }
        }
    };
}

impl WorkerThread {
    fn run(self) {
        debug!("Cache worker thread started.");

        Self::lower_thread_priority();

        #[cfg(test)]
        let (stats, condvar) = &*self.stats;

        for event in self.receiver.iter() {
            match event {
                CacheEvent::OnCacheGet(path) => self.handle_on_cache_get(path),
                CacheEvent::OnCacheUpdate(path) => self.handle_on_cache_update(path),
            }

            #[cfg(test)]
            {
                let mut stats = stats.lock().expect("Failed to acquire worker stats lock");
                stats.handled += 1;
                condvar.notify_all();
            }
        }
    }

    #[cfg(target_os = "fuchsia")]
    fn lower_thread_priority() {
        // TODO This needs to use Fuchsia thread profiles
        // https://fuchsia.dev/fuchsia-src/reference/kernel_objects/profile
        warn!(
            "Lowering thread priority on Fuchsia is currently a noop. It might affect application performance."
        );
    }

    #[cfg(target_os = "windows")]
    fn lower_thread_priority() {
        use windows_sys::Win32::System::Threading::*;

        // https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setthreadpriority
        // https://docs.microsoft.com/en-us/windows/win32/procthread/scheduling-priorities

        if unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) } == 0 {
            warn!(
                "Failed to lower worker thread priority. It might affect application performance."
            );
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "fuchsia")))]
    fn lower_thread_priority() {
        // http://man7.org/linux/man-pages/man7/sched.7.html

        const NICE_DELTA_FOR_BACKGROUND_TASKS: i32 = 3;

        match rustix::process::nice(NICE_DELTA_FOR_BACKGROUND_TASKS) {
            Ok(current_nice) => {
                debug!("New nice value of worker thread: {current_nice}");
            }
            Err(err) => {
                warn!(
                    "Failed to lower worker thread priority ({err:?}). It might affect application performance."
                );
            }
        };
    }

    /// Increases the usage counter and recompresses the file
    /// if the usage counter reached configurable threshold.
    fn handle_on_cache_get(&self, path: PathBuf) {
        trace!("handle_on_cache_get() for path: {}", path.display());

        // construct .stats file path
        let filename = path.file_name().unwrap().to_str().unwrap();
        let stats_path = path.with_file_name(format!("{filename}.stats"));

        // load .stats file (default if none or error)
        let mut stats = read_stats_file(stats_path.as_ref())
            .unwrap_or_else(|| ModuleCacheStatistics::default(&self.cache_config));

        // step 1: update the usage counter & write to the disk
        //         it's racy, but it's fine (the counter will be just smaller,
        //         sometimes will retrigger recompression)
        stats.usages += 1;
        if !write_stats_file(stats_path.as_ref(), &stats) {
            return;
        }

        // step 2: recompress if there's a need
        let opt_compr_lvl = self.cache_config.optimized_compression_level();
        if stats.compression_level >= opt_compr_lvl
            || stats.usages
                < self
                    .cache_config
                    .optimized_compression_usage_counter_threshold()
        {
            return;
        }

        let lock_path = if let Some(p) = acquire_task_fs_lock(
            path.as_ref(),
            self.cache_config.optimizing_compression_task_timeout(),
            self.cache_config
                .allowed_clock_drift_for_files_from_future(),
        ) {
            p
        } else {
            return;
        };

        trace!("Trying to recompress file: {}", path.display());

        // recompress, write to other file, rename (it's atomic file content exchange)
        // and update the stats file
        let compressed_cache_bytes = unwrap_or_warn!(
            fs::read(&path),
            return,
            "Failed to read old cache file",
            path
        );

        let cache_bytes = unwrap_or_warn!(
            zstd::decode_all(&compressed_cache_bytes[..]),
            return,
            "Failed to decompress cached code",
            path
        );

        let recompressed_cache_bytes = unwrap_or_warn!(
            zstd::encode_all(&cache_bytes[..], opt_compr_lvl),
            return,
            "Failed to compress cached code",
            path
        );

        unwrap_or_warn!(
            fs::write(&lock_path, &recompressed_cache_bytes),
            return,
            "Failed to write recompressed cache",
            lock_path
        );

        unwrap_or_warn!(
            fs::rename(&lock_path, &path),
            {
                if let Err(error) = fs::remove_file(&lock_path) {
                    warn!(
                        "Failed to clean up (remove) recompressed cache, path {}, err: {}",
                        lock_path.display(),
                        error
                    );
                }

                return;
            },
            "Failed to rename recompressed cache",
            lock_path
        );

        // update stats file (reload it! recompression can take some time)
        if let Some(mut new_stats) = read_stats_file(stats_path.as_ref()) {
            if new_stats.compression_level >= opt_compr_lvl {
                // Rare race:
                //    two instances with different opt_compr_lvl: we don't know in which order they updated
                //    the cache file and the stats file (they are not updated together atomically)
                // Possible solution is to use directories per cache entry, but it complicates the system
                // and is not worth it.
                debug!(
                    "DETECTED task did more than once (or race with new file): \
                     recompression of {}. Note: if optimized compression level setting \
                     has changed in the meantine, the stats file might contain \
                     inconsistent compression level due to race.",
                    path.display()
                );
            } else {
                new_stats.compression_level = opt_compr_lvl;
                let _ = write_stats_file(stats_path.as_ref(), &new_stats);
            }

            if new_stats.usages < stats.usages {
                debug!(
                    "DETECTED lower usage count (new file or race with counter \
                     increasing): file {}",
                    path.display()
                );
            }
        } else {
            debug!(
                "Can't read stats file again to update compression level (it might got \
                 cleaned up): file {}",
                stats_path.display()
            );
        }

        trace!("Task finished: recompress file: {}", path.display());
    }

    fn directory(&self) -> &PathBuf {
        self.cache_config
            .directory()
            .expect("CacheConfig should be validated before being passed to a WorkerThread")
    }

    fn handle_on_cache_update(&self, path: PathBuf) {
        trace!("handle_on_cache_update() for path: {}", path.display());

        // ---------------------- step 1: create .stats file

        // construct .stats file path
        let filename = path
            .file_name()
            .expect("Expected valid cache file name")
            .to_str()
            .expect("Expected valid cache file name");
        let stats_path = path.with_file_name(format!("{filename}.stats"));

        // create and write stats file
        let mut stats = ModuleCacheStatistics::default(&self.cache_config);
        stats.usages += 1;
        write_stats_file(&stats_path, &stats);

        // ---------------------- step 2: perform cleanup task if needed

        // acquire lock for cleanup task
        // Lock is a proof of recent cleanup task, so we don't want to delete them.
        // Expired locks will be deleted by the cleanup task.
        let cleanup_file = self.directory().join(".cleanup"); // some non existing marker file
        if acquire_task_fs_lock(
            &cleanup_file,
            self.cache_config.cleanup_interval(),
            self.cache_config
                .allowed_clock_drift_for_files_from_future(),
        )
        .is_none()
        {
            return;
        }

        trace!("Trying to clean up cache");

        let mut cache_index = self.list_cache_contents();
        let future_tolerance = SystemTime::now()
            .checked_add(
                self.cache_config
                    .allowed_clock_drift_for_files_from_future(),
            )
            .expect("Brace your cache, the next Big Bang is coming (time overflow)");
        cache_index.sort_unstable_by(|lhs, rhs| {
            // sort by age
            use CacheEntry::*;
            match (lhs, rhs) {
                (Recognized { mtime: lhs_mt, .. }, Recognized { mtime: rhs_mt, .. }) => {
                    match (*lhs_mt > future_tolerance, *rhs_mt > future_tolerance) {
                        // later == younger
                        (false, false) => rhs_mt.cmp(lhs_mt),
                        // files from far future are treated as oldest recognized files
                        // we want to delete them, so the cache keeps track of recent files
                        // however, we don't delete them uncodintionally,
                        // because .stats file can be overwritten with a meaningful mtime
                        (true, false) => cmp::Ordering::Greater,
                        (false, true) => cmp::Ordering::Less,
                        (true, true) => cmp::Ordering::Equal,
                    }
                }
                // unrecognized is kind of infinity
                (Recognized { .. }, Unrecognized { .. }) => cmp::Ordering::Less,
                (Unrecognized { .. }, Recognized { .. }) => cmp::Ordering::Greater,
                (Unrecognized { .. }, Unrecognized { .. }) => cmp::Ordering::Equal,
            }
        });

        // find "cut" boundary:
        // - remove unrecognized files anyway,
        // - remove some cache files if some quota has been exceeded
        let mut total_size = 0u64;
        let mut start_delete_idx = None;
        let mut start_delete_idx_if_deleting_recognized_items: Option<usize> = None;

        let total_size_limit = self.cache_config.files_total_size_soft_limit();
        let file_count_limit = self.cache_config.file_count_soft_limit();
        let tsl_if_deleting = total_size_limit
            .checked_mul(
                self.cache_config
                    .files_total_size_limit_percent_if_deleting() as u64,
            )
            .unwrap()
            / 100;
        let fcl_if_deleting = file_count_limit
            .checked_mul(self.cache_config.file_count_limit_percent_if_deleting() as u64)
            .unwrap()
            / 100;

        for (idx, item) in cache_index.iter().enumerate() {
            let size = if let CacheEntry::Recognized { size, .. } = item {
                size
            } else {
                start_delete_idx = Some(idx);
                break;
            };

            total_size += size;
            if start_delete_idx_if_deleting_recognized_items.is_none()
                && (total_size > tsl_if_deleting || (idx + 1) as u64 > fcl_if_deleting)
            {
                start_delete_idx_if_deleting_recognized_items = Some(idx);
            }

            if total_size > total_size_limit || (idx + 1) as u64 > file_count_limit {
                start_delete_idx = start_delete_idx_if_deleting_recognized_items;
                break;
            }
        }

        if let Some(idx) = start_delete_idx {
            for item in &cache_index[idx..] {
                let (result, path, entity) = match item {
                    CacheEntry::Recognized { path, .. }
                    | CacheEntry::Unrecognized {
                        path,
                        is_dir: false,
                    } => (fs::remove_file(path), path, "file"),
                    CacheEntry::Unrecognized { path, is_dir: true } => {
                        (fs::remove_dir_all(path), path, "directory")
                    }
                };
                if let Err(err) = result {
                    warn!(
                        "Failed to remove {} during cleanup, path: {}, err: {}",
                        entity,
                        path.display(),
                        err
                    );
                }
            }
        }

        trace!("Task finished: clean up cache");
    }

    // Be fault tolerant: list as much as you can, and ignore the rest
    fn list_cache_contents(&self) -> Vec<CacheEntry> {
        fn enter_dir(
            vec: &mut Vec<CacheEntry>,
            dir_path: &Path,
            level: u8,
            cache_config: &CacheConfig,
        ) {
            macro_rules! add_unrecognized {
                (file: $path:expr) => {
                    add_unrecognized!(false, $path)
                };
                (dir: $path:expr) => {
                    add_unrecognized!(true, $path)
                };
                ($is_dir:expr, $path:expr) => {
                    vec.push(CacheEntry::Unrecognized {
                        path: $path.to_path_buf(),
                        is_dir: $is_dir,
                    })
                };
            }
            macro_rules! add_unrecognized_and {
                ([ $( $ty:ident: $path:expr ),* ], $cont:stmt) => {{
                    $( add_unrecognized!($ty: $path); )*
                        $cont
                }};
            }

            macro_rules! unwrap_or {
                ($result:expr, $cont:stmt, $err_msg:expr) => {
                    unwrap_or!($result, $cont, $err_msg, dir_path)
                };
                ($result:expr, $cont:stmt, $err_msg:expr, $path:expr) => {
                    unwrap_or_warn!(
                        $result,
                        $cont,
                        format!("{}, level: {}", $err_msg, level),
                        $path
                    )
                };
            }

            // If we fail to list a directory, something bad is happening anyway
            // (something touches our cache or we have disk failure)
            // Try to delete it, so we can stay within soft limits of the cache size.
            // This comment applies later in this function, too.
            let it = unwrap_or!(
                fs::read_dir(dir_path),
                add_unrecognized_and!([dir: dir_path], return),
                "Failed to list cache directory, deleting it"
            );

            let mut cache_files = HashMap::new();
            for entry in it {
                // read_dir() returns an iterator over results - in case some of them are errors
                // we don't know their names, so we can't delete them. We don't want to delete
                // the whole directory with good entries too, so we just ignore the erroneous entries.
                let entry = unwrap_or!(
                    entry,
                    continue,
                    "Failed to read a cache dir entry (NOT deleting it, it still occupies space)"
                );
                let path = entry.path();
                match (level, path.is_dir()) {
                    (0..=1, true) => enter_dir(vec, &path, level + 1, cache_config),
                    (0..=1, false) => {
                        if level == 0
                            && path.file_stem() == Some(OsStr::new(".cleanup"))
                                && path.extension().is_some()
                                // assume it's cleanup lock
                                && !is_fs_lock_expired(
                                    Some(&entry),
                                    &path,
                                    cache_config.cleanup_interval(),
                                    cache_config.allowed_clock_drift_for_files_from_future(),
                                )
                        {
                            continue; // skip active lock
                        }
                        add_unrecognized!(file: path);
                    }
                    (2, false) => {
                        match path.extension().and_then(OsStr::to_str) {
                            // mod or stats file
                            None | Some("stats") => {
                                cache_files.insert(path, entry);
                            }

                            Some(ext) => {
                                // check if valid lock
                                let recognized = ext.starts_with("wip-")
                                    && !is_fs_lock_expired(
                                        Some(&entry),
                                        &path,
                                        cache_config.optimizing_compression_task_timeout(),
                                        cache_config.allowed_clock_drift_for_files_from_future(),
                                    );

                                if !recognized {
                                    add_unrecognized!(file: path);
                                }
                            }
                        }
                    }
                    (_, is_dir) => add_unrecognized!(is_dir, path),
                }
            }

            // associate module with its stats & handle them
            // assumption: just mods and stats
            for (path, entry) in cache_files.iter() {
                let path_buf: PathBuf;
                let (mod_, stats_, is_mod) = match path.extension() {
                    Some(_) => {
                        path_buf = path.with_extension("");
                        (
                            cache_files.get(&path_buf).map(|v| (&path_buf, v)),
                            Some((path, entry)),
                            false,
                        )
                    }
                    None => {
                        path_buf = path.with_extension("stats");
                        (
                            Some((path, entry)),
                            cache_files.get(&path_buf).map(|v| (&path_buf, v)),
                            true,
                        )
                    }
                };

                // construct a cache entry
                match (mod_, stats_, is_mod) {
                    (Some((mod_path, mod_entry)), Some((stats_path, stats_entry)), true) => {
                        let mod_metadata = unwrap_or!(
                            mod_entry.metadata(),
                            add_unrecognized_and!([file: stats_path, file: mod_path], continue),
                            "Failed to get metadata, deleting BOTH module cache and stats files",
                            mod_path
                        );
                        let stats_mtime = unwrap_or!(
                            stats_entry.metadata().and_then(|m| m.modified()),
                            add_unrecognized_and!(
                                [file: stats_path],
                                unwrap_or!(
                                    mod_metadata.modified(),
                                    add_unrecognized_and!(
                                        [file: stats_path, file: mod_path],
                                        continue
                                    ),
                                    "Failed to get mtime, deleting BOTH module cache and stats \
                                     files",
                                    mod_path
                                )
                            ),
                            "Failed to get metadata/mtime, deleting the file",
                            stats_path
                        );
                        // .into() called for the SystemTimeStub if cfg(test)
                        vec.push(CacheEntry::Recognized {
                            path: mod_path.to_path_buf(),
                            mtime: stats_mtime.into(),
                            size: mod_metadata.len(),
                        })
                    }
                    (Some(_), Some(_), false) => (), // was or will be handled by previous branch
                    (Some((mod_path, mod_entry)), None, _) => {
                        let (mod_metadata, mod_mtime) = unwrap_or!(
                            mod_entry
                                .metadata()
                                .and_then(|md| md.modified().map(|mt| (md, mt))),
                            add_unrecognized_and!([file: mod_path], continue),
                            "Failed to get metadata/mtime, deleting the file",
                            mod_path
                        );
                        // .into() called for the SystemTimeStub if cfg(test)
                        vec.push(CacheEntry::Recognized {
                            path: mod_path.to_path_buf(),
                            mtime: mod_mtime.into(),
                            size: mod_metadata.len(),
                        })
                    }
                    (None, Some((stats_path, _stats_entry)), _) => {
                        debug!("Found orphaned stats file: {}", stats_path.display());
                        add_unrecognized!(file: stats_path);
                    }
                    _ => unreachable!(),
                }
            }
        }

        let mut vec = Vec::new();
        enter_dir(&mut vec, self.directory(), 0, &self.cache_config);
        vec
    }
}

fn read_stats_file(path: &Path) -> Option<ModuleCacheStatistics> {
    fs::read_to_string(path)
        .map_err(|err| {
            trace!(
                "Failed to read stats file, path: {}, err: {}",
                path.display(),
                err
            )
        })
        .and_then(|contents| {
            toml::from_str::<ModuleCacheStatistics>(&contents).map_err(|err| {
                trace!(
                    "Failed to parse stats file, path: {}, err: {}",
                    path.display(),
                    err,
                )
            })
        })
        .ok()
}

fn write_stats_file(path: &Path, stats: &ModuleCacheStatistics) -> bool {
    toml::to_string_pretty(&stats)
        .map_err(|err| {
            warn!(
                "Failed to serialize stats file, path: {}, err: {}",
                path.display(),
                err
            )
        })
        .and_then(|serialized| {
            fs_write_atomic(path, "stats", serialized.as_bytes()).map_err(|_| ())
        })
        .is_ok()
}

/// Tries to acquire a lock for specific task.
///
/// Returns Some(path) to the lock if succeeds. The task path must not
/// contain any extension and have file stem.
///
/// To release a lock you need either manually rename or remove it,
/// or wait until it expires and cleanup task removes it.
///
/// Note: this function is racy. Main idea is: be fault tolerant and
///       never block some task. The price is that we rarely do some task
///       more than once.
fn acquire_task_fs_lock(
    task_path: &Path,
    timeout: Duration,
    allowed_future_drift: Duration,
) -> Option<PathBuf> {
    assert!(task_path.extension().is_none());
    assert!(task_path.file_stem().is_some());

    // list directory
    let dir_path = task_path.parent()?;
    let it = fs::read_dir(dir_path)
        .map_err(|err| {
            warn!(
                "Failed to list cache directory, path: {}, err: {}",
                dir_path.display(),
                err
            )
        })
        .ok()?;

    // look for existing locks
    for entry in it {
        let entry = entry
            .map_err(|err| {
                warn!(
                    "Failed to list cache directory, path: {}, err: {}",
                    dir_path.display(),
                    err
                )
            })
            .ok()?;

        let path = entry.path();
        if path.is_dir() || path.file_stem() != task_path.file_stem() {
            continue;
        }

        // check extension and mtime
        match path.extension() {
            None => continue,
            Some(ext) => {
                if let Some(ext_str) = ext.to_str() {
                    // if it's None, i.e. not valid UTF-8 string, then that's not our lock for sure
                    if ext_str.starts_with("wip-")
                        && !is_fs_lock_expired(Some(&entry), &path, timeout, allowed_future_drift)
                    {
                        return None;
                    }
                }
            }
        }
    }

    // create the lock
    let lock_path = task_path.with_extension(format!("wip-{}", std::process::id()));
    let _file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&lock_path)
        .map_err(|err| {
            warn!(
                "Failed to create lock file (note: it shouldn't exists): path: {}, err: {}",
                lock_path.display(),
                err
            )
        })
        .ok()?;

    Some(lock_path)
}

// we have either both, or just path; dir entry is desirable since on some platforms we can get
// metadata without extra syscalls
// furthermore: it's better to get a path if we have it instead of allocating a new one from the dir entry
fn is_fs_lock_expired(
    entry: Option<&fs::DirEntry>,
    path: &PathBuf,
    threshold: Duration,
    allowed_future_drift: Duration,
) -> bool {
    let mtime = match entry
        .map_or_else(|| path.metadata(), |e| e.metadata())
        .and_then(|metadata| metadata.modified())
    {
        Ok(mt) => mt,
        Err(err) => {
            warn!(
                "Failed to get metadata/mtime, treating as an expired lock, path: {}, err: {}",
                path.display(),
                err
            );
            return true; // can't read mtime, treat as expired, so this task will not be starved
        }
    };

    // DON'T use: mtime.elapsed() -- we must call SystemTime directly for the tests to be deterministic
    match SystemTime::now().duration_since(mtime) {
        Ok(elapsed) => elapsed >= threshold,
        Err(err) => {
            trace!(
                "Found mtime in the future, treating as a not expired lock, path: {}, err: {}",
                path.display(),
                err
            );
            // the lock is expired if the time is too far in the future
            // it is fine to have network share and not synchronized clocks,
            // but it's not good when user changes time in their system clock
            err.duration() > allowed_future_drift
        }
    }
}

#[cfg(test)]
mod tests;
