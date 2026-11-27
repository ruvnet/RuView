use crate::connection::explain::explain;
use crate::connection::ConnectionState;
use crate::describe::Describe;
use crate::error::Error;
use crate::statement::VirtualStatement;
use crate::type_info::DataType;
use crate::{Sqlite, SqliteColumn};
use sqlx_core::Either;
use std::convert::identity;

pub(crate) fn describe(conn: &mut ConnectionState, query: &str) -> Result<Describe<Sqlite>, Error> {
    // describing a statement from SQLite can be involved
    // each SQLx statement is comprised of multiple SQL statements

    let mut statement = VirtualStatement::new(query, false)?;

    let mut columns = Vec::new();
    let mut nullable = Vec::new();
    let mut num_params = 0;

    // we start by finding the first statement that *can* return results
    while let Some(stmt) = statement.prepare_next(&mut conn.handle)? {
        num_params += stmt.handle.bind_parameter_count();

        let mut stepped = false;

        let num = stmt.handle.column_count();
        if num == 0 {
            // no columns in this statement; skip
            continue;
        }

        // next we try to use [column_decltype] to inspect the type of each column
        columns.reserve(num);

        // as a last resort, we explain the original query and attempt to
        // infer what would the expression types be as a fallback
        // to [column_decltype]

        // if explain.. fails, ignore the failure and we'll have no fallback
        let (fallback, fallback_nullable) = match explain(conn, stmt.handle.sql()) {
            Ok(v) => v,
            Err(error) => {
                tracing::debug!(%error, "describe: explain introspection failed");

                (vec![], vec![])
            }
        };

        for col in 0..num {
            let name = stmt.handle.column_name(col).to_owned();

            let type_info = if let Some(ty) = stmt.handle.column_decltype(col) {
                ty
            } else {
                // if that fails, we back up and attempt to step the statement
                // once *if* its read-only and then use [column_type] as a
                // fallback to [column_decltype]
                if !stepped && stmt.handle.read_only() {
                    stepped = true;
                    let _ = stmt.handle.step();
                }

                let mut ty = stmt.handle.column_type_info(col);

                if ty.0 == DataType::Null {
                    if let Some(fallback) = fallback.get(col).cloned() {
                        ty = fallback;
                    }
                }

                ty
            };

            // check explain
            let col_nullable = stmt.handle.column_nullable(col)?;
            let exp_nullable = fallback_nullable.get(col).copied().and_then(identity);

            nullable.push(exp_nullable.or(col_nullable));

            columns.push(SqliteColumn {
                name: name.into(),
                type_info,
                ordinal: col,
            });
        }
    }

    Ok(Describe {
        columns,
        parameters: Some(Either::Right(num_params)),
        nullable,
    })
}
