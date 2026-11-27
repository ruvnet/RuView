//! Module for configuring the cache system.

use anyhow::{Context, Result, anyhow, bail};
use directories_next::ProjectDirs;
use log::{trace, warn};
use serde::{
    Deserialize,
    de::{self, Deserializer},
};
use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// wrapped, so we have named section in config,
// also, for possible future compatibility
#[derive(serde_derive::Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct Config {
    cache: CacheConfig,
}

/// Global configuration for how the cache is managed
#[derive(serde_derive::Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct CacheConfig {
    directory: Option<PathBuf>,
    #[serde(
        default = "default_worker_event_queue_size",
        rename = "worker-event-queue-size",
        deserialize_with = "deserialize_si_prefix"
    )]
    worker_event_queue_size: u64,
    #[serde(
        default = "default_baseline_compression_level",
        rename = "baseline-compression-level"
    )]
    baseline_compression_level: i32,
    #[serde(
        default = "default_optimized_compression_level",
        rename = "optimized-compression-level"
    )]
    optimized_compression_level: i32,
    #[serde(
        default = "default_optimized_compression_usage_counter_threshold",
        rename = "optimized-compression-usage-counter-threshold",
        deserialize_with = "deserialize_si_prefix"
    )]
    optimized_compression_usage_counter_threshold: u64,
    #[serde(
        default = "default_cleanup_interval",
        rename = "cleanup-interval",
        deserialize_with = "deserialize_duration"
    )]
    cleanup_interval: Duration,
    #[serde(
        default = "default_optimizing_compression_task_timeout",
        rename = "optimizing-compression-task-timeout",
        deserialize_with = "deserialize_duration"
    )]
    optimizing_compression_task_timeout: Duration,
    #[serde(
        default = "default_allowed_clock_drift_for_files_from_future",
        rename = "allowed-clock-drift-for-files-from-future",
        deserialize_with = "deserialize_duration"
    )]
    allowed_clock_drift_for_files_from_future: Duration,
    #[serde(
        default = "default_file_count_soft_limit",
        rename = "file-count-soft-limit",
        deserialize_with = "deserialize_si_prefix"
    )]
    file_count_soft_limit: u64,
    #[serde(
        default = "default_files_total_size_soft_limit",
        rename = "files-total-size-soft-limit",
        deserialize_with = "deserialize_disk_space"
    )]
    files_total_size_soft_limit: u64,
    #[serde(
        default = "default_file_count_limit_percent_if_deleting",
        rename = "file-count-limit-percent-if-deleting",
        deserialize_with = "deserialize_percent"
    )]
    file_count_limit_percent_if_deleting: u8,
    #[serde(
        default = "default_files_total_size_limit_percent_if_deleting",
        rename = "files-total-size-limit-percent-if-deleting",
        deserialize_with = "deserialize_percent"
    )]
    files_total_size_limit_percent_if_deleting: u8,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            directory: None,
            worker_event_queue_size: default_worker_event_queue_size(),
            baseline_compression_level: default_baseline_compression_level(),
            optimized_compression_level: default_optimized_compression_level(),
            optimized_compression_usage_counter_threshold:
                default_optimized_compression_usage_counter_threshold(),
            cleanup_interval: default_cleanup_interval(),
            optimizing_compression_task_timeout: default_optimizing_compression_task_timeout(),
            allowed_clock_drift_for_files_from_future:
                default_allowed_clock_drift_for_files_from_future(),
            file_count_soft_limit: default_file_count_soft_limit(),
            files_total_size_soft_limit: default_files_total_size_soft_limit(),
            file_count_limit_percent_if_deleting: default_file_count_limit_percent_if_deleting(),
            files_total_size_limit_percent_if_deleting:
                default_files_total_size_limit_percent_if_deleting(),
        }
    }
}

