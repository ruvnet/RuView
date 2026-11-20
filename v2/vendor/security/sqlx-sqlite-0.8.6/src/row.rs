#![allow(clippy::rc_buffer)]

use std::sync::Arc;

use sqlx_core::column::ColumnIndex;
use sqlx_core::error::Error;
use sqlx_core::ext::ustr::UStr;
use sqlx_core::row::Row;
use sqlx_core::HashMap;

use crate::statement::StatementHandle;
use crate::{Sqlite, SqliteColumn, SqliteValue, SqliteValueRef};

/// Implementation of [`Row`] for SQLite.
pub struct SqliteRow {
    pub(crate) values: Box<[SqliteValue]>,
    pub(crate) columns: Arc<Vec<SqliteColumn>>,
    pub(crate) column_names: Arc<HashMap<UStr, usize>>,
}

// Accessing values from the statement object is
// safe across threads as long as we don't call [sqlite3_step]

// we block ourselves from doing that by only exposing
// a set interface on [StatementHandle]

unsafe impl Send for SqliteRow {}
unsafe impl Sync for SqliteRow {}

impl SqliteRow {
    pub(crate) fn current(
        statement: &StatementHandle,
        columns: &Arc<Vec<SqliteColumn>>,
        column_names: &Arc<HashMap<UStr, usize>>,
    ) -> Self {
        let size = statement.column_count();
        let mut values = Vec::with_capacity(size);

        for i in 0..size {
            values.push(unsafe {
                let raw = statement.column_value(i);

                SqliteValue::new(raw, columns[i].type_info.clone())
            });
        }

        Self {
            values: values.into_boxed_slice(),
            columns: Arc::clone(columns),
            column_names: Arc::clone(column_names),
        }
    }
}

impl Row for SqliteRow {
    type Database = Sqlite;

    fn columns(&self) -> &[SqliteColumn] {
        &self.columns
    }

    fn try_get_raw<I>(&self, index: I) -> Result<SqliteValueRef<'_>, Error>
    where
        I: ColumnIndex<Self>,
    {
        let index = index.index(self)?;
        Ok(SqliteValueRef::value(&self.values[index]))
    }
}

impl ColumnIndex<SqliteRow> for &'_ str {
    fn index(&self, row: &SqliteRow) -> Result<usize, Error> {
        row.column_names
            .get(*self)
            .ok_or_else(|| Error::ColumnNotFound((*self).into()))
            .copied()
    }
}

// #[cfg(feature = "any")]
// impl From<SqliteRow> for crate::any::AnyRow {
//     #[inline]
//     fn from(row: SqliteRow) -> Self {
//         crate::any::AnyRow {
//             columns: row.columns.iter().map(|col| col.clone().into()).collect(),
//             kind: crate::any::row::AnyRowKind::Sqlite(row),
//         }
//     }
// }
