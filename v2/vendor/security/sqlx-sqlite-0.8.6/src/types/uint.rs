use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::type_info::DataType;
use crate::types::Type;
use crate::{Sqlite, SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};

impl Type<Sqlite> for u8 {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Int4)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, DataType::Int4 | DataType::Integer)
    }
}

impl<'q> Encode<'q, Sqlite> for u8 {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Int(*self as i32));

        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Sqlite> for u8 {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        // NOTE: using `sqlite3_value_int64()` here because `sqlite3_value_int()` silently truncates
        // which leads to bugs, e.g.:
        // https://github.com/launchbadge/sqlx/issues/3179
        // Similar bug in Postgres: https://github.com/launchbadge/sqlx/issues/3161
        Ok(value.int64().try_into()?)
    }
}

impl Type<Sqlite> for u16 {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Int4)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, DataType::Int4 | DataType::Integer)
    }
}

impl<'q> Encode<'q, Sqlite> for u16 {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Int(*self as i32));

        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Sqlite> for u16 {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int64().try_into()?)
    }
}

impl Type<Sqlite> for u32 {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Integer)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, DataType::Int4 | DataType::Integer)
    }
}

impl<'q> Encode<'q, Sqlite> for u32 {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Int64(*self as i64));

        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Sqlite> for u32 {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int64().try_into()?)
    }
}

impl Type<Sqlite> for u64 {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Integer)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, DataType::Int4 | DataType::Integer)
    }
}

impl<'r> Decode<'r, Sqlite> for u64 {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int64().try_into()?)
    }
}