/// Creates a new configuration file at specified path, or default path if None is passed.
/// Fails if file already exists.
pub fn create_new_config<P: AsRef<Path> + Debug>(config_file: Option<P>) -> Result<PathBuf> {
    trace!("Creating new config file, path: {config_file:?}");

    let config_file = match config_file {
        Some(path) => path.as_ref().to_path_buf(),
        None => default_config_path()?,
    };

    if config_file.exists() {
        bail!(
            "Configuration file '{}' already exists.",
            config_file.display()
        );
    }

    let parent_dir = config_file
        .parent()
        .ok_or_else(|| anyhow!("Invalid cache config path: {}", config_file.display()))?;

    fs::create_dir_all(parent_dir).with_context(|| {
        format!(
            "Failed to create config directory, config path: {}",
            config_file.display(),
        )
    })?;

    let content = "\
# Comment out certain settings to use default values.
# For more settings, please refer to the documentation:
# https://bytecodealliance.github.io/wasmtime/cli-cache.html

[cache]
";

    fs::write(&config_file, content).with_context(|| {
        format!(
            "Failed to flush config to the disk, path: {}",
            config_file.display(),
        )
    })?;

    Ok(config_file.to_path_buf())
}

// permitted levels from: https://docs.rs/zstd/0.4.28+zstd.1.4.3/zstd/stream/write/struct.Encoder.html
const ZSTD_COMPRESSION_LEVELS: std::ops::RangeInclusive<i32> = 0..=21;

// Default settings, you're welcome to tune them!
// TODO: what do we want to warn users about?

// At the moment of writing, the modules couldn't depend on another,
// so we have at most one module per wasmtime instance
// if changed, update cli-cache.md
const fn default_worker_event_queue_size() -> u64 {
    0x10
}
const fn worker_event_queue_size_warning_threshold() -> u64 {
    3
}
// should be quick and provide good enough compression
// if changed, update cli-cache.md
const fn default_baseline_compression_level() -> i32 {
    zstd::DEFAULT_COMPRESSION_LEVEL
}
// should provide significantly better compression than baseline
// if changed, update cli-cache.md
const fn default_optimized_compression_level() -> i32 {
    20
}
// shouldn't be to low to avoid recompressing too many files
// if changed, update cli-cache.md
const fn default_optimized_compression_usage_counter_threshold() -> u64 {
    0x100
}
// if changed, update cli-cache.md
const fn default_cleanup_interval() -> Duration {
    Duration::from_secs(60 * 60)
}
// if changed, update cli-cache.md
const fn default_optimizing_compression_task_timeout() -> Duration {
    Duration::from_secs(30 * 60)
}
// the default assumes problems with timezone configuration on network share + some clock drift
// please notice 24 timezones = max 23h difference between some of them
// if changed, update cli-cache.md
const fn default_allowed_clock_drift_for_files_from_future() -> Duration {
    Duration::from_secs(60 * 60 * 24)
}
// if changed, update cli-cache.md
const fn default_file_count_soft_limit() -> u64 {
    0x10_000
}
// if changed, update cli-cache.md
const fn default_files_total_size_soft_limit() -> u64 {
    1024 * 1024 * 512
}
// if changed, update cli-cache.md
const fn default_file_count_limit_percent_if_deleting() -> u8 {
    70
}
// if changed, update cli-cache.md
const fn default_files_total_size_limit_percent_if_deleting() -> u8 {
    70
}

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "BytecodeAlliance", "wasmtime")
}

fn default_config_path() -> Result<PathBuf> {
    match project_dirs() {
        Some(dirs) => Ok(dirs.config_dir().join("config.toml")),
        None => bail!("config file not specified and failed to get the default"),
    }
}

// Deserializers of our custom formats
// can be replaced with const generics later
macro_rules! generate_deserializer {
    ($name:ident($numname:ident: $numty:ty, $unitname:ident: &str) -> $retty:ty {$body:expr}) => {
        fn $name<'de, D>(deserializer: D) -> Result<$retty, D::Error>
        where
            D: Deserializer<'de>,
        {
            let text = String::deserialize(deserializer)?;
            let text = text.trim();
            let split_point = text.find(|c: char| !c.is_numeric());
            let (num, unit) = split_point.map_or_else(|| (text, ""), |p| text.split_at(p));
            let deserialized = (|| {
                let $numname = num.parse::<$numty>().ok()?;
                let $unitname = unit.trim();
                $body
            })();
            if let Some(deserialized) = deserialized {
                Ok(deserialized)
            } else {
                Err(de::Error::custom(
                    "Invalid value, please refer to the documentation",
                ))
            }
        }
    };
}

