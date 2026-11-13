use crate::error::Error;
use std::str::FromStr;

/// Refer to [SQLite documentation] for the meaning of various synchronous settings.
///
/// [SQLite documentation]: https://www.sqlite.org/pragma.html#pragma_synchronous
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SqliteSynchronous {
    Off,
    Normal,
    #[default]
    Full,
    Extra,
}

impl SqliteSynchronous {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SqliteSynchronous::Off => "OFF",
            SqliteSynchronous::Normal => "NORMAL",
            SqliteSynchronous::Full => "FULL",
            SqliteSynchronous::Extra => "EXTRA",
        }
    }
}

impl FromStr for SqliteSynchronous {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Ok(match &*s.to_ascii_lowercase() {
            "off" => SqliteSynchronous::Off,
            "normal" => SqliteSynchronous::Normal,
            "full" => SqliteSynchronous::Full,
            "extra" => SqliteSynchronous::Extra,

            _ => {
                return Err(Error::Configuration(
                    format!("unknown value {s:?} for `synchronous`").into(),
                ));
            }
        })
    }
}
