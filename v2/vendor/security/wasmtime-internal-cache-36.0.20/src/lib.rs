//! > **⚠️ Warning ⚠️**: this crate is an internal-only crate for the Wasmtime
//! > project and is not intended for general use. APIs are not strictly
//! > reviewed for safety and usage outside of Wasmtime may have bugs. If
//! > you're interested in using this feel free to file an issue on the
//! > Wasmtime repository to start a discussion about doing so, but otherwise
//! > be aware that your usage of this crate is not supported.

use anyhow::Result;
use base64::Engine;
use log::{debug, trace, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::hash::Hash;
use std::hash::Hasher;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::time::Duration;
use std::{fs, io};

#[macro_use] // for tests
mod config;
mod worker;

pub use config::{CacheConfig, create_new_config};
use worker::Worker;

/// Global configuration for how the cache is managed
#[derive(Debug, Clone)]
pub struct Cache {
    config: CacheConfig,
    worker: Worker,
    state: Arc<CacheState>,
}

macro_rules! generate_config_setting_getter {
    ($setting:ident: $setting_type:ty) => {
        #[doc = concat!("Returns ", "`", stringify!($setting), "`.")]
        pub fn $setting(&self) -> $setting_type {
            self.config.$setting()
        }
    };
}

impl Cache {
    /// Builds a [`Cache`] from the configuration and spawns the cache worker.
    ///
    /// If you want to load the cache configuration from a file, use [`CacheConfig::from_file`].
    /// You can call [`CacheConfig::new`] for the default configuration.
    ///
    /// # Errors
    /// Returns an error if the configuration is invalid.
    pub fn new(mut config: CacheConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            worker: Worker::start_new(&config),
            config,
            state: Default::default(),
        })
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
    pub fn from_file(path: Option<&Path>) -> Result<Self> {
        let config = CacheConfig::from_file(path)?;
        Self::new(config)
    }

    generate_config_setting_getter!(worker_event_queue_size: u64);
    generate_config_setting_getter!(baseline_compression_level: i32);
    generate_config_setting_getter!(optimized_compression_level: i32);
    generate_config_setting_getter!(optimized_compression_usage_counter_threshold: u64);
    generate_config_setting_getter!(cleanup_interval: Duration);
    generate_config_setting_getter!(optimizing_compression_task_timeout: Duration);
    generate_config_setting_getter!(allowed_clock_drift_for_files_from_future: Duration);
    generate_config_setting_getter!(file_count_soft_limit: u64);
    generate_config_setting_getter!(files_total_size_soft_limit: u64);
    generate_config_setting_getter!(file_count_limit_percent_if_deleting: u8);
    generate_config_setting_getter!(files_total_size_limit_percent_if_deleting: u8);

    /// Returns path to the cache directory.
    pub fn directory(&self) -> &PathBuf {
        &self
            .config
            .directory()
            .expect("directory should be validated in Config::new")
    }

    #[cfg(test)]
    fn worker(&self) -> &Worker {
        &self.worker
    }

    /// Returns the number of cache hits seen so far
    pub fn cache_hits(&self) -> usize {
        self.state.hits.load(SeqCst)
    }

    /// Returns the number of cache misses seen so far
    pub fn cache_misses(&self) -> usize {
        self.state.misses.load(SeqCst)
    }

    pub(crate) fn on_cache_get_async(&self, path: impl AsRef<Path>) {
        self.state.hits.fetch_add(1, SeqCst);
        self.worker.on_cache_get_async(path)
    }

    pub(crate) fn on_cache_update_async(&self, path: impl AsRef<Path>) {
        self.state.misses.fetch_add(1, SeqCst);
        self.worker.on_cache_update_async(path)
    }
}

#[derive(Default, Debug)]
struct CacheState {
    hits: AtomicUsize,
    misses: AtomicUsize,
}

/// Module level cache entry.
pub struct ModuleCacheEntry<'cache>(Option<ModuleCacheEntryInner<'cache>>);

struct ModuleCacheEntryInner<'cache> {
    root_path: PathBuf,
    cache: &'cache Cache,
}

struct Sha256Hasher(Sha256);

impl<'cache> ModuleCacheEntry<'cache> {
    /// Create the cache entry.
    pub fn new(compiler_name: &str, cache: Option<&'cache Cache>) -> Self {
        Self(cache.map(|cache| ModuleCacheEntryInner::new(compiler_name, cache)))
    }

    #[cfg(test)]
    fn from_inner(inner: ModuleCacheEntryInner<'cache>) -> Self {
        Self(Some(inner))
    }

    /// Gets cached data if state matches, otherwise calls `compute`.
    ///
    /// Data is automatically serialized/deserialized with `bincode`.
    pub fn get_data<T, U, E>(&self, state: T, compute: fn(&T) -> Result<U, E>) -> Result<U, E>
    where
        T: Hash,
        U: Serialize + for<'a> Deserialize<'a>,
    {
        self.get_data_raw(
            &state,
            compute,
            |_state, data| postcard::to_allocvec(data).ok(),
            |_state, data| postcard::from_bytes(&data).ok(),
        )
    }

