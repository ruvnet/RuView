use super::ConnectionState;
use crate::{error::Error, SqliteConnection, SqliteError};
use libsqlite3_sys::{
    sqlite3_deserialize, sqlite3_free, sqlite3_malloc64, sqlite3_serialize,
    SQLITE_DESERIALIZE_FREEONCLOSE, SQLITE_DESERIALIZE_READONLY, SQLITE_DESERIALIZE_RESIZEABLE,
    SQLITE_NOMEM, SQLITE_OK,
};
use std::ffi::c_char;
use std::fmt::Debug;
use std::{
    ops::{Deref, DerefMut},
    ptr,
    ptr::NonNull,
};

impl SqliteConnection {
    /// Serialize the given SQLite database schema using [`sqlite3_serialize()`].
    ///
    /// The returned buffer is a SQLite managed allocation containing the equivalent data
    /// as writing the database to disk. It is freed on-drop.
    ///  
    /// To serialize the primary, unqualified schema (`main`), pass `None` for the schema name.
    ///
    /// # Errors
    /// * [`Error::InvalidArgument`] if the schema name contains a zero/NUL byte (`\0`).
    /// * [`Error::Database`] if the schema does not exist or another error occurs.
    ///
    /// [`sqlite3_serialize()`]: https://sqlite.org/c3ref/serialize.html
    pub async fn serialize(&mut self, schema: Option<&str>) -> Result<SqliteOwnedBuf, Error> {
        let schema = schema.map(SchemaName::try_from).transpose()?;

        self.worker.serialize(schema).await
    }

    /// Deserialize a SQLite database from a buffer into the specified schema using [`sqlite3_deserialize()`].
    ///
    /// The given schema will be disconnected and re-connected as an in-memory database
    /// backed by `data`, which should be the serialized form of a database previously returned
    /// by a call to [`Self::serialize()`], documented as being equivalent to
    /// the contents of the database file on disk.
    ///
    /// An error will be returned if a schema with the given name is not already attached.  
    /// You can use `ATTACH ':memory' as "<schema name>"` to create an empty schema first.
    ///
    /// Pass `None` to deserialize to the primary, unqualified schema (`main`).
    ///
    /// The SQLite connection will take ownership of `data` and will free it when the connection
    /// is closed or the schema is detached ([`SQLITE_DESERIALIZE_FREEONCLOSE`][deserialize-flags]).
    ///
    /// If `read_only` is `true`, the schema is opened as read-only ([`SQLITE_DESERIALIZE_READONLY`][deserialize-flags]).  
    /// If `false`, the schema is marked as resizable ([`SQLITE_DESERIALIZE_RESIZABLE`][deserialize-flags]).
    ///
    /// If the database is in WAL mode, an error is returned.
    /// See [`sqlite3_deserialize()`] for details.
    ///
    /// # Errors
    /// * [`Error::InvalidArgument`] if the schema name contains a zero/NUL byte (`\0`).
    /// * [`Error::Database`] if an error occurs during deserialization.
    ///
    /// [`sqlite3_deserialize()`]: https://sqlite.org/c3ref/deserialize.html
    /// [deserialize-flags]: https://sqlite.org/c3ref/c_deserialize_freeonclose.html
    pub async fn deserialize(
        &mut self,
        schema: Option<&str>,
        data: SqliteOwnedBuf,
        read_only: bool,
    ) -> Result<(), Error> {
        let schema = schema.map(SchemaName::try_from).transpose()?;

        self.worker.deserialize(schema, data, read_only).await
    }
}

pub(crate) fn serialize(
    conn: &mut ConnectionState,
    schema: Option<SchemaName>,
) -> Result<SqliteOwnedBuf, Error> {
    let mut size = 0;

    let buf = unsafe {
        let ptr = sqlite3_serialize(
            conn.handle.as_ptr(),
            schema.as_ref().map_or(ptr::null(), SchemaName::as_ptr),
            &mut size,
            0,
        );

        // looking at the source, `sqlite3_serialize` actually sets `size = -1` on error:
        // https://github.com/sqlite/sqlite/blob/da5f81387843f92652128087a8f8ecef0b79461d/src/memdb.c#L776
        usize::try_from(size)
            .ok()
            .and_then(|size| SqliteOwnedBuf::from_raw(ptr, size))
    };

    if let Some(buf) = buf {
        return Ok(buf);
    }

    if let Some(error) = conn.handle.last_error() {
        return Err(error.into());
    }

    if size > 0 {
        // If `size` is positive but `sqlite3_serialize` still returned NULL,
        // the most likely culprit is an out-of-memory condition.
        return Err(SqliteError::from_code(SQLITE_NOMEM).into());
    }

    // Otherwise, the schema was probably not found.
    // We return the equivalent error as when you try to execute `PRAGMA <schema>.page_count`
    // against a non-existent schema.
    Err(SqliteError::generic(format!(
        "database {} does not exist",
        schema.as_ref().map_or("main", SchemaName::as_str)
    ))
    .into())
}

