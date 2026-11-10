use std::borrow::Cow;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use futures_channel::oneshot;
use futures_intrusive::sync::{Mutex, MutexGuard};
use tracing::span::Span;

use sqlx_core::describe::Describe;
use sqlx_core::error::Error;
use sqlx_core::transaction::{
    begin_ansi_transaction_sql, commit_ansi_transaction_sql, rollback_ansi_transaction_sql,
};
use sqlx_core::Either;

use crate::connection::describe::describe;
use crate::connection::establish::EstablishParams;
use crate::connection::execute;
use crate::connection::ConnectionState;
use crate::{Sqlite, SqliteArguments, SqliteQueryResult, SqliteRow, SqliteStatement};

use super::serialize::{deserialize, serialize, SchemaName, SqliteOwnedBuf};

// Each SQLite connection has a dedicated thread.

// TODO: Tweak this so that we can use a thread pool per pool of SQLite3 connections to reduce
//       OS resource usage. Low priority because a high concurrent load for SQLite3 is very
//       unlikely.

pub(crate) struct ConnectionWorker {
    command_tx: flume::Sender<(Command, tracing::Span)>,
    /// Mutex for locking access to the database.
    pub(crate) shared: Arc<WorkerSharedState>,
}

pub(crate) struct WorkerSharedState {
    transaction_depth: AtomicUsize,
    cached_statements_size: AtomicUsize,
    pub(crate) conn: Mutex<ConnectionState>,
}

impl WorkerSharedState {
    pub(crate) fn get_transaction_depth(&self) -> usize {
        self.transaction_depth.load(Ordering::Acquire)
    }

    pub(crate) fn get_cached_statements_size(&self) -> usize {
        self.cached_statements_size.load(Ordering::Acquire)
    }
}

enum Command {
    Prepare {
        query: Box<str>,
        tx: oneshot::Sender<Result<SqliteStatement<'static>, Error>>,
    },
    Describe {
        query: Box<str>,
        tx: oneshot::Sender<Result<Describe<Sqlite>, Error>>,
    },
    Execute {
        query: Box<str>,
        arguments: Option<SqliteArguments<'static>>,
        persistent: bool,
        tx: flume::Sender<Result<Either<SqliteQueryResult, SqliteRow>, Error>>,
        limit: Option<usize>,
    },
    Serialize {
        schema: Option<SchemaName>,
        tx: oneshot::Sender<Result<SqliteOwnedBuf, Error>>,
    },
    Deserialize {
        schema: Option<SchemaName>,
        data: SqliteOwnedBuf,
        read_only: bool,
        tx: oneshot::Sender<Result<(), Error>>,
    },
    Begin {
        tx: rendezvous_oneshot::Sender<Result<(), Error>>,
        statement: Option<Cow<'static, str>>,
    },
    Commit {
        tx: rendezvous_oneshot::Sender<Result<(), Error>>,
    },
    Rollback {
        tx: Option<rendezvous_oneshot::Sender<Result<(), Error>>>,
    },
    UnlockDb,
    ClearCache {
        tx: oneshot::Sender<()>,
    },
    Ping {
        tx: oneshot::Sender<()>,
    },
    Shutdown {
        tx: oneshot::Sender<()>,
    },
}

