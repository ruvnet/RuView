//! **SQLite** database driver.
//!
//! ### Note: linkage is semver-exempt.
//! This driver uses the `libsqlite3-sys` crate which links the native library for SQLite 3.
//! With the "sqlite" feature, we enable the `bundled` feature which builds and links SQLite from
//! source.
//!
//! We reserve the right to upgrade the version of `libsqlite3-sys` as necessary to pick up new
//! `3.x.y` versions of SQLite.
//!
//! Due to Cargo's requirement that only one version of a crate that links a given native library
//! exists in the dependency graph at a time, using SQLx alongside another crate linking
//! `libsqlite3-sys` like `rusqlite` is a semver hazard.
//!
//! If you are doing so, we recommend pinning the version of both SQLx and the other crate you're
//! using to prevent a `cargo update` from breaking things, e.g.:
//!
//! ```toml
//! sqlx = { version = "=0.8.1", features = ["sqlite"] }
//! rusqlite = "=0.32.1"
//! ```
//!
//! and then upgrade these crates in lockstep when necessary.
//!
//! ### Dynamic linking
//! To dynamically link to a system SQLite library, the "sqlite-unbundled" feature can be used
//! instead.
//!
//! This allows updating SQLite independently of SQLx or using forked versions, but you must have
//! SQLite installed on the system or provide a path to the library at build time (See
//! [the `rusqlite` README](https://github.com/rusqlite/rusqlite?tab=readme-ov-file#notes-on-building-rusqlite-and-libsqlite3-sys)
//! for details).
//!
//! It may result in link errors if the SQLite version is too old. Version `3.20.0` or newer is
//! recommended. It can increase build time due to the use of bindgen.

// SQLite is a C library. All interactions require FFI which is unsafe.
// All unsafe blocks should have comments pointing to SQLite docs and ensuring that we maintain
// invariants.
#![allow(unsafe_code)]

#[macro_use]
extern crate sqlx_core;

use std::sync::atomic::AtomicBool;

pub use arguments::{SqliteArgumentValue, SqliteArguments};
pub use column::SqliteColumn;
pub use connection::serialize::SqliteOwnedBuf;
#[cfg(feature = "preupdate-hook")]
pub use connection::PreupdateHookResult;
pub use connection::{LockedSqliteHandle, SqliteConnection, SqliteOperation, UpdateHookResult};
pub use database::Sqlite;
pub use error::SqliteError;
pub use options::{
    SqliteAutoVacuum, SqliteConnectOptions, SqliteJournalMode, SqliteLockingMode, SqliteSynchronous,
};
pub use query_result::SqliteQueryResult;
pub use row::SqliteRow;
pub use statement::SqliteStatement;
pub use transaction::SqliteTransactionManager;
pub use type_info::SqliteTypeInfo;
pub use value::{SqliteValue, SqliteValueRef};

use crate::connection::establish::EstablishParams;

pub(crate) use sqlx_core::driver_prelude::*;

use sqlx_core::describe::Describe;
use sqlx_core::error::Error;
use sqlx_core::executor::Executor;

mod arguments;
mod column;
mod connection;
mod database;
mod error;
mod logger;
mod options;
mod query_result;
mod row;
mod statement;
mod transaction;
mod type_checking;
mod type_info;
pub mod types;
mod value;

#[cfg(feature = "any")]
pub mod any;

#[cfg(feature = "regexp")]
mod regexp;

#[cfg(feature = "migrate")]
mod migrate;

#[cfg(feature = "migrate")]
mod testing;

/// An alias for [`Pool`][crate::pool::Pool], specialized for SQLite.
pub type SqlitePool = crate::pool::Pool<Sqlite>;

/// An alias for [`PoolOptions`][crate::pool::PoolOptions], specialized for SQLite.
pub type SqlitePoolOptions = crate::pool::PoolOptions<Sqlite>;

/// An alias for [`Executor<'_, Database = Sqlite>`][Executor].
pub trait SqliteExecutor<'c>: Executor<'c, Database = Sqlite> {}
impl<'c, T: Executor<'c, Database = Sqlite>> SqliteExecutor<'c> for T {}

/// An alias for [`Transaction`][sqlx_core::transaction::Transaction], specialized for SQLite.
pub type SqliteTransaction<'c> = sqlx_core::transaction::Transaction<'c, Sqlite>;

// NOTE: required due to the lack of lazy normalization
impl_into_arguments_for_arguments!(SqliteArguments<'q>);
impl_column_index_for_row!(SqliteRow);
impl_column_index_for_statement!(SqliteStatement);
impl_acquire!(Sqlite, SqliteConnection);

// required because some databases have a different handling of NULL
impl_encode_for_option!(Sqlite);

/// UNSTABLE: for use by `sqlx-cli` only.
#[doc(hidden)]
pub static CREATE_DB_WAL: AtomicBool = AtomicBool::new(true);

/// UNSTABLE: for use by `sqlite-macros-core` only.
#[doc(hidden)]
pub fn describe_blocking(query: &str, database_url: &str) -> Result<Describe<Sqlite>, Error> {
    let opts: SqliteConnectOptions = database_url.parse()?;
    let params = EstablishParams::from_options(&opts)?;
    let mut conn = params.establish()?;

    // Execute any ancillary `PRAGMA`s
    connection::execute::iter(&mut conn, &opts.pragma_string(), None, false)?.finish()?;

    connection::describe::describe(&mut conn, query)

    // SQLite database is closed immediately when `conn` is dropped
}
