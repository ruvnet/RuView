use crate::connection::handle::ConnectionHandle;
use crate::connection::LogSettings;
use crate::connection::{ConnectionState, Statements};
use crate::error::Error;
use crate::{SqliteConnectOptions, SqliteError};
use libsqlite3_sys::{
    sqlite3, sqlite3_busy_timeout, sqlite3_db_config, sqlite3_extended_result_codes, sqlite3_free,
    sqlite3_load_extension, sqlite3_open_v2, SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION, SQLITE_OK,
    SQLITE_OPEN_CREATE, SQLITE_OPEN_FULLMUTEX, SQLITE_OPEN_MEMORY, SQLITE_OPEN_NOMUTEX,
    SQLITE_OPEN_PRIVATECACHE, SQLITE_OPEN_READONLY, SQLITE_OPEN_READWRITE, SQLITE_OPEN_SHAREDCACHE,
    SQLITE_OPEN_URI,
};
use percent_encoding::NON_ALPHANUMERIC;
use sqlx_core::IndexMap;
use std::collections::BTreeMap;
use std::ffi::{c_void, CStr, CString};
use std::io;
use std::os::raw::c_int;
use std::ptr::{addr_of_mut, null, null_mut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

// This was originally `AtomicU64` but that's not supported on MIPS (or PowerPC):
// https://github.com/launchbadge/sqlx/issues/2859
// https://doc.rust-lang.org/stable/std/sync/atomic/index.html#portability
static THREAD_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Copy, Clone)]
enum SqliteLoadExtensionMode {
    /// Enables only the C-API, leaving the SQL function disabled.
    Enable,
    /// Disables both the C-API and the SQL function.
    DisableAll,
}

impl SqliteLoadExtensionMode {
    fn to_int(self) -> c_int {
        match self {
            SqliteLoadExtensionMode::Enable => 1,
            SqliteLoadExtensionMode::DisableAll => 0,
        }
    }
}

pub struct EstablishParams {
    filename: CString,
    open_flags: i32,
    busy_timeout: Duration,
    statement_cache_capacity: usize,
    log_settings: LogSettings,
    extensions: IndexMap<CString, Option<CString>>,
    pub(crate) thread_name: String,
    pub(crate) command_channel_size: usize,
    #[cfg(feature = "regexp")]
    register_regexp_function: bool,
}