impl ConnectionWorker {
    pub(crate) async fn establish(params: EstablishParams) -> Result<Self, Error> {
        let (establish_tx, establish_rx) = oneshot::channel();

        thread::Builder::new()
            .name(params.thread_name.clone())
            .spawn(move || {
                let (command_tx, command_rx) = flume::bounded(params.command_channel_size);

                let conn = match params.establish() {
                    Ok(conn) => conn,
                    Err(e) => {
                        establish_tx.send(Err(e)).ok();
                        return;
                    }
                };

                let shared = Arc::new(WorkerSharedState {
                    transaction_depth: AtomicUsize::new(0),
                    cached_statements_size: AtomicUsize::new(0),
                    // note: must be fair because in `Command::UnlockDb` we unlock the mutex
                    // and then immediately try to relock it; an unfair mutex would immediately
                    // grant us the lock even if another task is waiting.
                    conn: Mutex::new(conn, true),
                });
                let mut conn = shared.conn.try_lock().unwrap();

                if establish_tx
                    .send(Ok(Self {
                        command_tx,
                        shared: Arc::clone(&shared),
                    }))
                    .is_err()
                {
                    return;
                }

                // If COMMIT or ROLLBACK is processed but not acknowledged, there would be another
                // ROLLBACK sent when the `Transaction` drops. We need to ignore it otherwise we
                // would rollback an already completed transaction.
                let mut ignore_next_start_rollback = false;

                for (cmd, span) in command_rx {
                    let _guard = span.enter();
                    match cmd {
                        Command::Prepare { query, tx } => {
                            tx.send(prepare(&mut conn, &query).map(|prepared| {
                                update_cached_statements_size(
                                    &conn,
                                    &shared.cached_statements_size,
                                );
                                prepared
                            }))
                            .ok();
                        }
                        Command::Describe { query, tx } => {
                            tx.send(describe(&mut conn, &query)).ok();
                        }
                        Command::Execute {
                            query,
                            arguments,
                            persistent,
                            tx,
                            limit
                        } => {
                            let iter = match execute::iter(&mut conn, &query, arguments, persistent)
                            {
                                Ok(iter) => iter,
                                Err(e) => {
                                    tx.send(Err(e)).ok();
                                    continue;
                                }
                            };

                            match limit {
                                None => {
                                    for res in iter {
                                        let has_error = res.is_err();
                                        if tx.send(res).is_err() || has_error {
                                            break;
                                        }
                                    }
                                },
                                Some(limit) => {
                                    let mut iter = iter;
                                    let mut rows_returned = 0;

                                    while let Some(res) = iter.next() {
                                        if let Ok(ok) = &res {
                                            if ok.is_right() {
                                                rows_returned += 1;
                                                if rows_returned >= limit {
                                                    drop(iter);
                                                    let _ = tx.send(res);
                                                    break;
                                                }
                                            }
                                        }
                                        let has_error = res.is_err();
                                        if tx.send(res).is_err() || has_error {
                                            break;
                                        }
                                    }
                                },
                            }

                            update_cached_statements_size(&conn, &shared.cached_statements_size);
                        }
                        Command::Begin { tx, statement } => {
                            let depth = shared.transaction_depth.load(Ordering::Acquire);

                            let statement = match statement {
                                // custom `BEGIN` statements are not allowed if
                                // we're already in a transaction (we need to
                                // issue a `SAVEPOINT` instead)
                                Some(_) if depth > 0 => {
                                    if tx.blocking_send(Err(Error::InvalidSavePointStatement)).is_err() {
                                        break;
                                    }
                                    continue;
                                },
                                Some(statement) => statement,
                                None => begin_ansi_transaction_sql(depth),
                            };
                            let res =
                                conn.handle
                                    .exec(statement)
                                    .map(|_| {
                                        shared.transaction_depth.fetch_add(1, Ordering::Release);
                                    });
                            let res_ok = res.is_ok();

                            if tx.blocking_send(res).is_err() && res_ok {
                                // The BEGIN was processed but not acknowledged. This means no
                                // `Transaction` was created and so there is no way to commit /
                                // rollback this transaction. We need to roll it back
                                // immediately otherwise it would remain started forever.
                                if let Err(error) = conn
                                    .handle
                                    .exec(rollback_ansi_transaction_sql(depth + 1))
                                    .map(|_| {
                                        shared.transaction_depth.fetch_sub(1, Ordering::Release);
                                    })
                                {
                                    // The rollback failed. To prevent leaving the connection
                                    // in an inconsistent state we shutdown this worker which
                                    // causes any subsequent operation on the connection to fail.
                                    tracing::error!(%error, "failed to rollback cancelled transaction");
                                    break;
                                }
                            }
                        }
                        Command::Commit { tx } => {
                            let depth = shared.transaction_depth.load(Ordering::Acquire);

                            let res = if depth > 0 {
                                conn.handle
                                    .exec(commit_ansi_transaction_sql(depth))
                                    .map(|_| {
                                        shared.transaction_depth.fetch_sub(1, Ordering::Release);
                                    })
                            } else {
                                Ok(())
                            };
                            let res_ok = res.is_ok();

                            if tx.blocking_send(res).is_err() && res_ok {
                                // The COMMIT was processed but not acknowledged. This means that
                                // the `Transaction` doesn't know it was committed and will try to
                                // rollback on drop. We need to ignore that rollback.
                                ignore_next_start_rollback = true;
                            }
                        }
                        Command::Rollback { tx } => {
                            if ignore_next_start_rollback && tx.is_none() {
                                ignore_next_start_rollback = false;
                                continue;
                            }

                            let depth = shared.transaction_depth.load(Ordering::Acquire);

                            let res = if depth > 0 {
                                conn.handle
                                    .exec(rollback_ansi_transaction_sql(depth))
                                    .map(|_| {
                                        shared.transaction_depth.fetch_sub(1, Ordering::Release);
                                    })
                            } else {
                                Ok(())
                            };

                            let res_ok = res.is_ok();

                            if let Some(tx) = tx {
                                if tx.blocking_send(res).is_err() && res_ok {
                                    // The ROLLBACK was processed but not acknowledged. This means
                                    // that the `Transaction` doesn't know it was rolled back and
                                    // will try to rollback again on drop. We need to ignore that
                                    // rollback.
                                    ignore_next_start_rollback = true;
                                }
                            }
                        }
                        Command::Serialize { schema, tx } => {
                            tx.send(serialize(&mut conn, schema)).ok();
                        }
                        Command::Deserialize { schema, data, read_only, tx } => {
                            tx.send(deserialize(&mut conn, schema, data, read_only)).ok();
                        }
                        Command::ClearCache { tx } => {
                            conn.statements.clear();
                            update_cached_statements_size(&conn, &shared.cached_statements_size);
                            tx.send(()).ok();
                        }
                        Command::UnlockDb => {
                            drop(conn);
                            conn = futures_executor::block_on(shared.conn.lock());
                        }
                        Command::Ping { tx } => {
                            tx.send(()).ok();
                        }
                        Command::Shutdown { tx } => {
                            // drop the connection references before sending confirmation
                            // and ending the command loop
                            drop(conn);
                            drop(shared);
                            let _ = tx.send(());
                            return;
                        }
                    }
                }
            })?;

        establish_rx.await.map_err(|_| Error::WorkerCrashed)?
    }

