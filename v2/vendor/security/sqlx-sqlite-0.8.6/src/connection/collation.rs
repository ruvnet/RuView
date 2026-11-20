use std::cmp::Ordering;
use std::ffi::CString;
use std::fmt::{self, Debug, Formatter};
use std::os::raw::{c_int, c_void};
use std::slice;
use std::str::from_utf8_unchecked;
use std::sync::Arc;

use libsqlite3_sys::{sqlite3_create_collation_v2, SQLITE_OK, SQLITE_UTF8};

use crate::connection::handle::ConnectionHandle;
use crate::error::Error;

#[derive(Clone)]
pub struct Collation {
    name: Arc<str>,
    #[allow(clippy::type_complexity)]
    collate: Arc<dyn Fn(&str, &str) -> Ordering + Send + Sync + 'static>,
    // SAFETY: these must match the concrete type of `collate`
    call: unsafe extern "C" fn(
        arg1: *mut c_void,
        arg2: c_int,
        arg3: *const c_void,
        arg4: c_int,
        arg5: *const c_void,
    ) -> c_int,
    free: unsafe extern "C" fn(*mut c_void),
}

impl Collation {
    pub fn new<N, F>(name: N, collate: F) -> Self
    where
        N: Into<Arc<str>>,
        F: Fn(&str, &str) -> Ordering + Send + Sync + 'static,
    {
        unsafe extern "C" fn drop_arc_value<T>(p: *mut c_void) {
            drop(Arc::from_raw(p as *mut T));
        }

        Collation {
            name: name.into(),
            collate: Arc::new(collate),
            call: call_boxed_closure::<F>,
            free: drop_arc_value::<F>,
        }
    }

    pub(crate) fn create(&self, handle: &mut ConnectionHandle) -> Result<(), Error> {
        let raw_f = Arc::into_raw(Arc::clone(&self.collate));
        let c_name = CString::new(&*self.name)
            .map_err(|_| err_protocol!("invalid collation name: {:?}", self.name))?;
        let flags = SQLITE_UTF8;
        let r = unsafe {
            sqlite3_create_collation_v2(
                handle.as_ptr(),
                c_name.as_ptr(),
                flags,
                raw_f as *mut c_void,
                Some(self.call),
                Some(self.free),
            )
        };

        if r == SQLITE_OK {
            Ok(())
        } else {
            // The xDestroy callback is not called if the sqlite3_create_collation_v2() function fails.
            drop(unsafe { Arc::from_raw(raw_f) });
            Err(handle.expect_error().into())
        }
    }
}

impl Debug for Collation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Collation")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

pub(crate) fn create_collation<F>(
    handle: &mut ConnectionHandle,
    name: &str,
    compare: F,
) -> Result<(), Error>
where
    F: Fn(&str, &str) -> Ordering + Send + Sync + 'static,
{
    unsafe extern "C" fn free_boxed_value<T>(p: *mut c_void) {
        drop(Box::from_raw(p as *mut T));
    }

    let boxed_f: *mut F = Box::into_raw(Box::new(compare));
    let c_name =
        CString::new(name).map_err(|_| err_protocol!("invalid collation name: {}", name))?;
    let flags = SQLITE_UTF8;
    let r = unsafe {
        sqlite3_create_collation_v2(
            handle.as_ptr(),
            c_name.as_ptr(),
            flags,
            boxed_f as *mut c_void,
            Some(call_boxed_closure::<F>),
            Some(free_boxed_value::<F>),
        )
    };

    if r == SQLITE_OK {
        Ok(())
    } else {
        // The xDestroy callback is not called if the sqlite3_create_collation_v2() function fails.
        drop(unsafe { Box::from_raw(boxed_f) });
        Err(handle.expect_error().into())
    }
}

unsafe extern "C" fn call_boxed_closure<C>(
    data: *mut c_void,
    left_len: c_int,
    left_ptr: *const c_void,
    right_len: c_int,
    right_ptr: *const c_void,
) -> c_int
where
    C: Fn(&str, &str) -> Ordering,
{
    let boxed_f: *mut C = data as *mut C;

    // Note: unwinding is now caught at the FFI boundary:
    // https://doc.rust-lang.org/nomicon/ffi.html#ffi-and-unwinding
    assert!(!boxed_f.is_null());

    let left_len =
        usize::try_from(left_len).unwrap_or_else(|_| panic!("left_len out of range: {left_len}"));

    let right_len = usize::try_from(right_len)
        .unwrap_or_else(|_| panic!("right_len out of range: {right_len}"));

    let s1 = {
        let c_slice = slice::from_raw_parts(left_ptr as *const u8, left_len);
        from_utf8_unchecked(c_slice)
    };
    let s2 = {
        let c_slice = slice::from_raw_parts(right_ptr as *const u8, right_len);
        from_utf8_unchecked(c_slice)
    };
    let t = (*boxed_f)(s1, s2);

    match t {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}