generate_deserializer!(deserialize_duration(num: u64, unit: &str) -> Duration {
    match unit {
        "s" => Some(Duration::from_secs(num)),
        "m" => Some(Duration::from_secs(num * 60)),
        "h" => Some(Duration::from_secs(num * 60 * 60)),
        "d" => Some(Duration::from_secs(num * 60 * 60 * 24)),
        _ => None,
    }
});

generate_deserializer!(deserialize_si_prefix(num: u64, unit: &str) -> u64 {
    match unit {
        "" => Some(num),
        "K" => num.checked_mul(1_000),
        "M" => num.checked_mul(1_000_000),
        "G" => num.checked_mul(1_000_000_000),
        "T" => num.checked_mul(1_000_000_000_000),
        "P" => num.checked_mul(1_000_000_000_000_000),
        _ => None,
    }
});

generate_deserializer!(deserialize_disk_space(num: u64, unit: &str) -> u64 {
    match unit {
        "" => Some(num),
        "K" => num.checked_mul(1_000),
        "Ki" => num.checked_mul(1u64 << 10),
        "M" => num.checked_mul(1_000_000),
        "Mi" => num.checked_mul(1u64 << 20),
        "G" => num.checked_mul(1_000_000_000),
        "Gi" => num.checked_mul(1u64 << 30),
        "T" => num.checked_mul(1_000_000_000_000),
        "Ti" => num.checked_mul(1u64 << 40),
        "P" => num.checked_mul(1_000_000_000_000_000),
        "Pi" => num.checked_mul(1u64 << 50),
        _ => None,
    }
});

generate_deserializer!(deserialize_percent(num: u8, unit: &str) -> u8 {
    match unit {
        "%" => Some(num),
        _ => None,
    }
});

macro_rules! generate_setting_getter {
    ($setting:ident: $setting_type:ty) => {
        #[doc = concat!("Returns ", "`", stringify!($setting), "`.")]
        pub fn $setting(&self) -> $setting_type {
            self.$setting
        }
    };
}

impl CacheConfig {
    /// Creates a cache configuration with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads cache configuration specified at `path`.
    ///
    /// This method will read the file specified by `path` on the filesystem and
    /// attempt to load cache configuration from it. This method can also fail
    /// due to I/O errors, misconfiguration, syntax errors, etc. For expected
    /// syntax in the configuration file see the [documentation online][docs].
    ///
    /// Passing in `None` loads cache configuration from the system default path.
    /// This is located, for example, on Unix at `$HOME/.config/wasmtime/config.toml`
    /// and is typically created with the `wasmtime config new` command.
    ///
    /// # Errors
    ///
    /// This method can fail due to any error that happens when loading the file
    /// pointed to by `path` and attempting to load the cache configuration.
    ///
    /// [docs]: https://bytecodealliance.github.io/wasmtime/cli-cache.html
    pub fn from_file(config_file: Option<&Path>) -> Result<Self> {
        let mut config = Self::load_and_parse_file(config_file)?;
        config.validate()?;
        Ok(config)
    }

    fn load_and_parse_file(config_file: Option<&Path>) -> Result<Self> {
        // get config file path
        let (config_file, user_custom_file) = match config_file {
            Some(path) => (path.to_path_buf(), true),
            None => (default_config_path()?, false),
        };

        // read config, or use default one
        let entity_exists = config_file.exists();
        match (entity_exists, user_custom_file) {
            (false, false) => Ok(Self::new()),
            _ => {
                let contents = fs::read_to_string(&config_file).context(format!(
                    "failed to read config file: {}",
                    config_file.display()
                ))?;
                let config = toml::from_str::<Config>(&contents).context(format!(
                    "failed to parse config file: {}",
                    config_file.display()
                ))?;
                Ok(config.cache)
            }
        }
    }

    generate_setting_getter!(worker_event_queue_size: u64);
    generate_setting_getter!(baseline_compression_level: i32);
    generate_setting_getter!(optimized_compression_level: i32);
    generate_setting_getter!(optimized_compression_usage_counter_threshold: u64);
    generate_setting_getter!(cleanup_interval: Duration);
    generate_setting_getter!(optimizing_compression_task_timeout: Duration);
    generate_setting_getter!(allowed_clock_drift_for_files_from_future: Duration);
    generate_setting_getter!(file_count_soft_limit: u64);
    generate_setting_getter!(files_total_size_soft_limit: u64);
    generate_setting_getter!(file_count_limit_percent_if_deleting: u8);
    generate_setting_getter!(files_total_size_limit_percent_if_deleting: u8);

