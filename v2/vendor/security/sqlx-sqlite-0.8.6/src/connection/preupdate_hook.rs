use super::SqliteOperation;
use crate::type_info::DataType;
use crate::{SqliteError, SqliteTypeInfo, SqliteValueRef};

use libsqlite3_sys::{
    sqlite3, sqlite3_preupdate_count, sqlite3_preupdate_depth, sqlite3_preupdate_new,
    sqlite3_preupdate_old, sqlite3_value, sqlite3_value_type, SQLITE_OK,
};
use std::ffi::CStr;
use std::marker::PhantomData;
use std::os::raw::{c_char, c_int, c_void};
use std::panic::catch_unwind;
use std::ptr;
use std::ptr::NonNull;

#[derive(Debug, thiserror::Error)]
pub enum PreupdateError {
    /// Error returned from the database.
    #[error("error returned from database: {0}")]
    Database(#[source] SqliteError),
    /// Index is not within the valid column range
    #[error("{0} is not within the valid column range")]
    ColumnIndexOutOfBounds(i32),
    /// Column value accessor was invoked from an invalid operation
    #[error("column value accessor was invoked from an invalid operation")]
    InvalidOperation,
}

pub(crate) struct PreupdateHookHandler(
    pub(super) NonNull<dyn FnMut(PreupdateHookResult) + Send + 'static>,
);
unsafe impl Send for PreupdateHookHandler {}

#[derive(Debug)]
pub struct PreupdateHookResult<'a> {
    pub operation: SqliteOperation,
    pub database: &'a str,
    pub table: &'a str,
    db: *mut sqlite3,
    // The database pointer should not be usable after the preupdate hook.
    // The lifetime on this struct needs to ensure it cannot outlive the callback.
    _db_lifetime: PhantomData<&'a ()>,
    old_row_id: i64,
    new_row_id: i64,
}

impl<'a> PreupdateHookResult<'a> {
    /// Gets the amount of columns in the row being inserted, deleted, or updated.
    pub fn get_column_count(&self) -> i32 {
        unsafe { sqlite3_preupdate_count(self.db) }
    }

    /// Gets the depth of the query that triggered the preupdate hook.
    /// Returns 0 if the preupdate callback was invoked as a result of
    /// a direct insert, update, or delete operation;
    /// 1 for inserts, updates, or deletes invoked by top-level triggers;
    /// 2 for changes resulting from triggers called by top-level triggers; and so forth.
    pub fn get_query_depth(&self) -> i32 {
        unsafe { sqlite3_preupdate_depth(self.db) }
    }

    /// Gets the row id of the row being updated/deleted.
    /// Returns an error if called from an insert operation.
    pub fn get_old_row_id(&self) -> Result<i64, PreupdateError> {
        if self.operation == SqliteOperation::Insert {
            return Err(PreupdateError::InvalidOperation);
        }
        Ok(self.old_row_id)
    }

    /// Gets the row id of the row being inserted/updated.
    /// Returns an error if called from a delete operation.
    pub fn get_new_row_id(&self) -> Result<i64, PreupdateError> {
        if self.operation == SqliteOperation::Delete {
            return Err(PreupdateError::InvalidOperation);
        }
        Ok(self.new_row_id)
    }

    /// Gets the value of the row being updated/deleted at the specified index.
    /// Returns an error if called from an insert operation or the index is out of bounds.
    pub fn get_old_column_value(&self, i: i32) -> Result<SqliteValueRef<'a>, PreupdateError> {
        if self.operation == SqliteOperation::Insert {
            return Err(PreupdateError::InvalidOperation);
        }
        self.validate_column_index(i)?;

        let mut p_value: *mut sqlite3_value = ptr::null_mut();
        unsafe {
            let ret = sqlite3_preupdate_old(self.db, i, &mut p_value);
            self.get_value(ret, p_value)
        }
    }

    /// Gets the value of the row being inserted/updated at the specified index.
    /// Returns an error if called from a delete operation or the index is out of bounds.
    pub fn get_new_column_value(&self, i: i32) -> Result<SqliteValueRef<'a>, PreupdateError> {
        if self.operation == SqliteOperation::Delete {
            return Err(PreupdateError::InvalidOperation);
        }
        self.validate_column_index(i)?;

        let mut p_value: *mut sqlite3_value = ptr::null_mut();
        unsafe {
            let ret = sqlite3_preupdate_new(self.db, i, &mut p_value);
            self.get_value(ret, p_value)
        }
    }

    fn validate_column_index(&self, i: i32) -> Result<(), PreupdateError> {
        if i < 0 || i >= self.get_column_count() {
            return Err(PreupdateError::ColumnIndexOutOfBounds(i));
        }
        Ok(())
    }

    unsafe fn get_value(
        &self,
        ret: i32,
        p_value: *mut sqlite3_value,
    ) -> Result<SqliteValueRef<'a>, PreupdateError> {
        if ret != SQLITE_OK {
            return Err(PreupdateError::Database(SqliteError::new(self.db)));
        }
        let data_type = DataType::from_code(sqlite3_value_type(p_value));
        // SAFETY: SQLite will free the sqlite3_value when the callback returns
        Ok(SqliteValueRef::borrowed(p_value, SqliteTypeInfo(data_type)))
    }
}

pub(super) extern "C" fn preupdate_hook<F>(
    callback: *mut c_void,
    db: *mut sqlite3,
    op_code: c_int,
    database: *const c_char,
    table: *const c_char,
    old_row_id: i64,
    new_row_id: i64,
) where
    F: FnMut(PreupdateHookResult) + Send + 'static,
{
    unsafe {
        let _ = catch_unwind(|| {
            let callback: *mut F = callback.cast::<F>();
            let operation: SqliteOperation = op_code.into();
            let database = CStr::from_ptr(database).to_str().unwrap_or_default();
            let table = CStr::from_ptr(table).to_str().unwrap_or_default();

            (*callback)(PreupdateHookResult {
                operation,
                database,
                table,
                old_row_id,
                new_row_id,
                db,
                _db_lifetime: PhantomData,
            })
        });
    }
}
