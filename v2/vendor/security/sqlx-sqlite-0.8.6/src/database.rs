pub(crate) use sqlx_core::database::{Database, HasStatementCache};

use crate::{
    SqliteArgumentValue, SqliteArguments, SqliteColumn, SqliteConnection, SqliteQueryResult,
    SqliteRow, SqliteStatement, SqliteTransactionManager, SqliteTypeInfo, SqliteValue,
    SqliteValueRef,
};

/// Sqlite database driver.
#[derive(Debug)]
pub struct Sqlite;

impl Database for Sqlite {
    type Connection = SqliteConnection;

    type TransactionManager = SqliteTransactionManager;

    type Row = SqliteRow;

    type QueryResult = SqliteQueryResult;

    type Column = SqliteColumn;

    type TypeInfo = SqliteTypeInfo;

    type Value = SqliteValue;
    type ValueRef<'r> = SqliteValueRef<'r>;

    type Arguments<'q> = SqliteArguments<'q>;
    type ArgumentBuffer<'q> = Vec<SqliteArgumentValue<'q>>;

    type Statement<'q> = SqliteStatement<'q>;

    const NAME: &'static str = "SQLite";

    const URL_SCHEMES: &'static [&'static str] = &["sqlite"];
}

impl HasStatementCache for Sqlite {}
