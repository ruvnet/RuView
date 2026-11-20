use std::borrow::Cow;

use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::type_info::DataType;
use crate::types::Type;
use crate::{Sqlite, SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};

impl Type<Sqlite> for str {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Text)
    }
}

impl<'q> Encode<'q, Sqlite> for &'q str {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(Cow::Borrowed(*self)));

        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Sqlite> for &'r str {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        value.text()
    }
}

impl Type<Sqlite> for Box<str> {
    fn type_info() -> SqliteTypeInfo {
        <&str as Type<Sqlite>>::type_info()
    }
}

impl Encode<'_, Sqlite> for Box<str> {
    fn encode(self, args: &mut Vec<SqliteArgumentValue<'_>>) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(Cow::Owned(self.into_string())));

        Ok(IsNull::No)
    }

    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'_>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(Cow::Owned(
            self.clone().into_string(),
        )));

        Ok(IsNull::No)
    }
}

impl Decode<'_, Sqlite> for Box<str> {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        value.text().map(Box::from)
    }
}

impl Type<Sqlite> for String {
    fn type_info() -> SqliteTypeInfo {
        <&str as Type<Sqlite>>::type_info()
    }
}

impl<'q> Encode<'q, Sqlite> for String {
    fn encode(self, args: &mut Vec<SqliteArgumentValue<'q>>) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(Cow::Owned(self)));

        Ok(IsNull::No)
    }

    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(Cow::Owned(self.clone())));

        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Sqlite> for String {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        value.text().map(ToOwned::to_owned)
    }
}

impl Type<Sqlite> for Cow<'_, str> {
    fn type_info() -> SqliteTypeInfo {
        <&str as Type<Sqlite>>::type_info()
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <&str as Type<Sqlite>>::compatible(ty)
    }
}

impl<'q> Encode<'q, Sqlite> for Cow<'q, str> {
    fn encode(self, args: &mut Vec<SqliteArgumentValue<'q>>) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(self));

        Ok(IsNull::No)
    }

    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(self.clone()));

        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Sqlite> for Cow<'r, str> {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        value.text().map(Cow::Borrowed)
    }
}
