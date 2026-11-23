use std::borrow::Cow;
use std::cmp::Ordering;
use std::ffi::CStr;
use std::fmt::Write;
use std::fmt::{self, Debug, Formatter};
use std::os::raw::{c_char, c_int, c_void};
use std::panic::catch_unwind;
use std::ptr;
use std::ptr::NonNull;

use futures_core::future::BoxFuture;
use futures_intrusive::sync::MutexGuard;
use futures_util::future;
use libsqlite3_sys::{
    sqlite3, sqlite3_commit_hook, sqlite3_get_autocommit, sqlite3_progress_handler,
    sqlite3_rollback_hook, sqlite3_update_hook, SQLITE_DELETE, SQLITE_INSERT, SQLITE_UPDATE,
};
#[cfg(feature = "preupdate-hook")]
pub use preupdate_hook::*;

pub(crate) use handle::ConnectionHandle;
use sqlx_core::common::StatementCache;
pub(crate) use sqlx_core::connection::*;
use sqlx_core::error::Error;
use sqlx_core::executor::Executor;
use sqlx_core::transaction::Transaction;

use crate::connection::establish::EstablishParams;
use crate::connection::worker::ConnectionWorker;
use crate::options::OptimizeOnClose;
use crate::statement::VirtualStatement;
use crate::{Sqlite, SqliteConnectOptions, SqliteError};

pub(crate) mod collation;
pub(crate) mod describe;
pub(crate) mod establish;
pub(crate) mod execute;
mod executor;
mod explain;
mod handle;
pub(crate) mod intmap;
#[cfg(feature = "preupdate-hook")]
mod preupdate_hook;
pub(crate) mod serialize;

mod worker;

/// A connection to an open [Sqlite] database.
///
/// Because SQLite is an in-process database accessed by blocking API calls, SQLx uses a background
/// thread and communicates with it via channels to allow non-blocking access to the database.
///
/// Dropping this struct will signal the worker thread to quit and close the database, though
/// if an error occurs there is no way to pass it back to the user this way.
///
/// You can explicitly call [`.close()`][Self::close] to ensure the database is closed successfully
/// or get an error otherwise.
pub struct SqliteConnection {
    optimize_on_close: OptimizeOnClose,
    pub(crate) worker: ConnectionWorker,
    pub(crate) row_channel_size: usize,
}

pub struct LockedSqliteHandle<'a> {
    pub(crate) guard: MutexGuard<'a, ConnectionState>,
}

/// Represents a callback handler that will be shared with the underlying sqlite3 connection.
pub(crate) struct Handler(NonNull<dyn FnMut() -> bool + Send + 'static>);
unsafe impl Send for Handler {}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SqliteOperation {
    Insert,
    Update,
    Delete,
    Unknown(i32),
}

impl From<i32> for SqliteOperation {
    fn from(value: i32) -> Self {
        match value {
            SQLITE_INSERT => SqliteOperation::Insert,
            SQLITE_UPDATE => SqliteOperation::Update,
            SQLITE_DELETE => SqliteOperation::Delete,
            code => SqliteOperation::Unknown(code),
        }
    }
}

pub struct UpdateHookResult<'a> {
    pub operation: SqliteOperation,
    pub database: &'a str,
    pub table: &'a str,
    pub rowid: i64,
}