    /// Returns path to the cache directory if one is set.
    pub fn directory(&self) -> Option<&PathBuf> {
        self.directory.as_ref()
    }

    /// Specify where the cache directory is. Must be an absolute path.
    pub fn with_directory(&mut self, directory: impl Into<PathBuf>) -> &mut Self {
        self.directory = Some(directory.into());
        self
    }

    /// Size of cache worker event queue. If the queue is full, incoming cache usage events will be
    /// dropped.
    pub fn with_worker_event_queue_size(&mut self, size: u64) -> &mut Self {
        self.worker_event_queue_size = size;
        self
    }

    /// Compression level used when a new cache file is being written by the cache system. Wasmtime
    /// uses zstd compression.
    pub fn with_baseline_compression_level(&mut self, level: i32) -> &mut Self {
        self.baseline_compression_level = level;
        self
    }

    /// Compression level used when the cache worker decides to recompress a cache file. Wasmtime
    /// uses zstd compression.
    pub fn with_optimized_compression_level(&mut self, level: i32) -> &mut Self {
        self.optimized_compression_level = level;
        self
    }

    /// One of the conditions for the cache worker to recompress a cache file is to have usage
    /// count of the file exceeding this threshold.
    pub fn with_optimized_compression_usage_counter_threshold(
        &mut self,
        threshold: u64,
    ) -> &mut Self {
        self.optimized_compression_usage_counter_threshold = threshold;
        self
    }

    /// When the cache worker is notified about a cache file being updated by the cache system and
    /// this interval has already passed since last cleaning up, the worker will attempt a new
    /// cleanup.
    pub fn with_cleanup_interval(&mut self, interval: Duration) -> &mut Self {
        self.cleanup_interval = interval;
        self
    }

