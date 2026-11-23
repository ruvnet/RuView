//! Private API adapter for Candle 0.9's uncompressed ZIP reader/writer usage.
//!
//! ZIP parsing and extraction remain in registry `zip = 9.0.5`, pinned and
//! checksum-verified by the workspace lockfile. The wrappers validate filename
//! decoding before exposing the infallible filename access used by Candle 0.9.
//! The 7.2.0 version satisfies Candle's existing manifest; the build metadata
//! identifies the underlying major-version migration. This is not a general
//! promise of ZIP 7 feature or behavioral compatibility.

pub use zip_fixed::*;

use std::io::{self, Read, Seek};
use std::ops::Deref;
use std::path::Path;

/// Candle-facing archive whose filenames have already been decoded successfully.
pub struct ZipArchive<R: Read + Seek> {
    inner: zip_fixed::ZipArchive<R>,
    names: Vec<String>,
}

impl<R: Read + Seek> ZipArchive<R> {
    /// Open the upstream archive and propagate every filename decoding error.
    pub fn new(reader: R) -> result::ZipResult<Self> {
        let inner = zip_fixed::ZipArchive::new(reader)?;
        let names = inner
            .file_names()
            .map(|name| name.map(|name| name.into_owned()))
            .collect::<result::ZipResult<Vec<_>>>()?;
        Ok(Self { inner, names })
    }

    /// Iterate the already validated names using Candle's older access pattern.
    pub fn file_names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    /// Open a named entry through upstream ZIP 9.
    pub fn by_name(&mut self, name: &str) -> result::ZipResult<ZipFile<'_, R>> {
        ZipFile::new(self.inner.by_name(name)?)
    }

    /// Open an indexed entry through upstream ZIP 9.
    pub fn by_index(&mut self, index: usize) -> result::ZipResult<ZipFile<'_, R>> {
        ZipFile::new(self.inner.by_index(index)?)
    }

    /// Return the number of archive entries.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Report whether the archive has no entries.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Delegate extraction, including symlink confinement, to upstream ZIP 9.
    pub fn extract<P: AsRef<Path>>(&mut self, directory: P) -> result::ZipResult<()> {
        self.inner.extract(directory)
    }

    /// Explicitly compare local headers with the central directory.
    pub fn validate_header_consistency(&mut self) -> result::ZipResult<()> {
        self.inner.validate_header_consistency()
    }
}

/// Entry wrapper preserving Candle's `name() -> &str` access pattern.
pub struct ZipFile<'a, R: Read> {
    inner: zip_fixed::read::ZipFile<'a, R>,
    name: String,
}

impl<'a, R: Read> ZipFile<'a, R> {
    fn new(inner: zip_fixed::read::ZipFile<'a, R>) -> result::ZipResult<Self> {
        let name = inner.name()?.into_owned();
        Ok(Self { inner, name })
    }
    /// Return the name decoded successfully when this entry was opened.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<'a, R: Read> Deref for ZipFile<'a, R> {
    type Target = zip_fixed::read::ZipFile<'a, R>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<R: Read> Read for ZipFile<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read, Write};

    fn stored_archive() -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options =
            write::SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        writer.start_file("a.txt", options).unwrap();
        writer.write_all(b"model fixture").unwrap();
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn candle_style_stored_reader_writer_roundtrip() {
        let mut archive = ZipArchive::new(Cursor::new(stored_archive())).unwrap();
        assert_eq!(archive.file_names().collect::<Vec<_>>(), ["a.txt"]);
        archive.validate_header_consistency().unwrap();
        assert_eq!(archive.by_index(0).unwrap().name(), "a.txt");
        let mut content = String::new();
        archive
            .by_name("a.txt")
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content, "model fixture");
    }

    #[test]
    fn inconsistent_local_and_central_names_are_detectable() {
        let mut bytes = stored_archive();
        // The first local header's filename starts at byte 30. Keep its length
        // and the central directory intact, changing only the local name.
        assert_eq!(&bytes[30..35], b"a.txt");
        bytes[30] = b'b';
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert!(archive.validate_header_consistency().is_err());
    }

    #[test]
    fn unicode_names_are_preserved_and_missing_names_remain_errors() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("été.npy", write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"fixture").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(archive.file_names().collect::<Vec<_>>(), ["été.npy"]);
        assert_eq!(archive.by_name("été.npy").unwrap().name(), "été.npy");
        assert!(archive.by_name("missing.npy").is_err());
        assert!(ZipArchive::new(Cursor::new(Vec::<u8>::new())).is_err());
    }

    #[test]
    fn extraction_cannot_write_through_an_escaping_symlink() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        let outside = root.path().join("outside");
        std::fs::create_dir(&destination).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = write::SimpleFileOptions::default();
        writer
            .add_symlink("pivot", outside.to_string_lossy(), options)
            .unwrap();
        writer.add_symlink("bridge", "pivot", options).unwrap();
        writer.start_file("bridge/payload", options).unwrap();
        writer.write_all(b"must stay inside destination").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert!(archive.extract(&destination).is_err());
        assert!(!outside.join("payload").exists());
    }
}
