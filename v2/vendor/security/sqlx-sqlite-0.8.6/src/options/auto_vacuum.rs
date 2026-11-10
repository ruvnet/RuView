use crate::error::Error;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SqliteAutoVacuum {
    #[default]
    None,
    Full,
    Incremental,
}

impl SqliteAutoVacuum {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SqliteAutoVacuum::None => "NONE",
            SqliteAutoVacuum::Full => "FULL",
            SqliteAutoVacuum::Incremental => "INCREMENTAL",
        }
    }
}

impl FromStr for SqliteAutoVacuum {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Ok(match &*s.to_ascii_lowercase() {
            "none" => SqliteAutoVacuum::None,
            "full" => SqliteAutoVacuum::Full,
            "incremental" => SqliteAutoVacuum::Incremental,

            _ => {
                return Err(Error::Configuration(
                    format!("unknown value {s:?} for `auto_vacuum`").into(),
                ));
            }
        })
    }
}