    pub(crate) async fn prepare(&mut self, query: &str) -> Result<SqliteStatement<'static>, Error> {
        self.oneshot_cmd(|tx| Command::Prepare {
            query: query.into(),
            tx,
        })
        .await?
    }

    pub(crate) async fn describe(&mut self, query: &str) -> Result<Describe<Sqlite>, Error> {
        self.oneshot_cmd(|tx| Command::Describe {
            query: query.into(),
            tx,
        })
        .await?
    }

    pub(crate) async fn execute(
        &mut self,
        query: &str,
        args: Option<SqliteArguments<'_>>,
        chan_size: usize,
        persistent: bool,
        limit: Option<usize>,
    ) -> Result<flume::Receiver<Result<Either<SqliteQueryResult, SqliteRow>, Error>>, Error> {
        let (tx, rx) = flume::bounded(chan_size);

        self.command_tx
            .send_async((
                Command::Execute {
                    query: query.into(),
                    arguments: args.map(SqliteArguments::into_static),
                    persistent,
                    tx,
                    limit,
                },
                Span::current(),
            ))
            .await
            .map_err(|_| Error::WorkerCrashed)?;

        Ok(rx)
    }

    pub(crate) async fn begin(
        &mut self,
        statement: Option<Cow<'static, str>>,
    ) -> Result<(), Error> {
        self.oneshot_cmd_with_ack(|tx| Command::Begin { tx, statement })
            .await?
    }

    pub(crate) async fn commit(&mut self) -> Result<(), Error> {
        self.oneshot_cmd_with_ack(|tx| Command::Commit { tx })
            .await?
    }

    pub(crate) async fn rollback(&mut self) -> Result<(), Error> {
        self.oneshot_cmd_with_ack(|tx| Command::Rollback { tx: Some(tx) })
            .await?
    }

    pub(crate) fn start_rollback(&mut self) -> Result<(), Error> {
        self.command_tx
            .send((Command::Rollback { tx: None }, Span::current()))
            .map_err(|_| Error::WorkerCrashed)
    }

    pub(crate) async fn ping(&mut self) -> Result<(), Error> {
        self.oneshot_cmd(|tx| Command::Ping { tx }).await
    }

    pub(crate) async fn deserialize(
        &mut self,
        schema: Option<SchemaName>,
        data: SqliteOwnedBuf,
        read_only: bool,
    ) -> Result<(), Error> {
        self.oneshot_cmd(|tx| Command::Deserialize {
            schema,
            data,
            read_only,
            tx,
        })
        .await?
    }

    pub(crate) async fn serialize(
        &mut self,
        schema: Option<SchemaName>,
    ) -> Result<SqliteOwnedBuf, Error> {
        self.oneshot_cmd(|tx| Command::Serialize { schema, tx })
            .await?
    }

    async fn oneshot_cmd<F, T>(&mut self, command: F) -> Result<T, Error>
    where
        F: FnOnce(oneshot::Sender<T>) -> Command,
    {
        let (tx, rx) = oneshot::channel();

        self.command_tx
            .send_async((command(tx), Span::current()))
            .await
            .map_err(|_| Error::WorkerCrashed)?;

        rx.await.map_err(|_| Error::WorkerCrashed)
    }

    async fn oneshot_cmd_with_ack<F, T>(&mut self, command: F) -> Result<T, Error>
    where
        F: FnOnce(rendezvous_oneshot::Sender<T>) -> Command,
    {
        let (tx, rx) = rendezvous_oneshot::channel();

        self.command_tx
            .send_async((command(tx), Span::current()))
            .await
            .map_err(|_| Error::WorkerCrashed)?;

        rx.recv().await.map_err(|_| Error::WorkerCrashed)
    }

    pub(crate) async fn clear_cache(&mut self) -> Result<(), Error> {
        self.oneshot_cmd(|tx| Command::ClearCache { tx }).await
    }

    pub(crate) async fn unlock_db(&mut self) -> Result<MutexGuard<'_, ConnectionState>, Error> {
        let (guard, res) = futures_util::future::join(
            // we need to join the wait queue for the lock before we send the message
            self.shared.conn.lock(),
            self.command_tx
                .send_async((Command::UnlockDb, Span::current())),
        )
        .await;

        res.map_err(|_| Error::WorkerCrashed)?;

        Ok(guard)
    }

    /// Send a command to the worker to shut down the processing thread.
    ///
    /// A `WorkerCrashed` error may be returned if the thread has already stopped.
    pub(crate) fn shutdown(&mut self) -> impl Future<Output = Result<(), Error>> {
        let (tx, rx) = oneshot::channel();

        let send_res = self
            .command_tx
            .send((Command::Shutdown { tx }, Span::current()))
            .map_err(|_| Error::WorkerCrashed);

        async move {
            send_res?;

            // wait for the response
            rx.await.map_err(|_| Error::WorkerCrashed)
        }
    }
}

