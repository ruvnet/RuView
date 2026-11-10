//! Integration tests.

use memmap2::{Mmap, MmapMut};
use std::fs::File;
use std::io::{self, Read};
use std::ops::{Deref, DerefMut};

mod examples;
#[cfg(feature = "npz")]
mod npz;
mod primitive;
mod round_trip;

/// A contiguous block of bytes which may be aligned.
pub struct MaybeAlignedBytes {
    buf: Vec<u8>,
    start: usize,
    len: usize,
    align: usize,
}

impl MaybeAlignedBytes {
    /// Returns the number of elements.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if there are no elements.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the alignment value. Note that in the misaligned case, the data
    /// will not have this alignment.
    pub fn maybe_align(&self) -> usize {
        self.align
    }

    /// Returns whether the data is aligned or not.
    pub fn is_aligned(&self) -> bool {
        (self.buf.as_ptr() as usize + self.start) % self.align == 0
    }

    /// Returns zeroed bytes with the specified alignment.
    ///
    /// # Panics
    ///
    /// Panics if `align == 0`.
    pub fn aligned_zeros(len: usize, align: usize) -> MaybeAlignedBytes {
        let buf = vec![0; len + align];
        let start = align - buf.as_ptr() as usize % align;
        let out = MaybeAlignedBytes {
            buf,
            start,
            len,
            align,
        };
        debug_assert!(out.is_aligned());
        out
    }

    /// Returns zeroed bytes which are misaligned with respect to the specified
    /// alignment.
    ///
    /// # Panics
    ///
    /// Panics if `align <= 1`.
    pub fn misaligned_zeros(len: usize, align: usize) -> MaybeAlignedBytes {
        assert!(
            align > 1,
            "`align` must be > 1 in order to create misaligned bytes."
        );
        // Get aligned zeros.
        let mut out = MaybeAlignedBytes::aligned_zeros(len, align);
        // Adjust the start to be misaligned.
        if out.start > 0 {
            out.start -= 1;
        } else {
            out.start += 1;
        }
        debug_assert!(!out.is_aligned());
        debug_assert!(out.buf.len() >= out.start + out.len);
        out
    }

    /// Returns the provided bytes with the specified alignment.
    ///
    /// Copies the data if it's not already properly aligned.
    ///
    /// # Panics
    ///
    /// Panics if `align == 0`.
    pub fn aligned_from_bytes(bytes: Vec<u8>, align: usize) -> MaybeAlignedBytes {
        let len = bytes.len();
        if bytes.as_ptr() as usize % align == 0 {
            MaybeAlignedBytes {
                buf: bytes,
                start: 0,
                len,
                align,
            }
        } else {
            let mut out = MaybeAlignedBytes::aligned_zeros(len, align);
            out.copy_from_slice(&bytes);
            out
        }
    }

    /// Returns the provided bytes misaligned with respect to specified alignment.
    ///
    /// Copies the data if it's not already misaligned.
    ///
    /// # Panics
    ///
    /// Panics if `align <= 1`.
    pub fn misaligned_from_bytes(bytes: Vec<u8>, align: usize) -> MaybeAlignedBytes {
        let len = bytes.len();
        if bytes.as_ptr() as usize % align != 0 {
            MaybeAlignedBytes {
                buf: bytes,
                start: 0,
                len,
                align,
            }
        } else {
            let mut out = MaybeAlignedBytes::misaligned_zeros(len, align);
            out.copy_from_slice(&bytes);
            out
        }
    }
}

impl Deref for MaybeAlignedBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.buf[self.start..self.start + self.len]
    }
}

impl DerefMut for MaybeAlignedBytes {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.buf[self.start..self.start + self.len]
    }
}

macro_rules! impl_file_to_aligned_bytes {
    ($name:ident, $deref:ident, $mmap:path) => {
        /// Returns a value which dereferences to a slice of the file's data
        /// with at least 64-byte alignment.
        ///
        /// This is implemented using `mmap`, except under Miri, which doesn't
        /// support `mmap`. Under Miri, it's implemented by copying the data
        /// into RAM.
        ///
        /// # Safety
        ///
        /// The caller must ensure that the file is not modified until the return value
        /// is dropped.
        ///
        /// # Panics
        ///
        /// May error due to reading or memory-mapping the file.
        pub unsafe fn $name(mut file: &File) -> io::Result<Box<dyn $deref<Target = [u8]>>> {
            const ALIGN: usize = 64;
            if cfg!(miri) {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                Ok(Box::new(MaybeAlignedBytes::aligned_from_bytes(
                    bytes, ALIGN,
                )))
            } else {
                let out = Box::new($mmap(file)?);
                assert_eq!(0, out.as_ptr() as usize % ALIGN);
                Ok(out)
            }
        }
    };
}
impl_file_to_aligned_bytes!(file_to_aligned_bytes, Deref, Mmap::map);
impl_file_to_aligned_bytes!(file_to_aligned_mut_bytes, DerefMut, MmapMut::map_mut);