impl EstablishParams {
    pub fn from_options(options: &SqliteConnectOptions) -> Result<Self, Error> {
        let mut filename = options
            .filename
            .to_str()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "filename passed to SQLite must be valid UTF-8",
                )
            })?
            .to_owned();

        // Set common flags we expect to have in sqlite
        let mut flags = SQLITE_OPEN_URI;

        // By default, we connect to an in-memory database.
        // [SQLITE_OPEN_NOMUTEX] will instruct [sqlite3_open_v2] to return an error if it
        // cannot satisfy our wish for a thread-safe, lock-free connection object

        flags |= if options.serialized {
            SQLITE_OPEN_FULLMUTEX
        } else {
            SQLITE_OPEN_NOMUTEX
        };

        flags |= if options.read_only {
            SQLITE_OPEN_READONLY
        } else if options.create_if_missing {
            SQLITE_OPEN_CREATE | SQLITE_OPEN_READWRITE
        } else {
            SQLITE_OPEN_READWRITE
        };

        if options.in_memory {
            flags |= SQLITE_OPEN_MEMORY;
        }

        flags |= if options.shared_cache {
            SQLITE_OPEN_SHAREDCACHE
        } else {
            SQLITE_OPEN_PRIVATECACHE
        };

        let mut query_params = BTreeMap::new();

        if options.immutable {
            query_params.insert("immutable", "true");
        }

        if let Some(vfs) = options.vfs.as_deref() {
            query_params.insert("vfs", vfs);
        }

        if !query_params.is_empty() {
            filename = format!(
                "file:{}?{}",
                percent_encoding::percent_encode(filename.as_bytes(), NON_ALPHANUMERIC),
                serde_urlencoded::to_string(&query_params).unwrap()
            );
        }

        let filename = CString::new(filename).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "filename passed to SQLite must not contain nul bytes",
            )
        })?;

        let extensions = options
            .extensions
            .iter()
            .map(|(name, entry)| {
                let entry = entry
                    .as_ref()
                    .map(|e| {
                        CString::new(e.as_bytes()).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "extension entrypoint names passed to SQLite must not contain nul bytes"
                            )
                        })
                    })
                    .transpose()?;
                Ok((
                    CString::new(name.as_bytes()).map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "extension names passed to SQLite must not contain nul bytes",
                        )
                    })?,
                    entry,
                ))
            })
            .collect::<Result<IndexMap<CString, Option<CString>>, io::Error>>()?;

        let thread_id = THREAD_ID.fetch_add(1, Ordering::AcqRel);

        Ok(Self {
            filename,
            open_flags: flags,
            busy_timeout: options.busy_timeout,
            statement_cache_capacity: options.statement_cache_capacity,
            log_settings: options.log_settings.clone(),
            extensions,
            thread_name: (options.thread_name)(thread_id as u64),
            command_channel_size: options.command_channel_size,
            #[cfg(feature = "regexp")]
            register_regexp_function: options.register_regexp_function,
        })
    }

    // Enable extension loading via the db_config function, as recommended by the docs rather
    // than the more obvious `sqlite3_enable_load_extension`
    // https://www.sqlite.org/c3ref/db_config.html
    // https://www.sqlite.org/c3ref/c_dbconfig_defensive.html#sqlitedbconfigenableloadextension
    unsafe fn sqlite3_set_load_extension(
        db: *mut sqlite3,
        mode: SqliteLoadExtensionMode,
    ) -> Result<(), Error> {
        let status = sqlite3_db_config(
            db,
            SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION,
            mode.to_int(),
            null::<i32>(),
        );

        if status != SQLITE_OK {
            return Err(Error::Database(Box::new(SqliteError::new(db))));
        }

        Ok(())
    }

    pub(crate) fn establish(&self) -> Result<ConnectionState, Error> {
        let mut handle = null_mut();

        // <https://www.sqlite.org/c3ref/open.html>
        let mut status = unsafe {
            sqlite3_open_v2(self.filename.as_ptr(), &mut handle, self.open_flags, null())
        };

        if handle.is_null() {
            // Failed to allocate memory
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "SQLite is unable to allocate memory to hold the sqlite3 object",
            )));
        }

        // SAFE: tested for NULL just above
        // This allows any returns below to close this handle with RAII
        let mut handle = unsafe { ConnectionHandle::new(handle) };

        if status != SQLITE_OK {
            return Err(Error::Database(Box::new(handle.expect_error())));
        }

        // Enable extended result codes
        // https://www.sqlite.org/c3ref/extended_result_codes.html
        unsafe {
            // NOTE: ignore the failure here
            sqlite3_extended_result_codes(handle.as_ptr(), 1);
        }

        if !self.extensions.is_empty() {
            // Enable loading extensions
            unsafe {
                Self::sqlite3_set_load_extension(handle.as_ptr(), SqliteLoadExtensionMode::Enable)?;
            }

            for ext in self.extensions.iter() {
                // `sqlite3_load_extension` is unusual as it returns its errors via an out-pointer
                // rather than by calling `sqlite3_errmsg`
                let mut error_msg = null_mut();
                status = unsafe {
                    sqlite3_load_extension(
                        handle.as_ptr(),
                        ext.0.as_ptr(),
                        ext.1.as_ref().map_or(null(), |e| e.as_ptr()),
                        addr_of_mut!(error_msg),
                    )
                };

                if status != SQLITE_OK {
                    let mut e = handle.expect_error();

                    // SAFETY: We become responsible for any memory allocation at `&error`, so test
                    // for null and take an RAII version for returns
                    if !error_msg.is_null() {
                        e = e.with_message(unsafe {
                            let msg = CStr::from_ptr(error_msg).to_string_lossy().into();
                            sqlite3_free(error_msg as *mut c_void);
                            msg
                        });
                    }
                    return Err(Error::Database(Box::new(e)));
                }
            } // Preempt any hypothetical security issues arising from leaving ENABLE_LOAD_EXTENSION
              // on by disabling the flag again once we've loaded all the requested modules.
              // Fail-fast (via `?`) if disabling the extension loader didn't work for some reason,
              // avoids an unexpected state going undetected.
            unsafe {
                Self::sqlite3_set_load_extension(
                    handle.as_ptr(),
                    SqliteLoadExtensionMode::DisableAll,
                )?;
            }
        }

        #[cfg(feature = "regexp")]
        if self.register_regexp_function {
            // configure a `regexp` function for sqlite, it does not come with one by default
            let status = crate::regexp::register(handle.as_ptr());
            if status != SQLITE_OK {
                return Err(Error::Database(Box::new(handle.expect_error())));
            }
        }

        // Configure a busy timeout
        // This causes SQLite to automatically sleep in increasing intervals until the time
        // when there is something locked during [sqlite3_step].
        //
        // We also need to convert the u128 value to i32, checking we're not overflowing.
        let ms = i32::try_from(self.busy_timeout.as_millis())
            .expect("Given busy timeout value is too big.");

        status = unsafe { sqlite3_busy_timeout(handle.as_ptr(), ms) };

        if status != SQLITE_OK {
            return Err(Error::Database(Box::new(handle.expect_error())));
        }

        Ok(ConnectionState {
            handle,
            statements: Statements::new(self.statement_cache_capacity),
            log_settings: self.log_settings.clone(),
            progress_handler_callback: None,
            update_hook_callback: None,
            #[cfg(feature = "preupdate-hook")]
            preupdate_hook_callback: None,
            commit_hook_callback: None,
            rollback_hook_callback: None,
        })
    }
}
