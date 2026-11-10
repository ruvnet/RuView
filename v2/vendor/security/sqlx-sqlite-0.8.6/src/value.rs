use std::borrow::Cow;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::slice::from_raw_parts;
use std::str::from_utf8;
use std::sync::Arc;

use libsqlite3_sys::{
    sqlite3_value, sqlite3_value_blob, sqlite3_value_bytes, sqlite3_value_double,
    sqlite3_value_dup, sqlite3_value_free, sqlite3_value_int64, sqlite3_value_type, SQLITE_NULL,
};

pub(crate) use sqlx_core::value::{Value, ValueRef};

use crate::error::BoxDynError;
use crate::type_info::DataType;
use crate::{Sqlite, SqliteTypeInfo};

enum SqliteValueData<'r> {
    Value(&'r SqliteValue),
    BorrowedHandle(ValueHandle<'r>),
}

pub struct SqliteValueRef<'r>(SqliteValueData<'r>);

impl<'r> SqliteValueRef<'r> {
    pub(crate) fn value(value: &'r SqliteValue) -> Self {
        Self(SqliteValueData::Value(value))
    }

    // SAFETY: The supplied sqlite3_value must not be null and SQLite must free it. It will not be freed on drop.
    // The lifetime on this struct should tie it to whatever scope it's valid for before SQLite frees it.
    #[allow(unused)]
    pub(crate) unsafe fn borrowed(value: *mut sqlite3_value, type_info: SqliteTypeInfo) -> Self {
        debug_assert!(!value.is_null());
        let handle = ValueHandle::new_borrowed(NonNull::new_unchecked(value), type_info);
        Self(SqliteValueData::BorrowedHandle(handle))
    }

    // NOTE: `int()` is deliberately omitted because it will silently truncate a wider value,
    // which is likely to cause bugs:
    // https://github.com/launchbadge/sqlx/issues/3179
    // (Similar bug in Postgres): https://github.com/launchbadge/sqlx/issues/3161
    pub(super) fn int64(&self) -> i64 {
        match &self.0 {
            SqliteValueData::Value(v) => v.0.int64(),
            SqliteValueData::BorrowedHandle(v) => v.int64(),
        }
    }

    pub(super) fn double(&self) -> f64 {
        match &self.0 {
            SqliteValueData::Value(v) => v.0.double(),
            SqliteValueData::BorrowedHandle(v) => v.double(),
        }
    }

    pub(super) fn blob(&self) -> &'r [u8] {
        match &self.0 {
            SqliteValueData::Value(v) => v.0.blob(),
            SqliteValueData::BorrowedHandle(v) => v.blob(),
        }
    }

    pub(super) fn text(&self) -> Result<&'r str, BoxDynError> {
        match &self.0 {
            SqliteValueData::Value(v) => v.0.text(),
            SqliteValueData::BorrowedHandle(v) => v.text(),
        }
    }
}

impl<'r> ValueRef<'r> for SqliteValueRef<'r> {
    type Database = Sqlite;

    fn to_owned(&self) -> SqliteValue {
        match &self.0 {
            SqliteValueData::Value(v) => (*v).clone(),
            SqliteValueData::BorrowedHandle(v) => unsafe {
                SqliteValue::new(v.value.as_ptr(), v.type_info.clone())
            },
        }
    }

    fn type_info(&self) -> Cow<'_, SqliteTypeInfo> {
        match &self.0 {
            SqliteValueData::Value(v) => v.type_info(),
            SqliteValueData::BorrowedHandle(v) => v.type_info(),
        }
    }

    fn is_null(&self) -> bool {
        match &self.0 {
            SqliteValueData::Value(v) => v.is_null(),
            SqliteValueData::BorrowedHandle(v) => v.is_null(),
        }
    }
}

#[derive(Clone)]
pub struct SqliteValue(Arc<ValueHandle<'static>>);

pub(crate) struct ValueHandle<'a> {
    value: NonNull<sqlite3_value>,
    type_info: SqliteTypeInfo,
    free_on_drop: bool,
    _sqlite_value_lifetime: PhantomData<&'a ()>,
}

// SAFE: only protected value objects are stored in SqliteValue
unsafe impl<'a> Send for ValueHandle<'a> {}
unsafe impl<'a> Sync for ValueHandle<'a> {}

