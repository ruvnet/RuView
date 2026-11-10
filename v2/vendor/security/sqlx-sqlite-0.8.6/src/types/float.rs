use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::type_info::DataType;
use crate::types::Type;
use crate::{Sqlite, SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};

impl Type<Sqlite> for f32 {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Float)
    }
}

impl<'q> Encode<'q, Sqlite> for f32 {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Double((*self).into()));

        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Sqlite> for f32 {
    fn decode(value: SqliteValueRef<'r>) -> Result<f32, BoxDynError> {
        // Truncation is intentional
        #[allow(clippy::cast_possible_truncation)]
        Ok(value.double() as f32)
    }
}

impl Type<Sqlite> for f64 {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Float)
    }
}

impl<'q> Encode<'q, Sqlite> for f64 {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Double(*self));

        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Sqlite> for f64 {
    fn decode(value: SqliteValueRef<'r>) -> Result<f64, BoxDynError> {
        Ok(value.double())
    }
}
