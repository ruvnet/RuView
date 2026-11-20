use std::ffi::c_void;
use std::ffi::CStr;

use std::os::raw::{c_char, c_int};
use std::ptr;
use std::ptr::NonNull;
use std::slice::from_raw_parts;
use std::str::{from_utf8, from_utf8_unchecked};

use libsqlite3_sys::{
    sqlite3, sqlite3_bind_blob64, sqlite3_bind_double, sqlite3_bind_int, sqlite3_bind_int64,
    sqlite3_bind_null, sqlite3_bind_parameter_count, sqlite3_bind_parameter_name,
    sqlite3_bind_text64, sqlite3_changes, sqlite3_clear_bindings, sqlite3_column_blob,
    sqlite3_column_bytes, sqlite3_column_count, sqlite3_column_database_name,
    sqlite3_column_decltype, sqlite3_column_double, sqlite3_column_int, sqlite3_column_int64,
    sqlite3_column_name, sqlite3_column_origin_name, sqlite3_column_table_name,
    sqlite3_column_type, sqlite3_column_value, sqlite3_db_handle, sqlite3_finalize, sqlite3_reset,
    sqlite3_sql, sqlite3_step, sqlite3_stmt, sqlite3_stmt_readonly, sqlite3_table_column_metadata,
    sqlite3_value, SQLITE_DONE, SQLITE_LOCKED_SHAREDCACHE, SQLITE_MISUSE, SQLITE_OK, SQLITE_ROW,
    SQLITE_TRANSIENT, SQLITE_UTF8,
};

use crate::error::{BoxDynError, Error};
use crate::type_info::DataType;
use crate::{SqliteError, SqliteTypeInfo};

use super::unlock_notify;

#[derive(Debug)]
pub(crate) struct StatementHandle(NonNull<sqlite3_stmt>);

// access to SQLite3 statement handles are safe to send and share between threads
// as long as the `sqlite3_step` call is serialized.

unsafe impl Send for StatementHandle {}

macro_rules! expect_ret_valid {
    ($fn_name:ident($($args:tt)*)) => {{
        let val = $fn_name($($args)*);

        TryFrom::try_from(val)
            // This likely means UB in SQLite itself or our usage of it;
            // signed integer overflow is UB in the C standard.
            .unwrap_or_else(|_| panic!("{}() returned invalid value: {val:?}", stringify!($fn_name)))
    }}
}

macro_rules! check_col_idx {
    ($idx:ident) => {
        c_int::try_from($idx).unwrap_or_else(|_| panic!("invalid column index: {}", $idx))
    };
}

// might use some of this later
#[allow(dead_code)]
impl StatementHandle {
    pub(super) fn new(ptr: NonNull<sqlite3_stmt>) -> Self {
        Self(ptr)
    }

    #[inline]
    pub(super) unsafe fn db_handle(&self) -> *mut sqlite3 {
        // O(c) access to the connection handle for this statement handle
        // https://sqlite.org/c3ref/db_handle.html
        sqlite3_db_handle(self.0.as_ptr())
    }

    pub(crate) fn read_only(&self) -> bool {
        // https://sqlite.org/c3ref/stmt_readonly.html
        unsafe { sqlite3_stmt_readonly(self.0.as_ptr()) != 0 }
    }

    pub(crate) fn sql(&self) -> &str {
        // https://sqlite.org/c3ref/expanded_sql.html
        unsafe {
            let raw = sqlite3_sql(self.0.as_ptr());
            debug_assert!(!raw.is_null());

            from_utf8_unchecked(CStr::from_ptr(raw).to_bytes())
        }
    }

    #[inline]
    pub(crate) fn last_error(&mut self) -> SqliteError {
        unsafe { SqliteError::new(self.db_handle()) }
    }

    #[inline]
    pub(crate) fn column_count(&self) -> usize {
        // https://sqlite.org/c3ref/column_count.html
        unsafe { expect_ret_valid!(sqlite3_column_count(self.0.as_ptr())) }
    }

    #[inline]
    pub(crate) fn changes(&self) -> u64 {
        // returns the number of changes of the *last* statement; not
        // necessarily this statement.
        // https://sqlite.org/c3ref/changes.html
        unsafe { expect_ret_valid!(sqlite3_changes(self.db_handle())) }
    }

    #[inline]
    pub(crate) fn column_name(&self, index: usize) -> &str {
        // https://sqlite.org/c3ref/column_name.html
        unsafe {
            let name = sqlite3_column_name(self.0.as_ptr(), check_col_idx!(index));
            debug_assert!(!name.is_null());

            from_utf8_unchecked(CStr::from_ptr(name).to_bytes())
        }
    }