impl ValueHandle<'static> {
    fn new_owned(value: NonNull<sqlite3_value>, type_info: SqliteTypeInfo) -> Self {
        Self {
            value,
            type_info,
            free_on_drop: true,
            _sqlite_value_lifetime: PhantomData,
        }
    }
}

impl<'a> ValueHandle<'a> {
    fn new_borrowed(value: NonNull<sqlite3_value>, type_info: SqliteTypeInfo) -> Self {
        Self {
            value,
            type_info,
            free_on_drop: false,
            _sqlite_value_lifetime: PhantomData,
        }
    }

    fn type_info_opt(&self) -> Option<SqliteTypeInfo> {
        let dt = DataType::from_code(unsafe { sqlite3_value_type(self.value.as_ptr()) });

        if let DataType::Null = dt {
            None
        } else {
            Some(SqliteTypeInfo(dt))
        }
    }

    fn int64(&self) -> i64 {
        unsafe { sqlite3_value_int64(self.value.as_ptr()) }
    }

    fn double(&self) -> f64 {
        unsafe { sqlite3_value_double(self.value.as_ptr()) }
    }

    fn blob<'b>(&self) -> &'b [u8] {
        let len = unsafe { sqlite3_value_bytes(self.value.as_ptr()) };

        // This likely means UB in SQLite itself or our usage of it;
        // signed integer overflow is UB in the C standard.
        let len = usize::try_from(len).unwrap_or_else(|_| {
            panic!("sqlite3_value_bytes() returned value out of range for usize: {len}")
        });

        if len == 0 {
            // empty blobs are NULL so just return an empty slice
            return &[];
        }

        let ptr = unsafe { sqlite3_value_blob(self.value.as_ptr()) } as *const u8;
        debug_assert!(!ptr.is_null());

        unsafe { from_raw_parts(ptr, len) }
    }

    fn text<'b>(&self) -> Result<&'b str, BoxDynError> {
        Ok(from_utf8(self.blob())?)
    }

    fn type_info(&self) -> Cow<'_, SqliteTypeInfo> {
        self.type_info_opt()
            .map(Cow::Owned)
            .unwrap_or(Cow::Borrowed(&self.type_info))
    }

    fn is_null(&self) -> bool {
        unsafe { sqlite3_value_type(self.value.as_ptr()) == SQLITE_NULL }
    }
}

impl<'a> Drop for ValueHandle<'a> {
    fn drop(&mut self) {
        if self.free_on_drop {
            unsafe {
                sqlite3_value_free(self.value.as_ptr());
            }
        }
    }
}

impl SqliteValue {
    // SAFETY: The sqlite3_value must be non-null and SQLite must not free it. It will be freed on drop.
    pub(crate) unsafe fn new(value: *mut sqlite3_value, type_info: SqliteTypeInfo) -> Self {
        debug_assert!(!value.is_null());
        let handle =
            ValueHandle::new_owned(NonNull::new_unchecked(sqlite3_value_dup(value)), type_info);
        Self(Arc::new(handle))
    }
}

impl Value for SqliteValue {
    type Database = Sqlite;

    fn as_ref(&self) -> SqliteValueRef<'_> {
        SqliteValueRef::value(self)
    }

    fn type_info(&self) -> Cow<'_, SqliteTypeInfo> {
        self.0.type_info()
    }

    fn is_null(&self) -> bool {
        self.0.is_null()
    }
}

// #[cfg(feature = "any")]
// impl<'r> From<SqliteValueRef<'r>> for crate::any::AnyValueRef<'r> {
//     #[inline]
//     fn from(value: SqliteValueRef<'r>) -> Self {
//         crate::any::AnyValueRef {
//             type_info: value.type_info().clone().into_owned().into(),
//             kind: crate::any::value::AnyValueRefKind::Sqlite(value),
//         }
//     }
// }
//
// #[cfg(feature = "any")]
// impl From<SqliteValue> for crate::any::AnyValue {
//     #[inline]
//     fn from(value: SqliteValue) -> Self {
//         crate::any::AnyValue {
//             type_info: value.type_info().clone().into_owned().into(),
//             kind: crate::any::value::AnyValueKind::Sqlite(value),
//         }
//     }
// }