    /// When the cache worker decides to recompress a cache file, it makes sure that no other
    /// worker has started the task for this file within the last
    /// optimizing-compression-task-timeout interval. If some worker has started working on it,
    /// other workers are skipping this task.
    pub fn with_optimizing_compression_task_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.optimizing_compression_task_timeout = timeout;
        self
    }

    /// ### Locks
    ///
    /// When the cache worker attempts acquiring a lock for some task, it checks if some other
    /// worker has already acquired such a lock. To be fault tolerant and eventually execute every
    /// task, the locks expire after some interval. However, because of clock drifts and different
    /// timezones, it would happen that some lock was created in the future. This setting defines a
    /// tolerance limit for these locks. If the time has been changed in the system (i.e. two years
    /// backwards), the cache system should still work properly. Thus, these locks will be treated
    /// as expired (assuming the tolerance is not too big).
    ///
    /// ### Cache files
    ///
    /// Similarly to the locks, the cache files or their metadata might have modification time in
    /// distant future. The cache system tries to keep these files as long as possible. If the
    /// limits are not reached, the cache files will not be deleted. Otherwise, they will be
    /// treated as the oldest files, so they might survive. If the user actually uses the cache
    /// file, the modification time will be updated.
    pub fn with_allowed_clock_drift_for_files_from_future(&mut self, drift: Duration) -> &mut Self {
        self.allowed_clock_drift_for_files_from_future = drift;
        self
    }

    /// Soft limit for the file count in the cache directory.
    ///
    /// This doesn't include files with metadata. To learn more, please refer to the cache system
    /// section.
    pub fn with_file_count_soft_limit(&mut self, limit: u64) -> &mut Self {
        self.file_count_soft_limit = limit;
        self
    }

    /// Soft limit for the total size* of files in the cache directory.
    ///
    /// This doesn't include files with metadata. To learn more, please refer to the cache system
    /// section.
    ///
    /// *this is the file size, not the space physically occupied on the disk.
    pub fn with_files_total_size_soft_limit(&mut self, limit: u64) -> &mut Self {
        self.files_total_size_soft_limit = limit;
        self
    }

    /// If file-count-soft-limit is exceeded and the cache worker performs the cleanup task, then
    /// the worker will delete some cache files, so after the task, the file count should not
    /// exceed file-count-soft-limit * file-count-limit-percent-if-deleting.
    ///
    /// This doesn't include files with metadata. To learn more, please refer to the cache system
    /// section.
    pub fn with_file_count_limit_percent_if_deleting(&mut self, percent: u8) -> &mut Self {
        self.file_count_limit_percent_if_deleting = percent;
        self
    }

    /// If files-total-size-soft-limit is exceeded and cache worker performs the cleanup task, then
    /// the worker will delete some cache files, so after the task, the files total size should not
    /// exceed files-total-size-soft-limit * files-total-size-limit-percent-if-deleting.
    ///
    /// This doesn't include files with metadata. To learn more, please refer to the cache system
    /// section.
    pub fn with_files_total_size_limit_percent_if_deleting(&mut self, percent: u8) -> &mut Self {
        self.files_total_size_limit_percent_if_deleting = percent;
        self
    }

    /// validate values and fill in defaults
    pub(crate) fn validate(&mut self) -> Result<()> {
        self.validate_directory_or_default()?;
        self.validate_worker_event_queue_size();
        self.validate_baseline_compression_level()?;
        self.validate_optimized_compression_level()?;
        self.validate_file_count_limit_percent_if_deleting()?;
        self.validate_files_total_size_limit_percent_if_deleting()?;
        Ok(())
    }

    fn validate_directory_or_default(&mut self) -> Result<()> {
        if self.directory.is_none() {
            match project_dirs() {
                Some(proj_dirs) => self.directory = Some(proj_dirs.cache_dir().to_path_buf()),
                None => {
                    bail!("Cache directory not specified and failed to get the default");
                }
            }
        }

        // On Windows, if we want long paths, we need '\\?\' prefix, but it doesn't work
        // with relative paths. One way to get absolute path (the only one?) is to use
        // fs::canonicalize, but it requires that given path exists. The extra advantage
        // of this method is fact that the method prepends '\\?\' on Windows.
        let cache_dir = self.directory.as_ref().unwrap();

        if !cache_dir.is_absolute() {
            bail!(
                "Cache directory path has to be absolute, path: {}",
                cache_dir.display(),
            );
        }

        fs::create_dir_all(cache_dir).context(format!(
            "failed to create cache directory: {}",
            cache_dir.display()
        ))?;
        let canonical = fs::canonicalize(cache_dir).context(format!(
            "failed to canonicalize cache directory: {}",
            cache_dir.display()
        ))?;
        self.directory = Some(canonical);
        Ok(())
    }

    fn validate_worker_event_queue_size(&self) {
        if self.worker_event_queue_size < worker_event_queue_size_warning_threshold() {
            warn!("Detected small worker event queue size. Some messages might be lost.");
        }
    }

    fn validate_baseline_compression_level(&self) -> Result<()> {
        if !ZSTD_COMPRESSION_LEVELS.contains(&self.baseline_compression_level) {
            bail!(
                "Invalid baseline compression level: {} not in {:#?}",
                self.baseline_compression_level,
                ZSTD_COMPRESSION_LEVELS
            );
        }
        Ok(())
    }

    // assumption: baseline compression level has been verified
    fn validate_optimized_compression_level(&self) -> Result<()> {
        if !ZSTD_COMPRESSION_LEVELS.contains(&self.optimized_compression_level) {
            bail!(
                "Invalid optimized compression level: {} not in {:#?}",
                self.optimized_compression_level,
                ZSTD_COMPRESSION_LEVELS
            );
        }

        if self.optimized_compression_level < self.baseline_compression_level {
            bail!(
                "Invalid optimized compression level is lower than baseline: {} < {}",
                self.optimized_compression_level,
                self.baseline_compression_level
            );
        }
        Ok(())
    }

    fn validate_file_count_limit_percent_if_deleting(&self) -> Result<()> {
        if self.file_count_limit_percent_if_deleting > 100 {
            bail!(
                "Invalid files count limit percent if deleting: {} not in range 0-100%",
                self.file_count_limit_percent_if_deleting
            );
        }
        Ok(())
    }

    fn validate_files_total_size_limit_percent_if_deleting(&self) -> Result<()> {
        if self.files_total_size_limit_percent_if_deleting > 100 {
            bail!(
                "Invalid files total size limit percent if deleting: {} not in range 0-100%",
                self.files_total_size_limit_percent_if_deleting
            );
        }
        Ok(())
    }
}

#[cfg(test)]
#[macro_use]
pub mod tests;