    pub(crate) fn column_type_info(&self, index: usize) -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::from_code(self.column_type(index)))
    }

    pub(crate) fn column_type_info_opt(&self, index: usize) -> Option<SqliteTypeInfo> {
        match DataType::from_code(self.column_type(index)) {
            DataType::Null => None,
            dt => Some(SqliteTypeInfo(dt)),
        }
    }

    #[inline]
    pub(crate) fn column_decltype(&self, index: usize) -> Option<SqliteTypeInfo> {
        unsafe {
            let decl = sqlite3_column_decltype(self.0.as_ptr(), check_col_idx!(index));
            if decl.is_null() {
                // If the Nth column of the result set is an expression or subquery,
                // then a NULL pointer is returned.
                return None;
            }

            let decl = from_utf8_unchecked(CStr::from_ptr(decl).to_bytes());
            let ty: DataType = decl.parse().ok()?;

            Some(SqliteTypeInfo(ty))
        }
    }

    pub(crate) fn column_nullable(&self, index: usize) -> Result<Option<bool>, Error> {
        unsafe {
            let index = check_col_idx!(index);

            // https://sqlite.org/c3ref/column_database_name.html
            //
            // ### Note
            // The returned string is valid until the prepared statement is destroyed using
            // sqlite3_finalize() or until the statement is automatically reprepared by the
            // first call to sqlite3_step() for a particular run or until the same information
            // is requested again in a different encoding.
            let db_name = sqlite3_column_database_name(self.0.as_ptr(), index);
            let table_name = sqlite3_column_table_name(self.0.as_ptr(), index);
            let origin_name = sqlite3_column_origin_name(self.0.as_ptr(), index);

            if db_name.is_null() || table_name.is_null() || origin_name.is_null() {
                return Ok(None);
            }

            let mut not_null: c_int = 0;

            // https://sqlite.org/c3ref/table_column_metadata.html
            let status = sqlite3_table_column_metadata(
                self.db_handle(),
                db_name,
                table_name,
                origin_name,
                // function docs state to provide NULL for return values you don't care about
                ptr::null_mut(),
                ptr::null_mut(),
                &mut not_null,
                ptr::null_mut(),
                ptr::null_mut(),
            );

            if status != SQLITE_OK {
                // implementation note: the docs for sqlite3_table_column_metadata() specify
                // that an error can be returned if the column came from a view; however,
                // experimentally we found that the above functions give us the true origin
                // for columns in views that came from real tables and so we should never hit this
                // error; for view columns that are expressions we are given NULL for their origins
                // so we don't need special handling for that case either.
                //
                // this is confirmed in the `tests/sqlite-macros.rs` integration test
                return Err(SqliteError::new(self.db_handle()).into());
            }

            Ok(Some(not_null == 0))
        }
    }

    // Number Of SQL Parameters
    #[inline]
    pub(crate) fn bind_parameter_count(&self) -> usize {
        // https://www.sqlite.org/c3ref/bind_parameter_count.html
        unsafe { expect_ret_valid!(sqlite3_bind_parameter_count(self.0.as_ptr())) }
    }

    // Name Of A Host Parameter
    // NOTE: The first host parameter has an index of 1, not 0.
    #[inline]
    pub(crate) fn bind_parameter_name(&self, index: usize) -> Option<&str> {
        unsafe {
            // https://www.sqlite.org/c3ref/bind_parameter_name.html
            let name = sqlite3_bind_parameter_name(self.0.as_ptr(), check_col_idx!(index));
            if name.is_null() {
                return None;
            }

            Some(from_utf8_unchecked(CStr::from_ptr(name).to_bytes()))
        }
    }

    // Binding Values To Prepared Statements
    // https://www.sqlite.org/c3ref/bind_blob.html

    #[inline]
    pub(crate) fn bind_blob(&self, index: usize, v: &[u8]) -> c_int {
        unsafe {
            sqlite3_bind_blob64(
                self.0.as_ptr(),
                check_col_idx!(index),
                v.as_ptr() as *const c_void,
                v.len() as u64,
                SQLITE_TRANSIENT(),
            )
        }
    }

    #[inline]
    pub(crate) fn bind_text(&self, index: usize, v: &str) -> c_int {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let encoding = SQLITE_UTF8 as u8;

        unsafe {
            sqlite3_bind_text64(
                self.0.as_ptr(),
                check_col_idx!(index),
                v.as_ptr() as *const c_char,
                v.len() as u64,
                SQLITE_TRANSIENT(),
                encoding,
            )
        }
    }

    #[inline]
    pub(crate) fn bind_int(&self, index: usize, v: i32) -> c_int {
        unsafe { sqlite3_bind_int(self.0.as_ptr(), check_col_idx!(index), v as c_int) }
    }

    #[inline]
    pub(crate) fn bind_int64(&self, index: usize, v: i64) -> c_int {
        unsafe { sqlite3_bind_int64(self.0.as_ptr(), check_col_idx!(index), v) }
    }

    #[inline]
    pub(crate) fn bind_double(&self, index: usize, v: f64) -> c_int {
        unsafe { sqlite3_bind_double(self.0.as_ptr(), check_col_idx!(index), v) }
    }

    #[inline]
    pub(crate) fn bind_null(&self, index: usize) -> c_int {
        unsafe { sqlite3_bind_null(self.0.as_ptr(), check_col_idx!(index)) }
    }

    // result values from the query
    // https://www.sqlite.org/c3ref/column_blob.html

    #[inline]
    pub(crate) fn column_type(&self, index: usize) -> c_int {
        unsafe { sqlite3_column_type(self.0.as_ptr(), check_col_idx!(index)) }
    }

    #[inline]
    pub(crate) fn column_int(&self, index: usize) -> i32 {
        unsafe { sqlite3_column_int(self.0.as_ptr(), check_col_idx!(index)) as i32 }
    }

    #[inline]
    pub(crate) fn column_int64(&self, index: usize) -> i64 {
        unsafe { sqlite3_column_int64(self.0.as_ptr(), check_col_idx!(index)) as i64 }
    }

    #[inline]
    pub(crate) fn column_double(&self, index: usize) -> f64 {
        unsafe { sqlite3_column_double(self.0.as_ptr(), check_col_idx!(index)) }
    }

    #[inline]
    pub(crate) fn column_value(&self, index: usize) -> *mut sqlite3_value {
        unsafe { sqlite3_column_value(self.0.as_ptr(), check_col_idx!(index)) }
    }

    pub(crate) fn column_blob(&self, index: usize) -> &[u8] {
        let len = unsafe {
            expect_ret_valid!(sqlite3_column_bytes(self.0.as_ptr(), check_col_idx!(index)))
        };

        if len == 0 {
            // empty blobs are NULL so just return an empty slice
            return &[];
        }

        let ptr =
            unsafe { sqlite3_column_blob(self.0.as_ptr(), check_col_idx!(index)) } as *const u8;
        debug_assert!(!ptr.is_null());

        unsafe { from_raw_parts(ptr, len) }
    }

    pub(crate) fn column_text(&self, index: usize) -> Result<&str, BoxDynError> {
        Ok(from_utf8(self.column_blob(index))?)
    }

    pub(crate) fn clear_bindings(&self) {
        unsafe { sqlite3_clear_bindings(self.0.as_ptr()) };
    }

    pub(crate) fn reset(&mut self) -> Result<(), SqliteError> {
        // SAFETY: we have exclusive access to the handle
        unsafe {
            if sqlite3_reset(self.0.as_ptr()) != SQLITE_OK {
                return Err(SqliteError::new(self.db_handle()));
            }
        }

        Ok(())
    }

    pub(crate) fn step(&mut self) -> Result<bool, SqliteError> {
        // SAFETY: we have exclusive access to the handle
        unsafe {
            loop {
                match sqlite3_step(self.0.as_ptr()) {
                    SQLITE_ROW => return Ok(true),
                    SQLITE_DONE => return Ok(false),
                    SQLITE_MISUSE => panic!("misuse!"),
                    SQLITE_LOCKED_SHAREDCACHE => {
                        // The shared cache is locked by another connection. Wait for unlock
                        // notification and try again.
                        unlock_notify::wait(self.db_handle())?;
                        // Need to reset the handle after the unlock
                        // (https://www.sqlite.org/unlock_notify.html)
                        sqlite3_reset(self.0.as_ptr());
                    }
                    _ => return Err(SqliteError::new(self.db_handle())),
                }
            }
        }
    }
}

impl Drop for StatementHandle {
    fn drop(&mut self) {
        // SAFETY: we have exclusive access to the `StatementHandle` here
        unsafe {
            // https://sqlite.org/c3ref/finalize.html
            let status = sqlite3_finalize(self.0.as_ptr());
            if status == SQLITE_MISUSE {
                // Panic in case of detected misuse of SQLite API.
                //
                // sqlite3_finalize returns it at least in the
                // case of detected double free, i.e. calling
                // sqlite3_finalize on already finalized
                // statement.
                panic!("Detected sqlite3_finalize misuse.");
            }
        }
    }
}
