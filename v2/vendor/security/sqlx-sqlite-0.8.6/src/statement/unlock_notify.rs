use std::ffi::c_void;
use std::os::raw::c_int;
use std::slice;
use std::sync::{Condvar, Mutex};

use libsqlite3_sys::{sqlite3, sqlite3_unlock_notify, SQLITE_OK};

use crate::SqliteError;

// Wait for unlock notification (https://www.sqlite.org/unlock_notify.html)
pub unsafe fn wait(conn: *mut sqlite3) -> Result<(), SqliteError> {
    let notify = Notify::new();

    if sqlite3_unlock_notify(
        conn,
        Some(unlock_notify_cb),
        &notify as *const Notify as *mut Notify as *mut _,
    ) != SQLITE_OK
    {
        return Err(SqliteError::new(conn));
    }

    notify.wait();

    Ok(())
}

unsafe extern "C" fn unlock_notify_cb(ptr: *mut *mut c_void, len: c_int) {
    let ptr = ptr as *mut &Notify;
    // We don't have a choice; we can't panic and unwind into FFI here.
    let slice = slice::from_raw_parts(ptr, usize::try_from(len).unwrap_or(0));

    for notify in slice {
        notify.fire();
    }
}

struct Notify {
    mutex: Mutex<bool>,
    condvar: Condvar,
}

impl Notify {
    fn new() -> Self {
        Self {
            mutex: Mutex::new(false),
            condvar: Condvar::new(),
        }
    }

    fn wait(&self) {
        // We only want to wait until the lock is available again.
        #[allow(let_underscore_lock)]
        let _ = self
            .condvar
            .wait_while(self.mutex.lock().unwrap(), |fired| !*fired)
            .unwrap();
    }

    fn fire(&self) {
        let mut lock = self.mutex.lock().unwrap();
        *lock = true;
        self.condvar.notify_one();
    }
}