fn prepare(conn: &mut ConnectionState, query: &str) -> Result<SqliteStatement<'static>, Error> {
    // prepare statement object (or checkout from cache)
    let statement = conn.statements.get(query, true)?;

    let mut parameters = 0;
    let mut columns = None;
    let mut column_names = None;

    while let Some(statement) = statement.prepare_next(&mut conn.handle)? {
        parameters += statement.handle.bind_parameter_count();

        // the first non-empty statement is chosen as the statement we pull columns from
        if !statement.columns.is_empty() && columns.is_none() {
            columns = Some(Arc::clone(statement.columns));
            column_names = Some(Arc::clone(statement.column_names));
        }
    }

    Ok(SqliteStatement {
        sql: Cow::Owned(query.to_string()),
        columns: columns.unwrap_or_default(),
        column_names: column_names.unwrap_or_default(),
        parameters,
    })
}

fn update_cached_statements_size(conn: &ConnectionState, size: &AtomicUsize) {
    size.store(conn.statements.len(), Ordering::Release);
}

// A oneshot channel where send completes only after the receiver receives the value.
mod rendezvous_oneshot {
    use super::oneshot::{self, Canceled};

    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let (inner_tx, inner_rx) = oneshot::channel();
        (Sender { inner: inner_tx }, Receiver { inner: inner_rx })
    }

    pub struct Sender<T> {
        inner: oneshot::Sender<(T, oneshot::Sender<()>)>,
    }

    impl<T> Sender<T> {
        pub async fn send(self, value: T) -> Result<(), Canceled> {
            let (ack_tx, ack_rx) = oneshot::channel();
            self.inner.send((value, ack_tx)).map_err(|_| Canceled)?;
            ack_rx.await
        }

        pub fn blocking_send(self, value: T) -> Result<(), Canceled> {
            futures_executor::block_on(self.send(value))
        }
    }

    pub struct Receiver<T> {
        inner: oneshot::Receiver<(T, oneshot::Sender<()>)>,
    }

    impl<T> Receiver<T> {
        pub async fn recv(self) -> Result<T, Canceled> {
            let (value, ack_tx) = self.inner.await?;
            ack_tx.send(()).map_err(|_| Canceled)?;
            Ok(value)
        }
    }
}
