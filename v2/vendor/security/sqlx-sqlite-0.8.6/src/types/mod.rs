//! Conversions between Rust and **SQLite** types.
//!
//! # Types
//!
//! | Rust type                             | SQLite type(s)                                       |
//! |---------------------------------------|------------------------------------------------------|
//! | `bool`                                | BOOLEAN                                              |
//! | `i8`                                  | INTEGER                                              |
//! | `i16`                                 | INTEGER                                              |
//! | `i32`                                 | INTEGER, INT4                                        |
//! | `i64`                                 | BIGINT, INT8                                         |
//! | `u8`                                  | INTEGER                                              |
//! | `u16`                                 | INTEGER                                              |
//! | `u32`                                 | INTEGER                                              |
//! | `u64`                                 | INTEGER (Decode only; see note)                      |
//! | `f32`                                 | REAL                                                 |
//! | `f64`                                 | REAL                                                 |
//! | `&str`, [`String`]                    | TEXT                                                 |
//! | `&[u8]`, `Vec<u8>`                    | BLOB                                                 |
//!
//! #### Note: Unsigned Integers
//! Decoding of unsigned integer types simply performs a checked conversion
//! to ensure that overflow does not occur.
//!
//! Encoding of the unsigned integer types `u8`, `u16` and `u32` is implemented by zero-extending to
//! the next-larger signed type. So `u8` becomes `i16`, `u16` becomes `i32`, and `u32` becomes `i64`
//! while still retaining their semantic values.
//!
//! SQLite stores integers in a variable-width encoding and always handles them in memory as 64-bit
//! signed values, so no space is wasted by this implicit widening.
//!
//! However, there is no corresponding larger type for `u64` in SQLite
//! (it would require a native 16-byte integer, i.e. the equivalent of `i128`),
//! and so encoding is not supported for this type.
//!
//! Bit-casting `u64` to `i64`, or storing it as `REAL`, `BLOB` or `TEXT`,
//! would change the semantics of the value in SQL and so violates the principle of least surprise.
//!
//! ### [`chrono`](https://crates.io/crates/chrono)
//!
//! Requires the `chrono` Cargo feature flag.
//!
//! | Rust type                             | Sqlite type(s)                                       |
//! |---------------------------------------|------------------------------------------------------|
//! | `chrono::NaiveDateTime`               | DATETIME (TEXT, INTEGER, REAL)                       |
//! | `chrono::DateTime<Utc>`               | DATETIME (TEXT, INTEGER, REAL)                       |
//! | `chrono::DateTime<Local>`             | DATETIME (TEXT, INTEGER, REAL)                       |
//! | `chrono::DateTime<FixedOffset>`       | DATETIME (TEXT, INTEGER, REAL)                       |
//! | `chrono::NaiveDate`                   | DATE (TEXT only)                                     |
//! | `chrono::NaiveTime`                   | TIME (TEXT only)                                     |
//!
//! ##### NOTE: `DATETIME` conversions
//! SQLite may represent `DATETIME` values as one of three types: `TEXT`, `REAL`, or `INTEGER`.
//! Which one is used is entirely up to you and how you store timestamps in your database.
//!
//! The deserialization for `NaiveDateTime`, `DateTime<Utc>` and `DateTime<Local>` infer the date
//! format from the type of the value they're being decoded from:
//!
//! * If `TEXT`, the format is assumed to be an ISO-8601 compatible datetime string.
//!   A number of possible formats are tried; see `sqlx-sqlite/src/types/chrono.rs` for the current
//!   set of formats.
//! * If `INTEGER`, it is expected to be the number of seconds since January 1, 1970 00:00 UTC,
//!   as if returned from the `unixepoch()` function (without the `subsec` modifier).
//! * If `REAL`, it is expected to be the (possibly fractional) number of days since the Julian epoch,
//!   November 24, 4714 BCE 12:00 UTC, as if returned from the `julianday()` function.
//!
//! These types will always encode to a datetime string, either
//! with a timezone offset (`DateTime<Tz>` for any `Tz: TimeZone`) or without (`NaiveDateTime`).
//!
//! ##### NOTE: `CURRENT_TIMESTAMP` and comparison/interoperability of `DATETIME` values
//! As stated previously, `DateTime<Tz>` always encodes to a date-time string
//! _with_ a timezone offset,
//! in [RFC 3339 format][::chrono::DateTime::to_rfc3339_opts] (with `use_z: false`).
//!
//! However, most of SQLite's datetime functions
//! (including `datetime()` and `DEFAULT CURRENT_TIMESTAMP`)
//! do not use this format. They instead use `YYYY-MM-DD HH:MM:SS.SSSS` without a timezone offset.
//!
//! This may cause problems with interoperability with other applications, and especially
//! when comparing datetime values, which compares the actual string values lexicographically.
//!
//! Date-time strings in the SQLite format will generally _not_ compare consistently
//! with date-time strings in the RFC 3339 format.
//!
//! We recommend that you decide up-front whether `DATETIME` values should be stored
//! with explicit time zones or not, and use the corresponding type
//! (and its corresponding offset, if applicable) _consistently_ throughout your
//! application:
//!
//! * RFC 3339 format: `DateTime<Tz>` (e.g. `DateTime<Utc>`, `DateTime<Local>`, `DateTime<FixedOffset>`)
//!   * Changing or mixing and matching offsets may break comparisons with existing timestamps.
//!   * `DateTime<Local>` is **not recommended** for portable applications.
//!   * `DateTime<FixedOffset>` is only recommended if the offset is **constant**.
//! * SQLite format: `NaiveDateTime`
//!
//! Note that non-constant offsets may still cause issues when comparing timestamps,
//! as the comparison operators are not timezone-aware.
//!
//! ### [`time`](https://crates.io/crates/time)
//!
//! Requires the `time` Cargo feature flag.
//!
//! | Rust type                             | Sqlite type(s)                                       |
//! |---------------------------------------|------------------------------------------------------|
//! | `time::PrimitiveDateTime`             | DATETIME (TEXT, INTEGER)                             |
//! | `time::OffsetDateTime`                | DATETIME (TEXT, INTEGER)                             |
//! | `time::Date`                          | DATE (TEXT only)                                     |
//! | `time::Time`                          | TIME (TEXT only)                                     |
//!
//! ##### NOTE: `DATETIME` conversions
//! The behavior here is identical to the corresponding `chrono` types, minus the support for `REAL`
//! values as Julian days (it's just not implemented).
//!
//! `PrimitiveDateTime` and `OffsetDateTime` will always encode to a datetime string, either
//! with a timezone offset (`OffsetDateTime`) or without (`PrimitiveDateTime`).
//!
//! ##### NOTE: `CURRENT_TIMESTAMP` and comparison/interoperability of `DATETIME` values
//! As stated previously, `OffsetDateTime` always encodes to a datetime string _with_ a timezone offset,
//! in [RFC 3339 format][::time::format_description::well_known::Rfc3339] (using `Z` for UTC offsets).
//!
//! However, most of SQLite's datetime functions
//! (including `datetime()` and `DEFAULT CURRENT_TIMESTAMP`)
//! do not use this format. They instead use `YYYY-MM-DD HH:MM:SS.SSSS` without a timezone offset.
//!
//! This may cause problems with interoperability with other applications, and especially
//! when comparing datetime values, which compares the actual string values lexicographically.
//!
//! Date-time strings in the SQLite format will generally _not_ compare consistently
//! with date-time strings in the RFC 3339 format.
//!
//! We recommend that you decide up-front whether `DATETIME` values should be stored
//! with explicit time zones or not, and use the corresponding type
//! (and its corresponding offset, if applicable) _consistently_ throughout your
//! application:
//!
//! * RFC 3339 format: `OffsetDateTime` with a **constant** offset.
//!   * Changing or mixing and matching offsets may break comparisons with existing timestamps.
//!   * `OffsetDateTime::now_local()` is **not recommended** for portable applications.
//!   * Non-UTC offsets are only recommended if the offset is **constant**.
//! * SQLite format: `PrimitiveDateTime`
//!
//! Note that non-constant offsets may still cause issues when comparing timestamps,
//! as the comparison operators are not timezone-aware.
//!
//! ### [`uuid`](https://crates.io/crates/uuid)
//!
//! Requires the `uuid` Cargo feature flag.
//!
//! | Rust type                             | Sqlite type(s)                                       |
//! |---------------------------------------|------------------------------------------------------|
//! | `uuid::Uuid`                          | BLOB, TEXT                                           |
//! | `uuid::fmt::Hyphenated`               | TEXT                                                 |
//! | `uuid::fmt::Simple`                   | TEXT                                                 |
//!
//! ### [`json`](https://crates.io/crates/serde_json)
//!
//! Requires the `json` Cargo feature flag.
//!
//! | Rust type                             | Sqlite type(s)                                       |
//! |---------------------------------------|------------------------------------------------------|
//! | [`Json<T>`]                           | TEXT                                                 |
//! | `serde_json::JsonValue`               | TEXT                                                 |
//! | `&serde_json::value::RawValue`        | TEXT                                                 |
//!
//! # Nullable
//!
//! In addition, `Option<T>` is supported where `T` implements `Type`. An `Option<T>` represents
//! a potentially `NULL` value from SQLite.
//!
//! # Non-feature: `NUMERIC` / `rust_decimal` / `bigdecimal` Support
//! Support for mapping `rust_decimal::Decimal` and `bigdecimal::BigDecimal` to SQLite has been
//! deliberately omitted because SQLite does not have native support for high-
//! or arbitrary-precision decimal arithmetic, and to pretend so otherwise would be a
//! significant misstep in API design.
//!
//! The in-tree [`decimal.c`] extension is unfortunately not included in the [amalgamation],
//! which is used to build the bundled version of SQLite3 for `libsqlite3-sys` (which we have
//! enabled by default for the simpler setup experience), otherwise we could support that.
//!
//! The `NUMERIC` type affinity, while seemingly designed for storing decimal values,
//! stores non-integer real numbers as double-precision IEEE-754 floating point,
//! i.e. `REAL` in SQLite, `f64` in Rust, `double` in C/C++, etc.
//!
//! [Datatypes in SQLite: Type Affinity][type-affinity] (accessed 2023/11/20):
//!
//! > A column with NUMERIC affinity may contain values using all five storage classes.
//! > When text data is inserted into a NUMERIC column, the storage class of the text is converted to
//! > INTEGER or REAL (in order of preference) if the text is a well-formed integer or real literal,
//! > respectively. If the TEXT value is a well-formed integer literal that is too large to fit in a
//! > 64-bit signed integer, it is converted to REAL. For conversions between TEXT and REAL storage
//! > classes, only the first 15 significant decimal digits of the number are preserved.
//!
//! With the SQLite3 interactive CLI, we can see that a higher-precision value
//! (20 digits in this case) is rounded off:
//!
//! ```text
//! sqlite> CREATE TABLE foo(bar NUMERIC);
//! sqlite> INSERT INTO foo(bar) VALUES('1.2345678901234567890');
//! sqlite> SELECT * FROM foo;
//! 1.23456789012346
//! ```
//!
//! It appears the `TEXT` storage class is only used if the value contains invalid characters
//! or extra whitespace.
//!
//! Thus, the `NUMERIC` type affinity is **unsuitable** for storage of high-precision decimal values
//! and should be **avoided at all costs**.
//!
//! Support for `rust_decimal` and `bigdecimal` would only be a trap because users will naturally
//! want to use the `NUMERIC` type affinity, and might otherwise encounter serious bugs caused by
//! rounding errors that they were deliberately avoiding when they chose an arbitrary-precision type
//! over a floating-point type in the first place.
//!
//! Instead, you should only use a type affinity that SQLite will not attempt to convert implicitly,
//! such as `TEXT` or `BLOB`, and map values to/from SQLite as strings. You can do this easily
//! using [the `Text` adapter].
//!
//!
//! [`decimal.c`]: https://www.sqlite.org/floatingpoint.html#the_decimal_c_extension
//! [amalgamation]: https://www.sqlite.org/amalgamation.html
//! [type-affinity]: https://www.sqlite.org/datatype3.html#type_affinity
//! [the `Text` adapter]: Text

pub(crate) use sqlx_core::types::*;

mod bool;
mod bytes;
#[cfg(feature = "chrono")]
mod chrono;
mod float;
mod int;
#[cfg(feature = "json")]
mod json;
mod str;
mod text;
#[cfg(feature = "time")]
mod time;
mod uint;
#[cfg(feature = "uuid")]
mod uuid;
