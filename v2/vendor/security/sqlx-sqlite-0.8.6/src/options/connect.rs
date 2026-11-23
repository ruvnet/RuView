use crate::{SqliteConnectOptions, SqliteConnection};
use futures_core::future::BoxFuture;
use log::LevelFilter;
use sqlx_core::connection::ConnectOptions;
use sqlx_core::error::Error;
use sqlx_core::executor::Executor;
use std::fmt::Write;
use std::str::FromStr;
use std::time::Duration;
use url::Url;

impl ConnectOptions for SqliteConnectOptions {
    type Connection = SqliteConnection;

    fn from_url(url: &Url) -> Result<Self, Error> {
        // SQLite URL parsing is handled specially;
        // we want to treat the following URLs as equivalent:
        //
        // * sqlite:foo.db
        // * sqlite://foo.db
        //
        // If we used `Url::path()`, the latter would return an empty string
        // because `foo.db` gets parsed as the hostname.
        Self::from_str(url.as_str())
    }

    fn to_url_lossy(&self) -> Url {
        self.build_url()
    }

    fn connect(&self) -> BoxFuture<'_, Result<Self::Connection, Error>>
    where
        Self::Connection: Sized,
    {
        Box::pin(async move {
            let mut conn = SqliteConnection::establish(self).await?;

            // Execute PRAGMAs
            conn.execute(&*self.pragma_string()).await?;

            if !self.collations.is_empty() {
                let mut locked = conn.lock_handle().await?;

                for collation in &self.collations {
                    collation.create(&mut locked.guard.handle)?;
                }
            }

            Ok(conn)
        })
    }

    fn log_statements(mut self, level: LevelFilter) -> Self {
        self.log_settings.log_statements(level);
        self
    }

    fn log_slow_statements(mut self, level: LevelFilter, duration: Duration) -> Self {
        self.log_settings.log_slow_statements(level, duration);
        self
    }
}

impl SqliteConnectOptions {
    /// Collect all `PRAMGA` commands into a single string
    pub(crate) fn pragma_string(&self) -> String {
        let mut string = String::new();

        for (key, opt_value) in &self.pragmas {
            if let Some(value) = opt_value {
                write!(string, "PRAGMA {key} = {value}; ").ok();
            }
        }

        string
    }
}
