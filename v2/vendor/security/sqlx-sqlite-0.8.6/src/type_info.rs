use std::fmt::{self, Display, Formatter};
use std::os::raw::c_int;
use std::str::FromStr;

use libsqlite3_sys::{SQLITE_BLOB, SQLITE_FLOAT, SQLITE_INTEGER, SQLITE_NULL, SQLITE_TEXT};

use crate::error::BoxDynError;

pub(crate) use sqlx_core::type_info::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "offline", derive(serde::Serialize, serde::Deserialize))]
pub(crate) enum DataType {
    // These variants should correspond to `SQLITE_*` type constants.
    Null,
    /// Note: SQLite's type system has no notion of integer widths.
    /// The `INTEGER` type affinity can store up to 8 byte integers,
    /// making `i64` the only safe choice when mapping integer types to Rust.
    Integer,
    Float,
    Text,
    Blob,

    // Explicitly not supported: see documentation in `types/mod.rs`
    #[allow(dead_code)]
    Numeric,

    // non-standard extensions (chosen based on the column's declared type)
    /// Chosen if the column's declared type is `BOOLEAN`.
    Bool,
    /// Chosen if the column's declared type is `INT4`;
    /// instructs the macros to use `i32` instead of `i64`.
    /// Legacy feature; no idea if this is actually used anywhere.
    Int4,
    Date,
    Time,
    Datetime,
}

/// Type information for a SQLite type.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "offline", derive(serde::Serialize, serde::Deserialize))]
pub struct SqliteTypeInfo(pub(crate) DataType);

impl Display for SqliteTypeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

impl TypeInfo for SqliteTypeInfo {
    fn is_null(&self) -> bool {
        matches!(self.0, DataType::Null)
    }

    fn name(&self) -> &str {
        match self.0 {
            DataType::Null => "NULL",
            DataType::Text => "TEXT",
            DataType::Float => "REAL",
            DataType::Blob => "BLOB",
            DataType::Int4 | DataType::Integer => "INTEGER",
            DataType::Numeric => "NUMERIC",

            // non-standard extensions
            DataType::Bool => "BOOLEAN",
            DataType::Date => "DATE",
            DataType::Time => "TIME",
            DataType::Datetime => "DATETIME",
        }
    }
}

impl DataType {
    pub(crate) fn from_code(code: c_int) -> Self {
        match code {
            SQLITE_INTEGER => DataType::Integer,
            SQLITE_FLOAT => DataType::Float,
            SQLITE_BLOB => DataType::Blob,
            SQLITE_NULL => DataType::Null,
            SQLITE_TEXT => DataType::Text,

            // https://sqlite.org/c3ref/c_blob.html
            _ => panic!("unknown data type code {code}"),
        }
    }
}

// note: this implementation is particularly important as this is how the macros determine
//       what Rust type maps to what *declared* SQL type
// <https://www.sqlite.org/datatype3.html#affname>
impl FromStr for DataType {
    type Err = BoxDynError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_ascii_lowercase();
        Ok(match &*s {
            "int4" => DataType::Int4,
            "int8" => DataType::Integer,
            "boolean" | "bool" => DataType::Bool,

            "date" => DataType::Date,
            "time" => DataType::Time,
            "datetime" | "timestamp" => DataType::Datetime,

            _ if s.contains("int") => DataType::Integer,

            _ if s.contains("char") || s.contains("clob") || s.contains("text") => DataType::Text,

            _ if s.contains("blob") => DataType::Blob,

            _ if s.contains("real") || s.contains("floa") || s.contains("doub") => DataType::Float,

            _ => {
                return Err(format!("unknown type: `{s}`").into());
            }
        })
    }
}

// #[cfg(feature = "any")]
// impl From<SqliteTypeInfo> for crate::any::AnyTypeInfo {
//     #[inline]
//     fn from(ty: SqliteTypeInfo) -> Self {
//         crate::any::AnyTypeInfo(crate::any::type_info::AnyTypeInfoKind::Sqlite(ty))
//     }
// }

#[test]
fn test_data_type_from_str() -> Result<(), BoxDynError> {
    assert_eq!(DataType::Int4, "INT4".parse()?);

    assert_eq!(DataType::Integer, "INT".parse()?);
    assert_eq!(DataType::Integer, "INTEGER".parse()?);
    assert_eq!(DataType::Integer, "INTBIG".parse()?);
    assert_eq!(DataType::Integer, "MEDIUMINT".parse()?);

    assert_eq!(DataType::Integer, "BIGINT".parse()?);
    assert_eq!(DataType::Integer, "UNSIGNED BIG INT".parse()?);
    assert_eq!(DataType::Integer, "INT8".parse()?);

    assert_eq!(DataType::Text, "CHARACTER(20)".parse()?);
    assert_eq!(DataType::Text, "NCHAR(55)".parse()?);
    assert_eq!(DataType::Text, "TEXT".parse()?);
    assert_eq!(DataType::Text, "CLOB".parse()?);

    assert_eq!(DataType::Blob, "BLOB".parse()?);

    assert_eq!(DataType::Float, "REAL".parse()?);
    assert_eq!(DataType::Float, "FLOAT".parse()?);
    assert_eq!(DataType::Float, "DOUBLE PRECISION".parse()?);

    assert_eq!(DataType::Bool, "BOOLEAN".parse()?);
    assert_eq!(DataType::Bool, "BOOL".parse()?);

    assert_eq!(DataType::Datetime, "DATETIME".parse()?);
    assert_eq!(DataType::Time, "TIME".parse()?);
    assert_eq!(DataType::Date, "DATE".parse()?);

    Ok(())
}
