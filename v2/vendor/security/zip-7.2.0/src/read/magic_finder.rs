use std::io::{Read, Seek, SeekFrom};

use memchr::memmem::{Finder, FinderRev};

use crate::result::ZipResult;

pub trait FinderDirection<'a> {
    fn new(needle: &'a [u8]) -> Self;
    fn reset_cursor(bounds: (u64, u64), window_size: usize) -> u64;
    fn scope_window(window: &[u8], mid_window_offset: usize) -> (&[u8], usize);

    fn needle(&self) -> &[u8];
    fn find(&self, haystack: &[u8]) -> Option<usize>;
    fn move_cursor(&self, cursor: u64, bounds: (u64, u64), window_size: usize) -> Option<u64>;
    fn move_scope(&self, offset: usize) -> usize;
}

pub struct Forward<'a>(Finder<'a>);
impl<'a> FinderDirection<'a> for Forward<'a> {
    fn new(needle: &'a [u8]) -> Self {
        Self(Finder::new(needle))
    }

    fn reset_cursor((start_inclusive, _): (u64, u64), _: usize) -> u64 {
        start_inclusive
    }

    fn scope_window(window: &[u8], mid_window_offset: usize) -> (&[u8], usize) {
        (&window[mid_window_offset..], mid_window_offset)
    }

    fn find(&self, haystack: &[u8]) -> Option<usize> {
        self.0.find(haystack)
    }

    fn needle(&self) -> &[u8] {
        self.0.needle()
    }

    fn move_cursor(&self, cursor: u64, bounds: (u64, u64), window_size: usize) -> Option<u64> {
        let magic_overlap = self.needle().len().saturating_sub(1) as u64;
        let next = cursor.saturating_add(window_size as u64 - magic_overlap);

        if next >= bounds.1 {
            None
        } else {
            Some(next)
        }
    }

    fn move_scope(&self, offset: usize) -> usize {
        offset + self.needle().len()
    }
}

pub struct Backwards<'a>(FinderRev<'a>);
impl<'a> FinderDirection<'a> for Backwards<'a> {
    fn new(needle: &'a [u8]) -> Self {
        Self(FinderRev::new(needle))
    }

    fn reset_cursor(bounds: (u64, u64), window_size: usize) -> u64 {
        bounds
            .1
            .saturating_sub(window_size as u64)
            .clamp(bounds.0, bounds.1)
    }

    fn scope_window(window: &[u8], mid_window_offset: usize) -> (&[u8], usize) {
        (&window[..mid_window_offset], 0)
    }

    fn find(&self, haystack: &[u8]) -> Option<usize> {
        self.0.rfind(haystack)
    }

    fn needle(&self) -> &[u8] {
        self.0.needle()
    }

    fn move_cursor(&self, cursor: u64, bounds: (u64, u64), window_size: usize) -> Option<u64> {
        let magic_overlap = self.needle().len().saturating_sub(1) as u64;

        if cursor <= bounds.0 {
            None
        } else {
            Some(
                cursor
                    .saturating_add(magic_overlap)
                    .saturating_sub(window_size as u64)
                    .clamp(bounds.0, bounds.1),
            )
        }
    }

    fn move_scope(&self, offset: usize) -> usize {
        offset
    }
}

/// A utility for finding magic symbols from the end of a seekable reader.
///
/// Can be repurposed to recycle the internal buffer.
pub struct MagicFinder<Direction> {
    buffer: Box<[u8]>,
    pub(self) finder: Direction,
    cursor: u64,
    mid_buffer_offset: Option<usize>,
    bounds: (u64, u64),
}

impl<'a, T: FinderDirection<'a>> MagicFinder<T> {
    /// Create a new magic bytes finder to look within specific bounds.
    pub fn new(magic_bytes: &'a [u8], start_inclusive: u64, end_exclusive: u64) -> Self {
        const BUFFER_SIZE: usize = 2048;

        // Smaller buffer size would be unable to locate bytes.
        // Equal buffer size would stall (the window could not be moved).
        debug_assert!(BUFFER_SIZE >= magic_bytes.len());

        Self {
            buffer: vec![0; BUFFER_SIZE].into_boxed_slice(),
            finder: T::new(magic_bytes),
            cursor: T::reset_cursor((start_inclusive, end_exclusive), BUFFER_SIZE),
            mid_buffer_offset: None,
            bounds: (start_inclusive, end_exclusive),
        }
    }

    /// Repurpose the finder for different bytes or bounds.
    pub fn repurpose(&mut self, magic_bytes: &'a [u8], bounds: (u64, u64)) -> &mut Self {
        debug_assert!(self.buffer.len() >= magic_bytes.len());

        self.finder = T::new(magic_bytes);
        self.cursor = T::reset_cursor(bounds, self.buffer.len());
        self.bounds = bounds;

        // Reset the mid-buffer offset, to invalidate buffer content.
        self.mid_buffer_offset = None;

        self
    }

