use crate::encode::{Encode, IsNull};
use crate::error::Error;
use crate::statement::StatementHandle;
use crate::Sqlite;
use atoi::atoi;
use libsqlite3_sys::SQLITE_OK;
use std::borrow::Cow;

pub(crate) use sqlx_core::arguments::*;
use sqlx_core::error::BoxDynError;

#[derive(Debug, Clone)]
pub enum SqliteArgumentValue<'q> {
    Null,
    Text(Cow<'q, str>),
    Blob(Cow<'q, [u8]>),
    Double(f64),
    Int(i32),
    Int64(i64),
}

#[derive(Default, Debug, Clone)]
pub struct SqliteArguments<'q> {
    pub(crate) values: Vec<SqliteArgumentValue<'q>>,
}

impl<'q> SqliteArguments<'q> {
    pub(crate) fn add<T>(&mut self, value: T) -> Result<(), BoxDynError>
    where
        T: Encode<'q, Sqlite>,
    {
        let value_length_before_encoding = self.values.len();

        match value.encode(&mut self.values) {
            Ok(IsNull::Yes) => self.values.push(SqliteArgumentValue::Null),
            Ok(IsNull::No) => {}
            Err(error) => {
                // reset the value buffer to its previous value if encoding failed so we don't leave a half-encoded value behind
                self.values.truncate(value_length_before_encoding);
                return Err(error);
            }
        };

        Ok(())
    }

    pub(crate) fn into_static(self) -> SqliteArguments<'static> {
        SqliteArguments {
            values: self
                .values
                .into_iter()
                .map(SqliteArgumentValue::into_static)
                .collect(),
        }
    }
}

impl<'q> Arguments<'q> for SqliteArguments<'q> {
    type Database = Sqlite;

    fn reserve(&mut self, len: usize, _size_hint: usize) {
        self.values.reserve(len);
    }

    fn add<T>(&mut self, value: T) -> Result<(), BoxDynError>
    where
        T: Encode<'q, Self::Database>,
    {
        self.add(value)
    }

    fn len(&self) -> usize {
        self.values.len()
    }
}

impl SqliteArguments<'_> {
    pub(super) fn bind(&self, handle: &mut StatementHandle, offset: usize) -> Result<usize, Error> {
        let mut arg_i = offset;
        // for handle in &statement.handles {

        let cnt = handle.bind_parameter_count();

        for param_i in 1..=cnt {
            // figure out the index of this bind parameter into our argument tuple
            let n: usize = if let Some(name) = handle.bind_parameter_name(param_i) {
                if let Some(name) = name.strip_prefix('?') {
                    // parameter should have the form ?NNN
                    atoi(name.as_bytes()).expect("parameter of the form ?NNN")
                } else if let Some(name) = name.strip_prefix('$') {
                    // parameter should have the form $NNN
                    atoi(name.as_bytes()).ok_or_else(|| {
                        err_protocol!(
                            "parameters with non-integer names are not currently supported: {}",
                            name
                        )
                    })?
                } else {
                    return Err(err_protocol!("unsupported SQL parameter format: {}", name));
                }
            } else {
                arg_i += 1;
                arg_i
            };

            if n > self.values.len() {
                // SQLite treats unbound variables as NULL
                // we reproduce this here
                // If you are reading this and think this should be an error, open an issue and we can
                // discuss configuring this somehow
                // Note that the query macros have a different way of enforcing
                // argument arity
                break;
            }

            self.values[n - 1].bind(handle, param_i)?;
        }

        Ok(arg_i - offset)
    }
}

impl SqliteArgumentValue<'_> {
    fn into_static(self) -> SqliteArgumentValue<'static> {
        use SqliteArgumentValue::*;

        match self {
            Null => Null,
            Text(text) => Text(text.into_owned().into()),
            Blob(blob) => Blob(blob.into_owned().into()),
            Int(v) => Int(v),
            Int64(v) => Int64(v),
            Double(v) => Double(v),
        }
    }

    fn bind(&self, handle: &mut StatementHandle, i: usize) -> Result<(), Error> {
        use SqliteArgumentValue::*;

        let status = match self {
            Text(v) => handle.bind_text(i, v),
            Blob(v) => handle.bind_blob(i, v),
            Int(v) => handle.bind_int(i, *v),
            Int64(v) => handle.bind_int64(i, *v),
            Double(v) => handle.bind_double(i, *v),
            Null => handle.bind_null(i),
        };

        if status != SQLITE_OK {
            return Err(handle.last_error().into());
        }

        Ok(())
    }
}
