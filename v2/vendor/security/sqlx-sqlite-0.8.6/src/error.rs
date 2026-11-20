use std::error::Error as StdError;
use std::ffi::CStr;
use std::fmt::{self, Display, Formatter};
use std::os::raw::c_int;
use std::{borrow::Cow, str};

use libsqlite3_sys::{
    sqlite3, sqlite3_errmsg, sqlite3_errstr, sqlite3_extended_errcode, SQLITE_CONSTRAINT_CHECK,
    SQLITE_CONSTRAINT_FOREIGNKEY, SQLITE_CONSTRAINT_NOTNULL, SQLITE_CONSTRAINT_PRIMARYKEY,
    SQLITE_CONSTRAINT_UNIQUE, SQLITE_ERROR,
};

pub(crate) use sqlx_core::error::*;

// Error Codes And Messages
// https://www.sqlite.org/c3ref/errcode.html

#[derive(Debug)]
pub struct SqliteError {
    code: c_int,
    message: Cow<'static, str>,
}

impl SqliteError {
    pub(crate) unsafe fn new(handle: *mut sqlite3) -> Self {
        Self::try_new(handle).expect("There should be an error")
    }

    pub(crate) unsafe fn try_new(handle: *mut sqlite3) -> Option<Self> {
        // returns the extended result code even when extended result codes are disabled
        let code: c_int = unsafe { sqlite3_extended_errcode(handle) };

        if code == 0 {
            return None;
        }

        // return English-language text that describes the error
        let message = unsafe {
            let msg = sqlite3_errmsg(handle);
            debug_assert!(!msg.is_null());

            str::from_utf8_unchecked(CStr::from_ptr(msg).to_bytes()).to_owned()
        };

        Some(Self {
            code,
            message: message.into(),
        })
    }

    /// For errors during extension load, the error message is supplied via a separate pointer
    pub(crate) fn with_message(mut self, error_msg: String) -> Self {
        self.message = error_msg.into();
        self
    }

    pub(crate) fn from_code(code: c_int) -> Self {
        let message = unsafe {
            let errstr = sqlite3_errstr(code);

            if !errstr.is_null() {
                // SAFETY: `errstr` is guaranteed to be UTF-8
                // The lifetime of the string is "internally managed";
                // the implementation just selects from an array of static strings.
                // We copy to an owned buffer in case `libsqlite3` is dynamically loaded somehow.
                Cow::Owned(str::from_utf8_unchecked(CStr::from_ptr(errstr).to_bytes()).into())
            } else {
                Cow::Borrowed("<error message unavailable>")
            }
        };

        SqliteError { code, message }
    }

    pub(crate) fn generic(message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code: SQLITE_ERROR,
            message: message.into(),
        }
    }
}

impl Display for SqliteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // We include the code as some produce ambiguous messages:
        // SQLITE_BUSY: "database is locked"
        // SQLITE_LOCKED: "database table is locked"
        // Sadly there's no function to get the string label back from an error code.
        write!(f, "(code: {}) {}", self.code, self.message)
    }
}

impl StdError for SqliteError {}

impl DatabaseError for SqliteError {
    #[inline]
    fn message(&self) -> &str {
        &self.message
    }

    /// The extended result code.
    #[inline]
    fn code(&self) -> Option<Cow<'_, str>> {
        Some(format!("{}", self.code).into())
    }

    #[doc(hidden)]
    fn as_error(&self) -> &(dyn StdError + Send + Sync + 'static) {
        self
    }

    #[doc(hidden)]
    fn as_error_mut(&mut self) -> &mut (dyn StdError + Send + Sync + 'static) {
        self
    }

    #[doc(hidden)]
    fn into_error(self: Box<Self>) -> Box<dyn StdError + Send + Sync + 'static> {
        self
    }

    fn kind(&self) -> ErrorKind {
        match self.code {
            SQLITE_CONSTRAINT_UNIQUE | SQLITE_CONSTRAINT_PRIMARYKEY => ErrorKind::UniqueViolation,
            SQLITE_CONSTRAINT_FOREIGNKEY => ErrorKind::ForeignKeyViolation,
            SQLITE_CONSTRAINT_NOTNULL => ErrorKind::NotNullViolation,
            SQLITE_CONSTRAINT_CHECK => ErrorKind::CheckViolation,
            _ => ErrorKind::Other,
        }
    }
}
