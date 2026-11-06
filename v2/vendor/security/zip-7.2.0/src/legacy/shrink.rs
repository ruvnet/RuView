use std::io::{self, Error, Read};

use bitstream_io::{BitRead, BitReader, Endianness, LittleEndian};

const MIN_CODE_SIZE: u8 = 9;
const MAX_CODE_SIZE: u8 = 13;

const MAX_CODE: usize = (1 << MAX_CODE_SIZE) - 1;
const CONTROL_CODE: usize = 256;
const INC_CODE_SIZE: u16 = 1;
const PARTIAL_CLEAR: u16 = 2;

/// Number of codes available for the LZW dictionary (excluding control codes)
/// These are the codes from CONTROL_CODE+1 to MAX_CODE
const FREE_CODE_QUEUE_SIZE: usize = MAX_CODE - CONTROL_CODE + 1;

// const HASH_BITS: usize = MAX_CODE_SIZE + 1; /* For a load factor of 0.5. */
// const HASHTAB_SIZE: usize = 1 << HASH_BITS;
const UNKNOWN_LEN: u16 = u16::MAX;

struct CodeQueue {
    next_idx: usize,
    codes: [Option<u16>; FREE_CODE_QUEUE_SIZE],
}

impl CodeQueue {
    fn new() -> Self {
        let mut codes = [None; FREE_CODE_QUEUE_SIZE];
        for (i, code) in ((CONTROL_CODE as u16 + 1)..=(MAX_CODE as u16 - 1)).enumerate() {
            codes[i] = Some(code);
        }
        Self { next_idx: 0, codes }
    }

    // Return the next code in the queue, or INVALID_CODE if the queue is empty.
    fn next(&self) -> Option<u16> {
        //   assert(q->next_idx < sizeof(q->codes) / sizeof(q->codes[0]));
        if let Some(Some(next)) = self.codes.get(self.next_idx) {
            Some(*next)
        } else {
            None
        }
    }

    /// Return and remove the next code from the queue, or return INVALID_CODE if
    /// the queue is empty.
    fn remove_next(&mut self) -> Option<u16> {
        let res = self.next();
        if res.is_some() {
            self.next_idx += 1;
        }
        res
    }
}

#[derive(Clone, Debug, Copy, Default)]
struct Codetab {
    last_dst_pos: usize,
    prefix_code: Option<u16>,
    len: u16,
    ext_byte: u8,
}

impl Codetab {
    pub fn create_new() -> [Self; MAX_CODE + 1] {
        let mut codetab = [Codetab::default(); MAX_CODE + 1];
        for (i, code) in codetab.iter_mut().enumerate().take(u8::MAX as usize + 1) {
            *code = Codetab {
                prefix_code: Some(i as u16),
                ext_byte: i as u8,
                len: 1,
                last_dst_pos: 0,
            };
        }
        codetab
    }
}

fn unshrink_partial_clear(codetab: &mut [Codetab], queue: &mut CodeQueue) {
    let mut is_prefix = [false; MAX_CODE + 1];

    // Scan for codes that have been used as a prefix.
    for code in codetab.iter().take(MAX_CODE + 1).skip(CONTROL_CODE + 1) {
        if let Some(prefix_code) = code.prefix_code {
            is_prefix[prefix_code as usize] = true;
        }
    }

    // Clear "non-prefix" codes in the table; populate the code queue.
    let mut code_queue_size = 0;
    for i in (CONTROL_CODE + 1)..MAX_CODE {
        if !is_prefix[i] {
            codetab[i].prefix_code = None;
            queue.codes[code_queue_size] = Some(i as u16);
            code_queue_size += 1;
        }
    }
    queue.codes[code_queue_size] = None; // End-of-queue marker.
    queue.next_idx = 0;
}