pub(crate) fn deserialize(
    conn: &mut ConnectionState,
    schema: Option<SchemaName>,
    data: SqliteOwnedBuf,
    read_only: bool,
) -> Result<(), Error> {
    // SQLITE_DESERIALIZE_FREEONCLOSE causes SQLite to take ownership of the buffer
    let mut flags = SQLITE_DESERIALIZE_FREEONCLOSE;
    if read_only {
        flags |= SQLITE_DESERIALIZE_READONLY;
    } else {
        flags |= SQLITE_DESERIALIZE_RESIZEABLE;
    }

    let (buf, size) = data.into_raw();

    let rc = unsafe {
        sqlite3_deserialize(
            conn.handle.as_ptr(),
            schema.as_ref().map_or(ptr::null(), SchemaName::as_ptr),
            buf,
            i64::try_from(size).unwrap(),
            i64::try_from(size).unwrap(),
            flags,
        )
    };

    match rc {
        SQLITE_OK => Ok(()),
        SQLITE_NOMEM => Err(SqliteError::from_code(SQLITE_NOMEM).into()),
        // SQLite unfortunately doesn't set any specific message for deserialization errors.
        _ => Err(SqliteError::generic("an error occurred during deserialization").into()),
    }
}

/// Memory buffer owned and allocated by SQLite. Freed on drop.
///
/// Intended primarily for use with [`SqliteConnection::serialize()`] and [`SqliteConnection::deserialize()`].
///
/// Can be created from `&[u8]` using the `TryFrom` impl. The slice must not be empty.
#[derive(Debug)]
pub struct SqliteOwnedBuf {
    ptr: NonNull<u8>,
    size: usize,
}

unsafe impl Send for SqliteOwnedBuf {}
unsafe impl Sync for SqliteOwnedBuf {}

impl Drop for SqliteOwnedBuf {
    fn drop(&mut self) {
        unsafe {
            sqlite3_free(self.ptr.as_ptr().cast());
        }
    }
}

impl SqliteOwnedBuf {
    /// Uses `sqlite3_malloc` to allocate a buffer and returns a pointer to it.
    ///
    /// # Safety
    /// The allocated buffer is uninitialized.
    unsafe fn with_capacity(size: usize) -> Option<SqliteOwnedBuf> {
        let ptr = sqlite3_malloc64(u64::try_from(size).unwrap()).cast::<u8>();
        Self::from_raw(ptr, size)
    }

    /// Creates a new mem buffer from a pointer that has been created with sqlite_malloc
    ///
    /// # Safety:
    /// * The pointer must point to a valid allocation created by `sqlite3_malloc()`, or `NULL`.
    unsafe fn from_raw(ptr: *mut u8, size: usize) -> Option<Self> {
        Some(Self {
            ptr: NonNull::new(ptr)?,
            size,
        })
    }

    fn into_raw(self) -> (*mut u8, usize) {
        let raw = (self.ptr.as_ptr(), self.size);
        // this is used in sqlite_deserialize and
        // underlying buffer must not be freed
        std::mem::forget(self);
        raw
    }
}

/// # Errors
/// Returns [`Error::InvalidArgument`] if the slice is empty.
impl TryFrom<&[u8]> for SqliteOwnedBuf {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        unsafe {
            // SAFETY: `buf` is not initialized until `ptr::copy_nonoverlapping` completes.
            let mut buf = Self::with_capacity(bytes.len()).ok_or_else(|| {
                Error::InvalidArgument("SQLite owned buffer cannot be empty".to_string())
            })?;
            ptr::copy_nonoverlapping(bytes.as_ptr(), buf.ptr.as_mut(), buf.size);
            Ok(buf)
        }
    }
}

impl Deref for SqliteOwnedBuf {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.size) }
    }
}

impl DerefMut for SqliteOwnedBuf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_mut(), self.size) }
    }
}

impl AsRef<[u8]> for SqliteOwnedBuf {
    fn as_ref(&self) -> &[u8] {
        self.deref()
    }
}

impl AsMut<[u8]> for SqliteOwnedBuf {
    fn as_mut(&mut self) -> &mut [u8] {
        self.deref_mut()
    }
}

/// Checked schema name to pass to SQLite.
///
/// # Safety:
/// * Valid UTF-8 (not guaranteed by `CString`)
/// * No internal zero bytes (`\0`) (not guaranteed by `String`)
/// * Terminated with a zero byte (`\0`) (not guaranteed by `String`)
#[derive(Debug)]
pub(crate) struct SchemaName(Box<str>);

impl SchemaName {
    /// Get the schema name as a string without the zero byte terminator.
    pub fn as_str(&self) -> &str {
        &self.0[..self.0.len() - 1]
    }

    /// Get a pointer to the string data, suitable for passing as C's `*const char`.
    ///
    /// # Safety
    /// The string data is guaranteed to be terminated with a zero byte.
    pub fn as_ptr(&self) -> *const c_char {
        self.0.as_ptr() as *const c_char
    }
}

impl<'a> TryFrom<&'a str> for SchemaName {
    type Error = Error;

    fn try_from(name: &'a str) -> Result<Self, Self::Error> {
        // SAFETY: we must ensure that the string does not contain an internal NULL byte
        if let Some(pos) = name.as_bytes().iter().position(|&b| b == 0) {
            return Err(Error::InvalidArgument(format!(
                "schema name {name:?} contains a zero byte at index {pos}"
            )));
        }

        let capacity = name.len().checked_add(1).unwrap();

        let mut s = String::new();
        // `String::with_capacity()` does not guarantee that it will not overallocate,
        // which might mean an unnecessary reallocation to make `capacity == len`
        // in the conversion to `Box<str>`.
        s.reserve_exact(capacity);

        s.push_str(name);
        s.push('\0');

        Ok(SchemaName(s.into()))
    }
}
