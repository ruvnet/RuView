use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::type_info::DataType;
use crate::types::Type;
use crate::{Sqlite, SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};

impl Type<Sqlite> for bool {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Bool)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, DataType::Bool | DataType::Int4 | DataType::Integer)
    }
}

impl<'q> Encode<'q, Sqlite> for bool {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Int((*self).into()));

        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Sqlite> for bool {
    fn decode(value: SqliteValueRef<'r>) -> Result<bool, BoxDynError> {
        Ok(value.int64() != 0)
    }
}