/// Read the next code from the input stream and return it in next_code. Returns
/// `Ok(None)` if the end of the stream is reached. If the stream contains invalid
/// data, returns `Err`.
fn read_code<T: std::io::Read, E: Endianness>(
    is: &mut BitReader<T, E>,
    code_size: &mut u8,
    codetab: &mut [Codetab],
    queue: &mut CodeQueue,
) -> io::Result<Option<u16>> {
    // assert(sizeof(code) * CHAR_BIT >= *code_size);
    let code = is.read_var::<u16>(*code_size as u32)?;

    // Handle regular codes (the common case).
    if code != CONTROL_CODE as u16 {
        return Ok(Some(code));
    }

    // Handle control codes.
    if let Ok(control_code) = is.read_var::<u16>(*code_size as u32) {
        match control_code {
            INC_CODE_SIZE => {
                if *code_size >= MAX_CODE_SIZE {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "tried to increase code size when already at maximum",
                    ));
                }
                *code_size += 1;
            }
            PARTIAL_CLEAR => {
                unshrink_partial_clear(codetab, queue);
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid control code {}", control_code),
                ));
            }
        }
        return read_code(is, code_size, codetab, queue);
    }
    Ok(None)
}

/// Output the string represented by a code into dst at dst_pos.
///
/// # Returns
/// new first byte of the string, or io::Error InvalidData on invalid prefix code.
fn output_code(
    dst: &mut Vec<u8>,
    code: u16,
    prev_code: u16,
    codetab: &mut [Codetab],
    queue: &CodeQueue,
    first_byte: u8,
) -> io::Result<u8> {
    debug_assert!(code <= MAX_CODE as u16 && code != CONTROL_CODE as u16);
    if code <= u8::MAX as u16 {
        dst.push(code as u8);
        return Ok(code as u8);
    }
    if codetab[code as usize].prefix_code.is_none()
        || codetab[code as usize].prefix_code == Some(code)
    {
        // Reject invalid codes. Self-referential codes may exist in
        // the table but cannot be used.
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid code"));
    }
    if codetab[code as usize].len != UNKNOWN_LEN {
        // Output string with known length (the common case).
        let ct = &codetab[code as usize];
        for i in ct.last_dst_pos..ct.last_dst_pos + ct.len as usize {
            dst.push(dst[i]);
        }
        return Ok(dst[ct.last_dst_pos]);
    }

    // Output a string of unknown length. This happens when the prefix
    // was invalid (due to partial clearing) when the code was inserted into
    // the table. The prefix can then become valid when it's added to the
    // table at a later point.
    let prefix_code = codetab[code as usize].prefix_code.unwrap();
    if cfg!(debug_assertions) {
        let tab_entry = codetab[code as usize];
        assert!(tab_entry.len == UNKNOWN_LEN);
        assert!(tab_entry.prefix_code.unwrap() as usize > CONTROL_CODE);
    }

    if Some(prefix_code) == queue.next() {
        /* The prefix code hasn't been added yet, but we were just
        about to: the KwKwK case. Add the previous string extended
        with its first byte. */
        codetab[prefix_code as usize] = Codetab {
            prefix_code: Some(prev_code),
            ext_byte: first_byte,
            len: codetab[prev_code as usize].len + 1,
            last_dst_pos: codetab[prev_code as usize].last_dst_pos,
        };
        dst.push(first_byte);
    } else if codetab[prefix_code as usize].prefix_code.is_none() {
        // The prefix code is still invalid.
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid prefix code",
        ));
    }

    // Output the prefix string, then the extension byte.
    let ct = codetab[prefix_code as usize];
    let last_dst_pos = dst.len();
    let start = ct.last_dst_pos;
    let len = ct.len as usize;
    let first = dst[start];
    dst.extend_from_within(start..start + len);
    dst.push(codetab[code as usize].ext_byte);
    // Update the code table now that the string has a length and pos.
    debug_assert!(prev_code != code);

    codetab[code as usize].len = (len + 1) as u16;
    codetab[code as usize].last_dst_pos = last_dst_pos;

    Ok(first)
}

