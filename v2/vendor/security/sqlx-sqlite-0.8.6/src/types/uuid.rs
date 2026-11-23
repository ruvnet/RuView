use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::type_info::DataType;
use crate::types::Type;
use crate::{Sqlite, SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use std::borrow::Cow;
use uuid::{
    fmt::{Hyphenated, Simple},
    Uuid,
};

impl Type<Sqlite> for Uuid {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Blob)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, DataType::Blob | DataType::Text)
    }
}

impl<'q> Encode<'q, Sqlite> for Uuid {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Blob(Cow::Owned(
            self.as_bytes().to_vec(),
        )));

        Ok(IsNull::No)
    }
}

impl Decode<'_, Sqlite> for Uuid {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        // construct a Uuid from the returned bytes
        Uuid::from_slice(value.blob()).map_err(Into::into)
    }
}

impl Type<Sqlite> for Hyphenated {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Text)
    }
}

impl<'q> Encode<'q, Sqlite> for Hyphenated {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(Cow::Owned(self.to_string())));

        Ok(IsNull::No)
    }
}

impl Decode<'_, Sqlite> for Hyphenated {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let uuid: Result<Uuid, BoxDynError> =
            Uuid::parse_str(&value.text().map(ToOwned::to_owned)?).map_err(Into::into);

        Ok(uuid?.hyphenated())
    }
}

impl Type<Sqlite> for Simple {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Text)
    }
}

impl<'q> Encode<'q, Sqlite> for Simple {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(Cow::Owned(self.to_string())));

        Ok(IsNull::No)
    }
}

impl Decode<'_, Sqlite> for Simple {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let uuid: Result<Uuid, BoxDynError> =
            Uuid::parse_str(&value.text().map(ToOwned::to_owned)?).map_err(Into::into);

        Ok(uuid?.simple())
    }
}