pub(crate) struct UpdateHookHandler(NonNull<dyn FnMut(UpdateHookResult) + Send + 'static>);
unsafe impl Send for UpdateHookHandler {}

pub(crate) struct CommitHookHandler(NonNull<dyn FnMut() -> bool + Send + 'static>);
unsafe impl Send for CommitHookHandler {}

pub(crate) struct RollbackHookHandler(NonNull<dyn FnMut() + Send + 'static>);
unsafe impl Send for RollbackHookHandler {}

pub(crate) struct ConnectionState {
    pub(crate) handle: ConnectionHandle,

    pub(crate) statements: Statements,

    log_settings: LogSettings,

    /// Stores the progress handler set on the current connection. If the handler returns `false`,
    /// the query is interrupted.
    progress_handler_callback: Option<Handler>,

    update_hook_callback: Option<UpdateHookHandler>,
    #[cfg(feature = "preupdate-hook")]
    preupdate_hook_callback: Option<preupdate_hook::PreupdateHookHandler>,

    commit_hook_callback: Option<CommitHookHandler>,

    rollback_hook_callback: Option<RollbackHookHandler>,
}

impl ConnectionState {
    /// Drops the `progress_handler_callback` if it exists.
    pub(crate) fn remove_progress_handler(&mut self) {
        if let Some(mut handler) = self.progress_handler_callback.take() {
            unsafe {
                sqlite3_progress_handler(self.handle.as_ptr(), 0, None, ptr::null_mut());
                let _ = { Box::from_raw(handler.0.as_mut()) };
            }
        }
    }

    pub(crate) fn remove_update_hook(&mut self) {
        if let Some(mut handler) = self.update_hook_callback.take() {
            unsafe {
                sqlite3_update_hook(self.handle.as_ptr(), None, ptr::null_mut());
                let _ = { Box::from_raw(handler.0.as_mut()) };
            }
        }
    }

    #[cfg(feature = "preupdate-hook")]
    pub(crate) fn remove_preupdate_hook(&mut self) {
        if let Some(mut handler) = self.preupdate_hook_callback.take() {
            unsafe {
                libsqlite3_sys::sqlite3_preupdate_hook(self.handle.as_ptr(), None, ptr::null_mut());
                let _ = { Box::from_raw(handler.0.as_mut()) };
            }
        }
    }

    pub(crate) fn remove_commit_hook(&mut self) {
        if let Some(mut handler) = self.commit_hook_callback.take() {
            unsafe {
                sqlite3_commit_hook(self.handle.as_ptr(), None, ptr::null_mut());
                let _ = { Box::from_raw(handler.0.as_mut()) };
            }
        }
    }

    pub(crate) fn remove_rollback_hook(&mut self) {
        if let Some(mut handler) = self.rollback_hook_callback.take() {
            unsafe {
                sqlite3_rollback_hook(self.handle.as_ptr(), None, ptr::null_mut());
                let _ = { Box::from_raw(handler.0.as_mut()) };
            }
        }
    }
}

pub(crate) struct Statements {
    // cache of semi-persistent statements
    cached: StatementCache<VirtualStatement>,
    // most recent non-persistent statement
    temp: Option<VirtualStatement>,
}

impl SqliteConnection {
    pub(crate) async fn establish(options: &SqliteConnectOptions) -> Result<Self, Error> {
        let params = EstablishParams::from_options(options)?;
        let worker = ConnectionWorker::establish(params).await?;
        Ok(Self {
            optimize_on_close: options.optimize_on_close.clone(),
            worker,
            row_channel_size: options.row_channel_size,
        })
    }

    /// Lock the SQLite database handle out from the worker thread so direct SQLite API calls can
    /// be made safely.
    ///
    /// Returns an error if the worker thread crashed.
    pub async fn lock_handle(&mut self) -> Result<LockedSqliteHandle<'_>, Error> {
        let guard = self.worker.unlock_db().await?;

        Ok(LockedSqliteHandle { guard })
    }
}

impl Debug for SqliteConnection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteConnection")
            .field("row_channel_size", &self.row_channel_size)
            .field("cached_statements_size", &self.cached_statements_size())
            .finish()
    }
}

impl Connection for SqliteConnection {
    type Database = Sqlite;

    type Options = SqliteConnectOptions;