fn hwunshrink(src: &[u8], uncompressed_size: usize, dst: &mut Vec<u8>) -> io::Result<()> {
    dst.reserve(uncompressed_size);
    let mut codetab = Codetab::create_new();
    let mut queue = CodeQueue::new();
    let mut is = BitReader::endian(src, LittleEndian);
    let mut code_size = MIN_CODE_SIZE;

    // Handle the first code separately since there is no previous code.
    let Some(curr_code) = read_code(&mut is, &mut code_size, &mut codetab, &mut queue)? else {
        return Ok(());
    };

    debug_assert!(curr_code != CONTROL_CODE as u16);
    if curr_code > u8::MAX as u16 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the first code must be a literal",
        ));
    }
    let mut first_byte = curr_code as u8; // make mutable
    codetab[curr_code as usize].last_dst_pos = dst.len();
    dst.push(first_byte);
    let mut prev_code = curr_code;
    while dst.len() < uncompressed_size {
        let curr_opt = match read_code(&mut is, &mut code_size, &mut codetab, &mut queue) {
            Ok(c) => c,
            Err(_) => break, // treat read error as end-of-stream for now; validated after loop
        };

        let Some(curr_code) = curr_opt else {
            return Err(Error::new(io::ErrorKind::InvalidData, "invalid code"));
        };

        let dst_pos = dst.len();
        // Handle KwKwK: next code used before being added.
        if Some(curr_code) == queue.next() {
            if codetab[prev_code as usize].prefix_code.is_none() {
                return Err(Error::new(
                    io::ErrorKind::InvalidData,
                    "previous code no longer valid",
                ));
            }
            // Extend the previous code with its first byte.
            debug_assert!(curr_code != prev_code);
            codetab[curr_code as usize] = Codetab {
                prefix_code: Some(prev_code),
                ext_byte: first_byte,
                len: codetab[prev_code as usize].len + 1,
                last_dst_pos: codetab[prev_code as usize].last_dst_pos,
            };
        }
        // Output the string represented by the current code.
        first_byte = output_code(dst, curr_code, prev_code, &mut codetab, &queue, first_byte)?;
        if let Some(new_code) = queue.remove_next() {
            let prev_entry = codetab[prev_code as usize];
            codetab[new_code as usize] = Codetab {
                prefix_code: Some(prev_code),
                ext_byte: first_byte,
                last_dst_pos: prev_entry.last_dst_pos,
                len: if prev_entry.prefix_code.is_none() {
                    // prev_code was invalidated in a partial
                    // clearing. Until that code is re-used, the
                    // string represented by new_code is
                    // indeterminate.
                    UNKNOWN_LEN
                } else {
                    prev_entry.len + 1
                },
            };
        }
        codetab[curr_code as usize].last_dst_pos = dst_pos;
        prev_code = curr_code;
    }

    if dst.len() != uncompressed_size {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "unexpected end of compressed stream",
        ));
    }

    Ok(())
}

#[derive(Debug)]
pub struct ShrinkDecoder<R> {
    compressed_reader: R,
    stream_read: bool,
    uncompressed_size: u64,
    stream: Vec<u8>,
    read_pos: usize,
}

impl<R: Read> ShrinkDecoder<R> {
    pub fn new(inner: R, uncompressed_size: u64) -> Self {
        Self {
            compressed_reader: inner,
            uncompressed_size,
            stream_read: false,
            stream: Vec::new(),
            read_pos: 0,
        }
    }
    pub fn into_inner(self) -> R {
        self.compressed_reader
    }
}

impl<R: Read> Read for ShrinkDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.stream_read {
            self.stream_read = true;
            let mut compressed_bytes = Vec::new();
            self.compressed_reader.read_to_end(&mut compressed_bytes)?;
            self.stream.reserve(self.uncompressed_size as usize);
            hwunshrink(
                &compressed_bytes,
                self.uncompressed_size as usize,
                &mut self.stream,
            )?;
        }
        let available = self.stream.len().saturating_sub(self.read_pos);
        if available == 0 {
            return Ok(0);
        }
        let n = available.min(buf.len());
        buf[..n].copy_from_slice(&self.stream[self.read_pos..self.read_pos + n]);
        self.read_pos += n;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use crate::legacy::shrink::hwunshrink;

    const LZW_FIG5: &[u8; 17] = b"ababcbababaaaaaaa";
    const LZW_FIG5_SHRUNK: [u8; 12] = [
        0x61, 0xc4, 0x04, 0x1c, 0x23, 0xb0, 0x60, 0x98, 0x83, 0x08, 0xc3, 0x00,
    ];

    #[test]
    fn test_unshrink_lzw_fig5() {
        let mut dst = Vec::with_capacity(LZW_FIG5.len());
        hwunshrink(&LZW_FIG5_SHRUNK, LZW_FIG5.len(), &mut dst).unwrap();
        assert_eq!(dst.as_slice(), LZW_FIG5);
    }
}
