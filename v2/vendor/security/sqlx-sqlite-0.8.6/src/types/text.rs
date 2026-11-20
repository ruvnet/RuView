use crate::{Sqlite, SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx_core::decode::Decode;
use sqlx_core::encode::{Encode, IsNull};
use sqlx_core::error::BoxDynError;
use sqlx_core::types::{Text, Type};
use std::fmt::Display;
use std::str::FromStr;

impl<T> Type<Sqlite> for Text<T> {
    fn type_info() -> SqliteTypeInfo {
        <String as Type<Sqlite>>::type_info()
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <String as Type<Sqlite>>::compatible(ty)
    }
}

impl<'q, T> Encode<'q, Sqlite> for Text<T>
where
    T: Display,
{
    fn encode_by_ref(&self, buf: &mut Vec<SqliteArgumentValue<'q>>) -> Result<IsNull, BoxDynError> {
        Encode::<Sqlite>::encode(self.0.to_string(), buf)
    }
}

impl<'r, T> Decode<'r, Sqlite> for Text<T>
where
    T: FromStr,
    BoxDynError: From<<T as FromStr>::Err>,
{
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let s: &str = Decode::<Sqlite>::decode(value)?;
        Ok(Self(s.parse()?))
    }
}