    fn close(mut self) -> BoxFuture<'static, Result<(), Error>> {
        Box::pin(async move {
            if let OptimizeOnClose::Enabled { analysis_limit } = self.optimize_on_close {
                let mut pragma_string = String::new();
                if let Some(limit) = analysis_limit {
                    write!(pragma_string, "PRAGMA analysis_limit = {limit}; ").ok();
                }
                pragma_string.push_str("PRAGMA optimize;");
                self.execute(&*pragma_string).await?;
            }
            let shutdown = self.worker.shutdown();
            // Drop the statement worker, which should
            // cover all references to the connection handle outside of the worker thread
            drop(self);
            // Ensure the worker thread has terminated
            shutdown.await
        })
    }

    fn close_hard(self) -> BoxFuture<'static, Result<(), Error>> {
        Box::pin(async move {
            drop(self);
            Ok(())
        })
    }

    /// Ensure the background worker thread is alive and accepting commands.
    fn ping(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(self.worker.ping())
    }

    fn begin(&mut self) -> BoxFuture<'_, Result<Transaction<'_, Self::Database>, Error>>
    where
        Self: Sized,
    {
        Transaction::begin(self, None)
    }

    fn begin_with(
        &mut self,
        statement: impl Into<Cow<'static, str>>,
    ) -> BoxFuture<'_, Result<Transaction<'_, Self::Database>, Error>>
    where
        Self: Sized,
    {
        Transaction::begin(self, Some(statement.into()))
    }

    fn cached_statements_size(&self) -> usize {
        self.worker.shared.get_cached_statements_size()
    }

    fn clear_cached_statements(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            self.worker.clear_cache().await?;
            Ok(())
        })
    }

    #[inline]
    fn shrink_buffers(&mut self) {
        // No-op.
    }

    #[doc(hidden)]
    fn flush(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        // For SQLite, FLUSH does effectively nothing...
        // Well, we could use this to ensure that the command channel has been cleared,
        // but it would only develop a backlog if a lot of queries are executed and then cancelled
        // partway through, and then this would only make that situation worse.
        Box::pin(future::ok(()))
    }

    #[doc(hidden)]
    fn should_flush(&self) -> bool {
        false
    }
}

/// Implements a C binding to a progress callback. The function returns `0` if the
/// user-provided callback returns `true`, and `1` otherwise to signal an interrupt.
extern "C" fn progress_callback<F>(callback: *mut c_void) -> c_int
where
    F: FnMut() -> bool,
{
    unsafe {
        let r = catch_unwind(|| {
            let callback: *mut F = callback.cast::<F>();
            (*callback)()
        });
        c_int::from(!r.unwrap_or_default())
    }
}

extern "C" fn update_hook<F>(
    callback: *mut c_void,
    op_code: c_int,
    database: *const c_char,
    table: *const c_char,
    rowid: i64,
) where
    F: FnMut(UpdateHookResult),
{
    unsafe {
        let _ = catch_unwind(|| {
            let callback: *mut F = callback.cast::<F>();
            let operation: SqliteOperation = op_code.into();
            let database = CStr::from_ptr(database).to_str().unwrap_or_default();
            let table = CStr::from_ptr(table).to_str().unwrap_or_default();
            (*callback)(UpdateHookResult {
                operation,
                database,
                table,
                rowid,
            })
        });
    }
}

extern "C" fn commit_hook<F>(callback: *mut c_void) -> c_int
where
    F: FnMut() -> bool,
{
    unsafe {
        let r = catch_unwind(|| {
            let callback: *mut F = callback.cast::<F>();
            (*callback)()
        });
        c_int::from(!r.unwrap_or_default())
    }
}

extern "C" fn rollback_hook<F>(callback: *mut c_void)
where
    F: FnMut(),
{
    unsafe {
        let _ = catch_unwind(|| {
            let callback: *mut F = callback.cast::<F>();
            (*callback)()
        });
    }
}

