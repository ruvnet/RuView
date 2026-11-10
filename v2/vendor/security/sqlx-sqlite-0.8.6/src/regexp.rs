#![deny(missing_docs, clippy::pedantic)]
#![allow(clippy::cast_sign_loss)] // some lengths returned from sqlite3 are `i32`, but rust needs `usize`

//! Here be dragons
//!
//! We need to register a custom REGEX implementation for sqlite
//! some useful resources:
//! - rusqlite has an example implementation: <https://docs.rs/rusqlite/0.28.0/rusqlite/functions/index.html>
//! - sqlite supports registering custom C functions: <https://www.sqlite.org/c3ref/create_function.html>
//!   - sqlite also supports a `A REGEXP B` syntax, but ONLY if the user implements `regex(B, A)`
//!   - Note that A and B are indeed swapped: the regex comes first, the field comes second
//!   - <https://www.sqlite.org/lang_expr.html#regexp>
//! - sqlx has a way to safely get a sqlite3 pointer:
//!   - <https://docs.rs/sqlx/0.6.2/sqlx/sqlite/struct.SqliteConnection.html#method.lock_handle>
//!   - <https://docs.rs/sqlx/0.6.2/sqlx/sqlite/struct.LockedSqliteHandle.html#method.as_raw_handle>

use libsqlite3_sys as ffi;
use log::error;
use regex::Regex;
use std::sync::Arc;

/// The function name for sqlite3. This must be "regexp\0"
static FN_NAME: &[u8] = b"regexp\0";

/// Register the regex function with sqlite.
///
/// Returns the result code of `sqlite3_create_function_v2`
pub fn register(sqlite3: *mut ffi::sqlite3) -> i32 {
    unsafe {
        ffi::sqlite3_create_function_v2(
            //  the database connection
            sqlite3,
            // the function name. Must be up to 255 bytes, and 0-terminated
            FN_NAME.as_ptr().cast(),
            // the number of arguments this function accepts. We want 2 arguments: The regex and the field
            2,
            // we want all our strings to be UTF8, and this function will return the same output with the same inputs
            ffi::SQLITE_UTF8 | ffi::SQLITE_DETERMINISTIC,
            // pointer to user data. We're not using user data
            std::ptr::null_mut(),
            // xFunc to be executed when we are invoked
            Some(sqlite3_regexp_func),
            // xStep, should be NULL for scalar functions
            None,
            // xFinal, should be NULL for scalar functions
            None,
            // xDestroy, called when this function is deregistered. Should be used to clean up our pointer to user-data
            None,
        )
    }
}

/// A function to be called on each invocation of `regex(REGEX, FIELD)` from sqlite3
///
/// - `ctx`: a pointer to the current sqlite3 context
/// - `n_arg`: The length of `args`
/// - `args`: the arguments of this function call
unsafe extern "C" fn sqlite3_regexp_func(
    ctx: *mut ffi::sqlite3_context,
    n_arg: i32,
    args: *mut *mut ffi::sqlite3_value,
) {
    // check the arg size. sqlite3 should already ensure this is only 2 args but we want to double check
    if n_arg != 2 {
        eprintln!("n_arg expected to be 2, is {n_arg}");
        ffi::sqlite3_result_error_code(ctx, ffi::SQLITE_CONSTRAINT_FUNCTION);
        return;
    }

    // arg0: Regex
    let Some(regex) = get_regex_from_arg(ctx, *args.offset(0), 0) else {
        return;
    };

    // arg1: value
    let Some(value) = get_text_from_arg(ctx, *args.offset(1)) else {
        return;
    };

    // if the regex matches the value, set the result int as 1, else as 0
    if regex.is_match(value) {
        ffi::sqlite3_result_int(ctx, 1);
    } else {
        ffi::sqlite3_result_int(ctx, 0);
    }
}