    /// Find the next magic bytes in the direction specified in the type.
    pub fn next<R: Read + Seek + ?Sized>(&mut self, reader: &mut R) -> ZipResult<Option<u64>> {
        loop {
            if self.cursor < self.bounds.0 || self.cursor >= self.bounds.1 {
                // The finder is consumed
                break;
            }

            /* Position the window and ensure correct length */
            let window_start = self.cursor;
            let window_end = self
                .cursor
                .saturating_add(self.buffer.len() as u64)
                .min(self.bounds.1);

            if window_end <= window_start {
                // Short-circuit on zero-sized windows to prevent loop
                break;
            }

            let window = &mut self.buffer[..(window_end - window_start) as usize];

            if self.mid_buffer_offset.is_none() {
                reader.seek(SeekFrom::Start(window_start))?;
                if let Err(e) = reader.read_exact(window) {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        // If we can't read the whole window, it means the bounds or file length are wrong.
                        break;
                    }
                    return Err(e.into());
                }
            }

            let (window, window_start_offset) = match self.mid_buffer_offset {
                Some(mid_buffer_offset) => T::scope_window(window, mid_buffer_offset),
                None => (&*window, 0usize),
            };

            if let Some(offset) = self.finder.find(window) {
                let magic_pos = window_start + window_start_offset as u64 + offset as u64;
                reader.seek(SeekFrom::Start(magic_pos))?;

                self.mid_buffer_offset = Some(self.finder.move_scope(window_start_offset + offset));

                return Ok(Some(magic_pos));
            }

            self.mid_buffer_offset = None;

            match self
                .finder
                .move_cursor(self.cursor, self.bounds, self.buffer.len())
            {
                Some(new_cursor) => {
                    self.cursor = new_cursor;
                }
                None => {
                    // Destroy the finder when we've reached the end of the bounds.
                    self.bounds.0 = self.bounds.1;
                    break;
                }
            }
        }

        Ok(None)
    }
}

/// A magic bytes finder with an optimistic guess that is tried before
/// the inner finder begins searching from end. This enables much faster
/// lookup in files without appended junk, because the magic bytes will be
/// found directly.
///
/// The guess can be marked as mandatory to produce an error. This is useful
/// if the ArchiveOffset is known and auto-detection is not desired.
pub struct OptimisticMagicFinder<Direction> {
    inner: MagicFinder<Direction>,
    initial_guess: Option<(u64, bool)>,
}

/// This is a temporary restriction, to avoid heap allocation in [`Self::next_back`].
///
/// We only use magic bytes of size 4 at the moment.
const STACK_BUFFER_SIZE: usize = 8;

impl<'a, Direction: FinderDirection<'a>> OptimisticMagicFinder<Direction> {
    /// Create a new empty optimistic magic bytes finder.
    pub fn new_empty() -> Self {
        Self {
            inner: MagicFinder::new(&[], 0, 0),
            initial_guess: None,
        }
    }

    /// Repurpose the finder for different bytes, bounds and initial guesses.
    pub fn repurpose(
        &mut self,
        magic_bytes: &'a [u8],
        bounds: (u64, u64),
        initial_guess: Option<(u64, bool)>,
    ) -> &mut Self {
        debug_assert!(magic_bytes.len() <= STACK_BUFFER_SIZE);

        self.inner.repurpose(magic_bytes, bounds);
        self.initial_guess = initial_guess;

        self
    }

    /// Equivalent to `next_back`, with an optional initial guess attempted before
    /// proceeding with reading from the back of the reader.
    pub fn next<R: Read + Seek + ?Sized>(&mut self, reader: &mut R) -> ZipResult<Option<u64>> {
        if let Some((v, mandatory)) = self.initial_guess {
            reader.seek(SeekFrom::Start(v))?;

            let mut buffer = [0; STACK_BUFFER_SIZE];
            let buffer = &mut buffer[..self.inner.finder.needle().len()];

            // Attempt to match only if there's enough space for the needle
            if v.saturating_add(buffer.len() as u64) <= self.inner.bounds.1 {
                if let Err(e) = reader.read_exact(buffer) {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        if mandatory {
                            return Ok(None);
                        }
                    } else {
                        return Err(e.into());
                    }
                } else if self.inner.finder.needle() == buffer {
                    // If a match is found, yield it.
                    self.initial_guess.take();
                    reader.seek(SeekFrom::Start(v))?;
                    return Ok(Some(v));
                }
            }

            // If a match is not found, but the initial guess was mandatory, return an error.
            if mandatory {
                return Ok(None);
            }

            // If the initial guess was not mandatory, remove it, as it was not found.
            self.initial_guess.take();
        }

        self.inner.next(reader)
    }
}
