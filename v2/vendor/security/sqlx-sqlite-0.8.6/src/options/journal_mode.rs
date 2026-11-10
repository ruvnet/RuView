use crate::error::Error;
use std::str::FromStr;

/// Refer to [SQLite documentation] for the meaning of the database journaling mode.
///
/// [SQLite documentation]: https://www.sqlite.org/pragma.html#pragma_journal_mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SqliteJournalMode {
    Delete,
    Truncate,
    Persist,
    Memory,
    #[default]
    Wal,
    Off,
}

impl SqliteJournalMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SqliteJournalMode::Delete => "DELETE",
            SqliteJournalMode::Truncate => "TRUNCATE",
            SqliteJournalMode::Persist => "PERSIST",
            SqliteJournalMode::Memory => "MEMORY",
            SqliteJournalMode::Wal => "WAL",
            SqliteJournalMode::Off => "OFF",
        }
    }
}

impl FromStr for SqliteJournalMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Ok(match &*s.to_ascii_lowercase() {
            "delete" => SqliteJournalMode::Delete,
            "truncate" => SqliteJournalMode::Truncate,
            "persist" => SqliteJournalMode::Persist,
            "memory" => SqliteJournalMode::Memory,
            "wal" => SqliteJournalMode::Wal,
            "off" => SqliteJournalMode::Off,

            _ => {
                return Err(Error::Configuration(
                    format!("unknown value {s:?} for `journal_mode`").into(),
                ));
            }
        })
    }
}