/// Get the regex from the given `arg` at the given `index`.
///
/// First this will check to see if the value exists in sqlite's `auxdata`. If it does, that regex will be returned.
/// sqlite is able to clean up this data at any point, but rust's [`Arc`] guarantees make sure things don't break.
///
/// If this value does not exist in `auxdata`, [`try_load_value`] is called and a regex is created from this. If any of
/// those fail, a message is printed and `None` is returned.
///
/// After this regex is created it is stored in `auxdata` and loaded again. If it fails to load, this means that
/// something inside of sqlite3 went wrong, and we return `None`.
///
/// If this value is stored correctly, or if it already existed, the arc reference counter is increased and this value is returned.
unsafe fn get_regex_from_arg(
    ctx: *mut ffi::sqlite3_context,
    arg: *mut ffi::sqlite3_value,
    index: i32,
) -> Option<Arc<Regex>> {
    // try to get the auxdata for this field
    let ptr = ffi::sqlite3_get_auxdata(ctx, index);
    if !ptr.is_null() {
        // if we have it, turn it into an Arc.
        // we need to make sure to call `increment_strong_count` because the returned `Arc` decrement this when it goes out of scope
        let ptr = ptr as *const Regex;
        Arc::increment_strong_count(ptr);
        return Some(Arc::from_raw(ptr));
    }
    // get the text for this field
    let value = get_text_from_arg(ctx, arg)?;
    // try to compile it into a regex
    let regex = match Regex::new(value) {
        Ok(regex) => Arc::new(regex),
        Err(e) => {
            error!("Invalid regex {value:?}: {e:?}");
            ffi::sqlite3_result_error_code(ctx, ffi::SQLITE_CONSTRAINT_FUNCTION);
            return None;
        }
    };
    // set the regex as auxdata for the next time around
    ffi::sqlite3_set_auxdata(
        ctx,
        index,
        // make sure to call `Arc::clone` here, setting the strong count to 2.
        // this will be cleaned up at 2 points:
        // - when the returned arc goes out of scope
        // - when sqlite decides to clean it up an calls `cleanup_arc_regex_pointer`
        Arc::into_raw(Arc::clone(&regex)) as *mut _,
        Some(cleanup_arc_regex_pointer),
    );
    Some(regex)
}

/// Get a text reference of the value of `arg`. If this value is not a string value, an error is printed and `None` is
/// returned.
///
/// The returned `&str` is valid for lifetime `'a` which can be determined by the caller. This lifetime should **not**
/// outlive `ctx`.
unsafe fn get_text_from_arg<'a>(
    ctx: *mut ffi::sqlite3_context,
    arg: *mut ffi::sqlite3_value,
) -> Option<&'a str> {
    let ty = ffi::sqlite3_value_type(arg);
    if ty == ffi::SQLITE_TEXT {
        let ptr = ffi::sqlite3_value_text(arg);
        let len = ffi::sqlite3_value_bytes(arg);
        let slice = std::slice::from_raw_parts(ptr.cast(), len as usize);
        match std::str::from_utf8(slice) {
            Ok(result) => Some(result),
            Err(e) => {
                log::error!("Incoming text is not valid UTF8: {e:?}");
                ffi::sqlite3_result_error_code(ctx, ffi::SQLITE_CONSTRAINT_FUNCTION);
                None
            }
        }
    } else {
        None
    }
}

/// Clean up the `Arc<Regex>` that is stored in the given `ptr`.
unsafe extern "C" fn cleanup_arc_regex_pointer(ptr: *mut std::ffi::c_void) {
    Arc::decrement_strong_count(ptr.cast::<Regex>());
}

#[cfg(test)]
mod tests {
    use sqlx::{ConnectOptions, Row};
    use std::str::FromStr;

    async fn test_db() -> crate::SqliteConnection {
        let mut conn = crate::SqliteConnectOptions::from_str("sqlite://:memory:")
            .unwrap()
            .with_regexp()
            .connect()
            .await
            .unwrap();
        sqlx::query("CREATE TABLE test (col TEXT NOT NULL)")
            .execute(&mut conn)
            .await
            .unwrap();
        for i in 0..10 {
            sqlx::query("INSERT INTO test VALUES (?)")
                .bind(format!("value {i}"))
                .execute(&mut conn)
                .await
                .unwrap();
        }
        conn
    }

    #[sqlx::test]
    async fn test_regexp_does_not_fail() {
        let mut conn = test_db().await;
        let result = sqlx::query("SELECT col FROM test WHERE col REGEXP 'foo.*bar'")
            .fetch_all(&mut conn)
            .await
            .expect("Could not execute query");
        assert!(result.is_empty());
    }

    #[sqlx::test]
    async fn test_regexp_filters_correctly() {
        let mut conn = test_db().await;

        let result = sqlx::query("SELECT col FROM test WHERE col REGEXP '.*2'")
            .fetch_all(&mut conn)
            .await
            .expect("Could not execute query");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].get::<String, usize>(0), String::from("value 2"));

        let result = sqlx::query("SELECT col FROM test WHERE col REGEXP '^3'")
            .fetch_all(&mut conn)
            .await
            .expect("Could not execute query");
        assert!(result.is_empty());
    }

    #[sqlx::test]
    async fn test_invalid_regexp_should_fail() {
        let mut conn = test_db().await;
        let result = sqlx::query("SELECT col from test WHERE col REGEXP '(?:?)'")
            .execute(&mut conn)
            .await;
        assert!(matches!(result, Err(sqlx::Error::Database(_))));
    }
}