impl LockedSqliteHandle<'_> {
    /// Returns the underlying sqlite3* connection handle.
    ///
    /// As long as this `LockedSqliteHandle` exists, it is guaranteed that the background thread
    /// is not making FFI calls on this database handle or any of its statements.
    ///
    /// ### Note: The `sqlite3` type is semver-exempt.
    /// This API exposes the `sqlite3` type from `libsqlite3-sys` crate for type safety.
    /// However, we reserve the right to upgrade `libsqlite3-sys` as necessary.
    ///
    /// Thus, if you are making direct calls via `libsqlite3-sys` you should pin the version
    /// of SQLx that you're using, and upgrade it and `libsqlite3-sys` manually as new
    /// versions are released.
    ///
    /// See [the driver root docs][crate] for details.
    pub fn as_raw_handle(&mut self) -> NonNull<sqlite3> {
        self.guard.handle.as_non_null_ptr()
    }

    /// Apply a collation to the open database.
    ///
    /// See [`SqliteConnectOptions::collation()`] for details.
    pub fn create_collation(
        &mut self,
        name: &str,
        compare: impl Fn(&str, &str) -> Ordering + Send + Sync + 'static,
    ) -> Result<(), Error> {
        collation::create_collation(&mut self.guard.handle, name, compare)
    }

    /// Sets a progress handler that is invoked periodically during long running calls. If the progress callback
    /// returns `false`, then the operation is interrupted.
    ///
    /// `num_ops` is the approximate number of [virtual machine instructions](https://www.sqlite.org/opcode.html)
    /// that are evaluated between successive invocations of the callback. If `num_ops` is less than one then the
    /// progress handler is disabled.
    ///
    /// Only a single progress handler may be defined at one time per database connection; setting a new progress
    /// handler cancels the old one.
    ///
    /// The progress handler callback must not do anything that will modify the database connection that invoked
    /// the progress handler. Note that sqlite3_prepare_v2() and sqlite3_step() both modify their database connections
    /// in this context.
    pub fn set_progress_handler<F>(&mut self, num_ops: i32, callback: F)
    where
        F: FnMut() -> bool + Send + 'static,
    {
        unsafe {
            let callback_boxed = Box::new(callback);
            // SAFETY: `Box::into_raw()` always returns a non-null pointer.
            let callback = NonNull::new_unchecked(Box::into_raw(callback_boxed));
            let handler = callback.as_ptr() as *mut _;
            self.guard.remove_progress_handler();
            self.guard.progress_handler_callback = Some(Handler(callback));

            sqlite3_progress_handler(
                self.as_raw_handle().as_mut(),
                num_ops,
                Some(progress_callback::<F>),
                handler,
            );
        }
    }

    pub fn set_update_hook<F>(&mut self, callback: F)
    where
        F: FnMut(UpdateHookResult) + Send + 'static,
    {
        unsafe {
            let callback_boxed = Box::new(callback);
            // SAFETY: `Box::into_raw()` always returns a non-null pointer.
            let callback = NonNull::new_unchecked(Box::into_raw(callback_boxed));
            let handler = callback.as_ptr() as *mut _;
            self.guard.remove_update_hook();
            self.guard.update_hook_callback = Some(UpdateHookHandler(callback));

            sqlite3_update_hook(
                self.as_raw_handle().as_mut(),
                Some(update_hook::<F>),
                handler,
            );
        }
    }

    /// Registers a hook that is invoked prior to each `INSERT`, `UPDATE`, and `DELETE` operation on a database table.
    /// At most one preupdate hook may be registered at a time on a single database connection.
    ///
    /// The preupdate hook only fires for changes to real database tables;
    /// it is not invoked for changes to virtual tables or to system tables like sqlite_sequence or sqlite_stat1.
    ///
    /// See https://sqlite.org/c3ref/preupdate_count.html
    #[cfg(feature = "preupdate-hook")]
    pub fn set_preupdate_hook<F>(&mut self, callback: F)
    where
        F: FnMut(PreupdateHookResult) + Send + 'static,
    {
        unsafe {
            let callback_boxed = Box::new(callback);
            // SAFETY: `Box::into_raw()` always returns a non-null pointer.
            let callback = NonNull::new_unchecked(Box::into_raw(callback_boxed));
            let handler = callback.as_ptr() as *mut _;
            self.guard.remove_preupdate_hook();
            self.guard.preupdate_hook_callback = Some(PreupdateHookHandler(callback));

            libsqlite3_sys::sqlite3_preupdate_hook(
                self.as_raw_handle().as_mut(),
                Some(preupdate_hook::<F>),
                handler,
            );
        }
    }

    /// Sets a commit hook that is invoked whenever a transaction is committed. If the commit hook callback
    /// returns `false`, then the operation is turned into a ROLLBACK.
    ///
    /// Only a single commit hook may be defined at one time per database connection; setting a new commit hook
    /// overrides the old one.
    ///
    /// The commit hook callback must not do anything that will modify the database connection that invoked
    /// the commit hook. Note that sqlite3_prepare_v2() and sqlite3_step() both modify their database connections
    /// in this context.
    ///
    /// See https://www.sqlite.org/c3ref/commit_hook.html
    pub fn set_commit_hook<F>(&mut self, callback: F)
    where
        F: FnMut() -> bool + Send + 'static,
    {
        unsafe {
            let callback_boxed = Box::new(callback);
            // SAFETY: `Box::into_raw()` always returns a non-null pointer.
            let callback = NonNull::new_unchecked(Box::into_raw(callback_boxed));
            let handler = callback.as_ptr() as *mut _;
            self.guard.remove_commit_hook();
            self.guard.commit_hook_callback = Some(CommitHookHandler(callback));

            sqlite3_commit_hook(
                self.as_raw_handle().as_mut(),
                Some(commit_hook::<F>),
                handler,
            );
        }
    }

    /// Sets a rollback hook that is invoked whenever a transaction rollback occurs. The rollback callback is not
    /// invoked if a transaction is automatically rolled back because the database connection is closed.
    ///
    /// See https://www.sqlite.org/c3ref/commit_hook.html
    pub fn set_rollback_hook<F>(&mut self, callback: F)
    where
        F: FnMut() + Send + 'static,
    {
        unsafe {
            let callback_boxed = Box::new(callback);
            // SAFETY: `Box::into_raw()` always returns a non-null pointer.
            let callback = NonNull::new_unchecked(Box::into_raw(callback_boxed));
            let handler = callback.as_ptr() as *mut _;
            self.guard.remove_rollback_hook();
            self.guard.rollback_hook_callback = Some(RollbackHookHandler(callback));

            sqlite3_rollback_hook(
                self.as_raw_handle().as_mut(),
                Some(rollback_hook::<F>),
                handler,
            );
        }
    }

    /// Removes the progress handler on a database connection. The method does nothing if no handler was set.
    pub fn remove_progress_handler(&mut self) {
        self.guard.remove_progress_handler();
    }

    pub fn remove_update_hook(&mut self) {
        self.guard.remove_update_hook();
    }

    #[cfg(feature = "preupdate-hook")]
    pub fn remove_preupdate_hook(&mut self) {
        self.guard.remove_preupdate_hook();
    }

    pub fn remove_commit_hook(&mut self) {
        self.guard.remove_commit_hook();
    }

    pub fn remove_rollback_hook(&mut self) {
        self.guard.remove_rollback_hook();
    }

    pub fn last_error(&mut self) -> Option<SqliteError> {
        self.guard.handle.last_error()
    }

    pub(crate) fn in_transaction(&mut self) -> bool {
        let ret = unsafe { sqlite3_get_autocommit(self.as_raw_handle().as_ptr()) };
        ret == 0
    }
}

impl Drop for ConnectionState {
    fn drop(&mut self) {
        // explicitly drop statements before the connection handle is dropped
        self.statements.clear();
        self.remove_progress_handler();
        self.remove_update_hook();
        self.remove_commit_hook();
        self.remove_rollback_hook();
    }
}

impl Statements {
    fn new(capacity: usize) -> Self {
        Statements {
            cached: StatementCache::new(capacity),
            temp: None,
        }
    }

    fn get(&mut self, query: &str, persistent: bool) -> Result<&mut VirtualStatement, Error> {
        if !persistent || !self.cached.is_enabled() {
            return Ok(self.temp.insert(VirtualStatement::new(query, false)?));
        }

        let exists = self.cached.contains_key(query);

        if !exists {
            let statement = VirtualStatement::new(query, true)?;
            self.cached.insert(query, statement);
        }

        let statement = self.cached.get_mut(query).unwrap();

        if exists {
            // as this statement has been executed before, we reset before continuing
            statement.reset()?;
        }

        Ok(statement)
    }

    fn len(&self) -> usize {
        self.cached.len()
    }

    fn clear(&mut self) {
        self.cached.clear();
        self.temp = None;
    }
}
