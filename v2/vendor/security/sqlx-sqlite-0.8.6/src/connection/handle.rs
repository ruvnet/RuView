use std::ffi::CString;
use std::ptr;
use std::ptr::NonNull;

use crate::error::Error;
use libsqlite3_sys::{
    sqlite3, sqlite3_close, sqlite3_exec, sqlite3_last_insert_rowid, SQLITE_LOCKED_SHAREDCACHE,
    SQLITE_OK,
};

use crate::{statement::unlock_notify, SqliteError};

/// Managed handle to the raw SQLite3 database handle.
/// The database handle will be closed when this is dropped and no `ConnectionHandleRef`s exist.
#[derive(Debug)]
pub(crate) struct ConnectionHandle(NonNull<sqlite3>);

// A SQLite3 handle is safe to send between threads, provided not more than
// one is accessing it at the same time. This is upheld as long as [SQLITE_CONFIG_MULTITHREAD] is
// enabled and [SQLITE_THREADSAFE] was enabled when sqlite was compiled. We refuse to work
// if these conditions are not upheld.

// <https://www.sqlite.org/c3ref/threadsafe.html>

// <https://www.sqlite.org/c3ref/c_config_covering_index_scan.html#sqliteconfigmultithread>

unsafe impl Send for ConnectionHandle {}

impl ConnectionHandle {
    #[inline]
    pub(super) unsafe fn new(ptr: *mut sqlite3) -> Self {
        Self(NonNull::new_unchecked(ptr))
    }

    #[inline]
    pub(crate) fn as_ptr(&self) -> *mut sqlite3 {
        self.0.as_ptr()
    }

    pub(crate) fn as_non_null_ptr(&self) -> NonNull<sqlite3> {
        self.0
    }

    pub(crate) fn last_insert_rowid(&mut self) -> i64 {
        // SAFETY: we have exclusive access to the database handle
        unsafe { sqlite3_last_insert_rowid(self.as_ptr()) }
    }

    pub(crate) fn last_error(&mut self) -> Option<SqliteError> {
        // SAFETY: we have exclusive access to the database handle
        unsafe { SqliteError::try_new(self.as_ptr()) }
    }

    #[track_caller]
    pub(crate) fn expect_error(&mut self) -> SqliteError {
        self.last_error()
            .expect("expected error code to be set in current context")
    }

    pub(crate) fn exec(&mut self, query: impl Into<String>) -> Result<(), Error> {
        let query = query.into();
        let query = CString::new(query).map_err(|_| err_protocol!("query contains nul bytes"))?;

        // SAFETY: we have exclusive access to the database handle
        unsafe {
            loop {
                let status = sqlite3_exec(
                    self.as_ptr(),
                    query.as_ptr(),
                    // callback if we wanted result rows
                    None,
                    // callback data
                    ptr::null_mut(),
                    // out-pointer for the error message, we just use `SqliteError::new()`
                    ptr::null_mut(),
                );

                match status {
                    SQLITE_OK => return Ok(()),
                    SQLITE_LOCKED_SHAREDCACHE => unlock_notify::wait(self.as_ptr())?,
                    _ => return Err(SqliteError::new(self.as_ptr()).into()),
                }
            }
        }
    }
}

impl Drop for ConnectionHandle {
    fn drop(&mut self) {
        unsafe {
            // https://sqlite.org/c3ref/close.html
            let status = sqlite3_close(self.0.as_ptr());
            if status != SQLITE_OK {
                // this should *only* happen due to an internal bug in SQLite where we left
                // SQLite handles open
                panic!("{}", SqliteError::new(self.0.as_ptr()));
            }
        }
    }
}
