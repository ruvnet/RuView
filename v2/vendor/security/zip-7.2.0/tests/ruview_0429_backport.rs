/// A symlink entry whose central directory declares an absurd uncompressed size
/// must be rejected, not pre-allocated.
///
/// The declared size comes from the archive itself. Before the fix, `extract`
/// passed it straight to `Vec::with_capacity`, so a hostile entry could ask for
/// 2^62 bytes and abort the whole process ("memory allocation of ... failed").
/// A symlink target is a path, so a size this large is malformed and the entry
/// must come back as an error.
#[cfg(not(miri))]
#[test]
fn extract_symlink_with_hostile_declared_size() -> zip::result::ZipResult<()> {
    use std::io::Cursor;
    use tempfile::TempDir;
    use zip::ZipArchive;

    /// One entry, flagged as a symlink, whose Zip64 extra field claims
    /// `declared_size` uncompressed bytes while storing only `payload`.
    fn build(declared_size: u64, payload: &[u8]) -> Vec<u8> {
        const S_IFLNK: u32 = 0o120_000;
        const SENTINEL: u32 = 0xFFFF_FFFF;
        let name = b"link";
        let mut o = Vec::new();

        // local file header
        o.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        o.extend_from_slice(&45u16.to_le_bytes()); // version needed: zip64
        o.extend_from_slice(&0u16.to_le_bytes()); // flags
        o.extend_from_slice(&0u16.to_le_bytes()); // method: Stored
        o.extend_from_slice(&0u16.to_le_bytes()); // mod time
        o.extend_from_slice(&0x21u16.to_le_bytes()); // mod date
        o.extend_from_slice(&0u32.to_le_bytes()); // crc32
        o.extend_from_slice(&SENTINEL.to_le_bytes());
        o.extend_from_slice(&SENTINEL.to_le_bytes());
        o.extend_from_slice(&(name.len() as u16).to_le_bytes());
        o.extend_from_slice(&20u16.to_le_bytes()); // zip64 extra field
        o.extend_from_slice(name);
        o.extend_from_slice(&0x0001u16.to_le_bytes()); // zip64 header id
        o.extend_from_slice(&16u16.to_le_bytes());
        o.extend_from_slice(&declared_size.to_le_bytes());
        o.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        o.extend_from_slice(payload);

        let cd_offset = o.len() as u32;

        // central directory header
        o.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
        o.extend_from_slice(&0x031Eu16.to_le_bytes()); // version made by: Unix
        o.extend_from_slice(&45u16.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        o.extend_from_slice(&0x21u16.to_le_bytes());
        o.extend_from_slice(&0u32.to_le_bytes()); // crc32
        o.extend_from_slice(&SENTINEL.to_le_bytes());
        o.extend_from_slice(&SENTINEL.to_le_bytes());
        o.extend_from_slice(&(name.len() as u16).to_le_bytes());
        o.extend_from_slice(&20u16.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes()); // comment len
        o.extend_from_slice(&0u16.to_le_bytes()); // disk number start
        o.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        o.extend_from_slice(&((S_IFLNK | 0o777) << 16).to_le_bytes()); // symlink
        o.extend_from_slice(&0u32.to_le_bytes()); // local header offset
        o.extend_from_slice(name);
        o.extend_from_slice(&0x0001u16.to_le_bytes());
        o.extend_from_slice(&16u16.to_le_bytes());
        o.extend_from_slice(&declared_size.to_le_bytes());
        o.extend_from_slice(&(payload.len() as u64).to_le_bytes());

        let cd_size = o.len() as u32 - cd_offset;

        // end of central directory
        o.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        o.extend_from_slice(&0u16.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        o.extend_from_slice(&1u16.to_le_bytes());
        o.extend_from_slice(&1u16.to_le_bytes());
        o.extend_from_slice(&cd_size.to_le_bytes());
        o.extend_from_slice(&cd_offset.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        o
    }

    for declared_size in [4097, 1u64 << 40, 1u64 << 62, u64::MAX] {
        let mut reader = ZipArchive::new(Cursor::new(build(declared_size, b"/etc/passwd")))?;
        assert!(reader.by_index(0)?.is_symlink());
        let tempdir = TempDir::with_prefix("test_hostile_symlink_size")?;

        let err = reader
            .extract(&tempdir)
            .expect_err("oversized symlink target must be rejected");
        assert!(
            matches!(err, zip::result::ZipError::InvalidArchive(_)),
            "expected InvalidArchive for a {declared_size} byte symlink target, got {err:?}"
        );
    }

    Ok(())
}


#[test]
fn stream_does_not_expose_the_symlink_bit_backport() -> zip::result::ZipResult<()> {
    use std::io::Cursor;
    use zip::{ZipWriter, write::SimpleFileOptions};
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer.add_symlink("link", "/target", SimpleFileOptions::default())?;
    let mut cur = writer.finish()?;
    cur.set_position(0);
    let file = zip::read::read_zipfile_from_stream(&mut cur)?.unwrap();
    assert!(!file.is_symlink());
    Ok(())
}

#[test]
fn valid_regular_file_roundtrip() -> zip::result::ZipResult<()> {
    use std::io::{Cursor, Write};
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    w.start_file("regular.txt", SimpleFileOptions::default())?;
    w.write_all(b"ordinary archive payload")?;
    let mut bytes = w.finish()?; bytes.set_position(0);
    let dir = tempfile::TempDir::new()?;
    ZipArchive::new(bytes)?.extract(dir.path())?;
    assert_eq!(std::fs::read(dir.path().join("regular.txt"))?, b"ordinary archive payload");
    Ok(())
}

#[cfg(unix)]
#[test]
fn valid_relative_symlink_roundtrip() -> zip::result::ZipResult<()> {
    use std::io::{Cursor, Write};
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    w.start_file("target", SimpleFileOptions::default())?;
    w.write_all(b"target payload")?;
    w.add_symlink("link", "target", SimpleFileOptions::default())?;
    let mut bytes = w.finish()?; bytes.set_position(0);
    let dir = tempfile::TempDir::new()?;
    ZipArchive::new(bytes)?.extract(dir.path())?;
    assert_eq!(std::fs::read_link(dir.path().join("link"))?, std::path::PathBuf::from("target"));
    assert_eq!(std::fs::read(dir.path().join("link"))?, b"target payload");
    Ok(())
}