    /// Gets cached data if state matches, otherwise calls `compute`.
    ///
    /// If the cache is disabled or no cached data is found then `compute` is
    /// called to calculate the data. If the data was found in cache it is
    /// passed to `deserialize`, which if successful will be the returned value.
    /// When computed the `serialize` function is used to generate the bytes
    /// from the returned value.
    pub fn get_data_raw<T, U, E>(
        &self,
        state: &T,
        // NOTE: These are function pointers instead of closures so that they
        // don't accidentally close over something not accounted in the cache.
        compute: fn(&T) -> Result<U, E>,
        serialize: fn(&T, &U) -> Option<Vec<u8>>,
        deserialize: fn(&T, Vec<u8>) -> Option<U>,
    ) -> Result<U, E>
    where
        T: Hash,
    {
        let inner = match &self.0 {
            Some(inner) => inner,
            None => return compute(state),
        };

        let mut hasher = Sha256Hasher(Sha256::new());
        state.hash(&mut hasher);
        let hash: [u8; 32] = hasher.0.finalize().into();
        // standard encoding uses '/' which can't be used for filename
        let hash = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&hash);

        if let Some(cached_val) = inner.get_data(&hash) {
            if let Some(val) = deserialize(state, cached_val) {
                let mod_cache_path = inner.root_path.join(&hash);
                inner.cache.on_cache_get_async(&mod_cache_path); // call on success
                return Ok(val);
            }
        }
        let val_to_cache = compute(state)?;
        if let Some(bytes) = serialize(state, &val_to_cache) {
            if inner.update_data(&hash, &bytes).is_some() {
                let mod_cache_path = inner.root_path.join(&hash);
                inner.cache.on_cache_update_async(&mod_cache_path); // call on success
            }
        }
        Ok(val_to_cache)
    }
}

impl<'cache> ModuleCacheEntryInner<'cache> {
    fn new(compiler_name: &str, cache: &'cache Cache) -> Self {
        // If debug assertions are enabled then assume that we're some sort of
        // local build. We don't want local builds to stomp over caches between
        // builds, so just use a separate cache directory based on the mtime of
        // our executable, which should roughly correlate with "you changed the
        // source code so you get a different directory".
        //
        // Otherwise if this is a release build we use the `GIT_REV` env var
        // which is either the git rev if installed from git or the crate
        // version if installed from crates.io.
        let compiler_dir = if cfg!(debug_assertions) {
            fn self_mtime() -> Option<String> {
                let path = std::env::current_exe().ok()?;
                let metadata = path.metadata().ok()?;
                let mtime = metadata.modified().ok()?;
                Some(match mtime.duration_since(std::time::UNIX_EPOCH) {
                    Ok(dur) => format!("{}", dur.as_millis()),
                    Err(err) => format!("m{}", err.duration().as_millis()),
                })
            }
            let self_mtime = self_mtime().unwrap_or("no-mtime".to_string());
            format!(
                "{comp_name}-{comp_ver}-{comp_mtime}",
                comp_name = compiler_name,
                comp_ver = env!("GIT_REV"),
                comp_mtime = self_mtime,
            )
        } else {
            format!(
                "{comp_name}-{comp_ver}",
                comp_name = compiler_name,
                comp_ver = env!("GIT_REV"),
            )
        };
        let root_path = cache.directory().join("modules").join(compiler_dir);

        Self { root_path, cache }
    }

    fn get_data(&self, hash: &str) -> Option<Vec<u8>> {
        let mod_cache_path = self.root_path.join(hash);
        trace!("get_data() for path: {}", mod_cache_path.display());
        let compressed_cache_bytes = fs::read(&mod_cache_path).ok()?;
        let cache_bytes = zstd::decode_all(&compressed_cache_bytes[..])
            .map_err(|err| warn!("Failed to decompress cached code: {err}"))
            .ok()?;
        Some(cache_bytes)
    }

    fn update_data(&self, hash: &str, serialized_data: &[u8]) -> Option<()> {
        let mod_cache_path = self.root_path.join(hash);
        trace!("update_data() for path: {}", mod_cache_path.display());
        let compressed_data = zstd::encode_all(
            &serialized_data[..],
            self.cache.baseline_compression_level(),
        )
        .map_err(|err| warn!("Failed to compress cached code: {err}"))
        .ok()?;

        // Optimize syscalls: first, try writing to disk. It should succeed in most cases.
        // Otherwise, try creating the cache directory and retry writing to the file.
        if fs_write_atomic(&mod_cache_path, "mod", &compressed_data).is_ok() {
            return Some(());
        }

        debug!(
            "Attempting to create the cache directory, because \
             failed to write cached code to disk, path: {}",
            mod_cache_path.display(),
        );

        let cache_dir = mod_cache_path.parent().unwrap();
        fs::create_dir_all(cache_dir)
            .map_err(|err| {
                warn!(
                    "Failed to create cache directory, path: {}, message: {}",
                    cache_dir.display(),
                    err
                )
            })
            .ok()?;

        match fs_write_atomic(&mod_cache_path, "mod", &compressed_data) {
            Ok(_) => Some(()),
            Err(err) => {
                warn!(
                    "Failed to write file with rename, target path: {}, err: {}",
                    mod_cache_path.display(),
                    err
                );
                None
            }
        }
    }
}

impl Hasher for Sha256Hasher {
    fn finish(&self) -> u64 {
        panic!("Sha256Hasher doesn't support finish!");
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
}

// Assumption: path inside cache directory.
// Then, we don't have to use sound OS-specific exclusive file access.
// Note: there's no need to remove temporary file here - cleanup task will do it later.
fn fs_write_atomic(path: &Path, reason: &str, contents: &[u8]) -> io::Result<()> {
    let lock_path = path.with_extension(format!("wip-atomic-write-{reason}"));
    fs::OpenOptions::new()
        .create_new(true) // atomic file creation (assumption: no one will open it without this flag)
        .write(true)
        .open(&lock_path)
        .and_then(|mut file| file.write_all(contents))
        // file should go out of scope and be closed at this point
        .and_then(|()| fs::rename(&lock_path, &path)) // atomic file rename
}

#[cfg(test)]
mod tests;
