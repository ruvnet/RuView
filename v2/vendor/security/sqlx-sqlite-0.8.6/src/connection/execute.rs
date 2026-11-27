use crate::connection::{ConnectionHandle, ConnectionState};
use crate::error::Error;
use crate::logger::QueryLogger;
use crate::statement::{StatementHandle, VirtualStatement};
use crate::{SqliteArguments, SqliteQueryResult, SqliteRow};
use sqlx_core::Either;

pub struct ExecuteIter<'a> {
    handle: &'a mut ConnectionHandle,
    statement: &'a mut VirtualStatement,
    logger: QueryLogger<'a>,
    args: Option<SqliteArguments<'a>>,

    /// since a `VirtualStatement` can encompass multiple actual statements,
    /// this keeps track of the number of arguments so far
    args_used: usize,

    goto_next: bool,
}

pub(crate) fn iter<'a>(
    conn: &'a mut ConnectionState,
    query: &'a str,
    args: Option<SqliteArguments<'a>>,
    persistent: bool,
) -> Result<ExecuteIter<'a>, Error> {
    // fetch the cached statement or allocate a new one
    let statement = conn.statements.get(query, persistent)?;

    let logger = QueryLogger::new(query, conn.log_settings.clone());

    Ok(ExecuteIter {
        handle: &mut conn.handle,
        statement,
        logger,
        args,
        args_used: 0,
        goto_next: true,
    })
}

fn bind(
    statement: &mut StatementHandle,
    arguments: &Option<SqliteArguments<'_>>,
    offset: usize,
) -> Result<usize, Error> {
    let mut n = 0;

    if let Some(arguments) = arguments {
        n = arguments.bind(statement, offset)?;
    }

    Ok(n)
}

impl ExecuteIter<'_> {
    pub fn finish(&mut self) -> Result<(), Error> {
        for res in self {
            let _ = res?;
        }

        Ok(())
    }
}

impl Iterator for ExecuteIter<'_> {
    type Item = Result<Either<SqliteQueryResult, SqliteRow>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let statement = if self.goto_next {
            let statement = match self.statement.prepare_next(self.handle) {
                Ok(Some(statement)) => statement,
                Ok(None) => return None,
                Err(e) => return Some(Err(e)),
            };

            self.goto_next = false;

            // sanity check: ensure the VM is reset and the bindings are cleared
            if let Err(e) = statement.handle.reset() {
                return Some(Err(e.into()));
            }

            statement.handle.clear_bindings();

            match bind(statement.handle, &self.args, self.args_used) {
                Ok(args_used) => self.args_used += args_used,
                Err(e) => return Some(Err(e)),
            }

            statement
        } else {
            self.statement.current()?
        };

        match statement.handle.step() {
            Ok(true) => {
                self.logger.increment_rows_returned();

                Some(Ok(Either::Right(SqliteRow::current(
                    statement.handle,
                    statement.columns,
                    statement.column_names,
                ))))
            }
            Ok(false) => {
                let last_insert_rowid = self.handle.last_insert_rowid();

                let changes = statement.handle.changes();
                self.logger.increase_rows_affected(changes);

                let done = SqliteQueryResult {
                    changes,
                    last_insert_rowid,
                };

                self.goto_next = true;

                Some(Ok(Either::Left(done)))
            }
            Err(e) => Some(Err(e.into())),
        }
    }
}

impl Drop for ExecuteIter<'_> {
    fn drop(&mut self) {
        self.statement.reset().ok();
    }
}
