use crate::error::Error;
use crate::pool::PoolOptions;
use crate::testing::{FixtureSnapshot, TestArgs, TestContext, TestSupport};
use crate::{Sqlite, SqliteConnectOptions};
use futures_core::future::BoxFuture;
use std::path::{Path, PathBuf};

pub(crate) use sqlx_core::testing::*;

const BASE_PATH: &str = "target/sqlx/test-dbs";

impl TestSupport for Sqlite {
    fn test_context(args: &TestArgs) -> BoxFuture<'_, Result<TestContext<Self>, Error>> {
        Box::pin(async move { test_context(args).await })
    }

    fn cleanup_test(db_name: &str) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move { Ok(crate::fs::remove_file(db_name).await?) })
    }

    fn cleanup_test_dbs() -> BoxFuture<'static, Result<Option<usize>, Error>> {
        Box::pin(async move {
            crate::fs::remove_dir_all(BASE_PATH).await?;
            Ok(None)
        })
    }

    fn snapshot(
        _conn: &mut Self::Connection,
    ) -> BoxFuture<'_, Result<FixtureSnapshot<Self>, Error>> {
        todo!()
    }

    fn db_name(args: &TestArgs) -> String {
        convert_path(args.test_path)
    }
}

async fn test_context(args: &TestArgs) -> Result<TestContext<Sqlite>, Error> {
    let db_path = convert_path(args.test_path);

    if let Some(parent_path) = Path::parent(db_path.as_ref()) {
        crate::fs::create_dir_all(parent_path)
            .await
            .expect("failed to create folders");
    }

    if Path::exists(db_path.as_ref()) {
        crate::fs::remove_file(&db_path)
            .await
            .expect("failed to remove database from previous test run");
    }

    Ok(TestContext {
        connect_opts: SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true),
        // This doesn't really matter for SQLite as the databases are independent of each other.
        // The main limitation is going to be the number of concurrent running tests.
        pool_opts: PoolOptions::new().max_connections(1000),
        db_name: db_path,
    })
}

fn convert_path(test_path: &str) -> String {
    let mut path = PathBuf::from(BASE_PATH);

    for segment in test_path.split("::") {
        path.push(segment);
    }

    path.set_extension("sqlite");

    path.into_os_string()
        .into_string()
        .expect("path should be UTF-8")
}

#[test]
fn test_convert_path() {
    let path = convert_path("foo::bar::baz::quux");

    assert_eq!(path, "target/sqlx/test-dbs/foo/bar/baz/quux.sqlite");
}
