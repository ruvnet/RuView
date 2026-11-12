use crate::result::invalid;

use super::{
    MAX_SYMLINK_TARGET_LEN,
    central_header_to_zip_file_inner, make_symlink, read_zipfile_from_stream, ZipCentralEntryBlock,
    ZipFile, ZipFileData, ZipResult,
};
use crate::spec::FixedSizeBlock;
use indexmap::IndexMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Stream decoder for zip.
#[derive(Debug)]
pub struct ZipStreamReader<R>(R);

impl<R> ZipStreamReader<R> {
    /// Create a new ZipStreamReader
    pub const fn new(reader: R) -> Self {
        Self(reader)
    }
}

impl<R: Read> ZipStreamReader<R> {
    fn parse_central_directory(&mut self) -> ZipResult<ZipStreamFileMetadata> {
        // Give archive_offset and central_header_start dummy value 0, since
        // they are not used in the output.
        let archive_offset = 0;
        let central_header_start = 0;

        // Parse central header
        let block = ZipCentralEntryBlock::parse(&mut self.0)?;
        let file = central_header_to_zip_file_inner(
            &mut self.0,
            archive_offset,
            central_header_start,
            block,
        )?;
        Ok(ZipStreamFileMetadata(file))
    }

    /// Iterate over the stream and extract all file and their
    /// metadata.
    pub fn visit<V: ZipStreamVisitor>(mut self, visitor: &mut V) -> ZipResult<()> {
        while let Some(mut file) = read_zipfile_from_stream(&mut self.0)? {
            visitor.visit_file(&mut file)?;
        }

        while let Ok(metadata) = self.parse_central_directory() {
            visitor.visit_additional_metadata(&metadata)?;
        }

        Ok(())
    }

    /// Extract a Zip archive into a directory, overwriting files if they
    /// already exist. Paths are sanitized with [`ZipFile::enclosed_name`].
    ///
    /// Extraction is not atomic; If an error is encountered, some of the files
    /// may be left on disk.
    pub fn extract<P: AsRef<Path>>(self, directory: P) -> ZipResult<()> {
        use std::fs;
        fs::create_dir_all(&directory)?;
        let directory = directory.as_ref().canonicalize()?;
        struct Extractor(PathBuf, IndexMap<Box<str>, ()>);
        impl ZipStreamVisitor for Extractor {
            fn visit_file<R: Read>(&mut self, file: &mut ZipFile<'_, R>) -> ZipResult<()> {
                self.1.insert(file.name().into(), ());
                let mut outpath = self.0.clone();
                file.safe_prepare_path(&self.0, &mut outpath, None::<&(_, fn(&Path) -> bool)>)?;

                if file.is_symlink() {
                    let declared_len = file.size();
                    if declared_len > MAX_SYMLINK_TARGET_LEN {
                        return Err(invalid!(
                            "symlink target declares {} bytes, more than the {} byte limit",
                            declared_len,
                            MAX_SYMLINK_TARGET_LEN
                        ));
                    }
                    let mut target = Vec::with_capacity(declared_len as usize);
                    file.read_to_end(&mut target)?;
                    make_symlink(&outpath, &target, &self.1)?;
                    return Ok(());
                }

                if file.is_dir() {
                    fs::create_dir_all(&outpath)?;
                } else {
                    let mut outfile = fs::File::create(&outpath)?;
                    io::copy(file, &mut outfile)?;
                }

                Ok(())
            }

            #[allow(unused)]
            fn visit_additional_metadata(
                &mut self,
                metadata: &ZipStreamFileMetadata,
            ) -> ZipResult<()> {
                #[cfg(unix)]
                {
                    use super::ZipError;
                    let filepath = metadata
                        .enclosed_name()
                        .ok_or(crate::result::invalid!("Invalid file path"))?;

                    let outpath = self.0.join(filepath);

                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mode) = metadata.unix_mode() {
                        fs::set_permissions(outpath, fs::Permissions::from_mode(mode))?;
                    }
                }

                Ok(())
            }
        }

        self.visit(&mut Extractor(directory, IndexMap::new()))
    }
}

/// Visitor for ZipStreamReader
pub trait ZipStreamVisitor {
    ///  * `file` - contains the content of the file and most of the metadata,
    ///    except:
    ///     - `comment`: set to an empty string
    ///     - `data_start`: set to 0
    ///     - `external_attributes`: `unix_mode()`: will return None
    fn visit_file<R: Read>(&mut self, file: &mut ZipFile<'_, R>) -> ZipResult<()>;

    /// This function is guaranteed to be called after all `visit_file`s.
    ///
    ///  * `metadata` - Provides missing metadata in `visit_file`.
    fn visit_additional_metadata(&mut self, metadata: &ZipStreamFileMetadata) -> ZipResult<()>;
}

/// Additional metadata for the file.
#[derive(Debug)]
pub struct ZipStreamFileMetadata(ZipFileData);

impl ZipStreamFileMetadata {
    /// Get the name of the file
    ///
    /// # Warnings
    ///
    /// It is dangerous to use this name directly when extracting an archive.
    /// It may contain an absolute path (`/etc/shadow`), or break out of the
    /// current directory (`../runtime`). Carelessly writing to these paths
    /// allows an attacker to craft a ZIP archive that will overwrite critical
    /// files.
    ///
    /// You can use the [`ZipFile::enclosed_name`] method to validate the name
    /// as a safe path.
    pub fn name(&self) -> &str {
        &self.0.file_name
    }

    /// Get the name of the file, in the raw (internal) byte representation.
    ///
    /// The encoding of this data is currently undefined.
    pub fn name_raw(&self) -> &[u8] {
        &self.0.file_name_raw
    }

    /// Rewrite the path, ignoring any path components with special meaning.
    ///
    /// - Absolute paths are made relative
    /// - [std::path::Component::ParentDir]s are ignored
    /// - Truncates the filename at a NULL byte
    ///
    /// This is appropriate if you need to be able to extract *something* from
    /// any archive, but will easily misrepresent trivial paths like
    /// `foo/../bar` as `foo/bar` (instead of `bar`). Because of this,
    /// [`ZipFile::enclosed_name`] is the better option in most scenarios.
    pub fn mangled_name(&self) -> PathBuf {
        self.0.file_name_sanitized()
    }

    /// Ensure the file path is safe to use as a [`Path`].
    ///
    /// - It can't contain NULL bytes
    /// - It can't resolve to a path outside the current directory
    ///   > `foo/../bar` is fine, `foo/../../bar` is not.
    /// - It can't be an absolute path
    ///
    /// This will read well-formed ZIP files correctly, and is resistant
    /// to path-based exploits. It is recommended over
    /// [`ZipFile::mangled_name`].
    pub fn enclosed_name(&self) -> Option<PathBuf> {
        self.0.enclosed_name()
    }

    /// Returns whether the file is actually a directory
    pub fn is_dir(&self) -> bool {
        self.name()
            .chars()
            .next_back()
            .is_some_and(|c| c == '/' || c == '\\')
    }

    /// Returns whether the file is a regular file
    pub fn is_file(&self) -> bool {
        !self.is_dir()
    }

    /// Get the comment of the file
    pub fn comment(&self) -> &str {
        &self.0.file_comment
    }

    /// Get unix mode for the file
    pub const fn unix_mode(&self) -> Option<u32> {
        self.0.unix_mode()
    }
}

#[cfg(test)]
mod test {
    use tempfile::TempDir;

    use crate::read::stream::{ZipStreamFileMetadata, ZipStreamReader, ZipStreamVisitor};
    use crate::read::ZipFile;
    use crate::result::ZipResult;
    use crate::write::SimpleFileOptions;
    use crate::ZipWriter;
    use std::collections::BTreeSet;
    use std::io::{Cursor, Read};

    struct DummyVisitor;
    impl ZipStreamVisitor for DummyVisitor {
        fn visit_file<R: Read>(&mut self, _file: &mut ZipFile<'_, R>) -> ZipResult<()> {
            Ok(())
        }

        fn visit_additional_metadata(
            &mut self,
            _metadata: &ZipStreamFileMetadata,
        ) -> ZipResult<()> {
            Ok(())
        }
    }

    #[allow(dead_code)]
    #[derive(Default, Debug, Eq, PartialEq)]
    struct CounterVisitor(u64, u64);
    impl ZipStreamVisitor for CounterVisitor {
        fn visit_file<R: Read>(&mut self, _file: &mut ZipFile<'_, R>) -> ZipResult<()> {
            self.0 += 1;
            Ok(())
        }

        fn visit_additional_metadata(
            &mut self,
            _metadata: &ZipStreamFileMetadata,
        ) -> ZipResult<()> {
            self.1 += 1;
            Ok(())
        }
    }

    #[test]
    fn invalid_offset() {
        ZipStreamReader::new(Cursor::new(include_bytes!(
            "../../tests/data/invalid_offset.zip"
        )))
        .visit(&mut DummyVisitor)
        .unwrap_err();
    }

    #[test]
    fn invalid_offset2() {
        ZipStreamReader::new(Cursor::new(include_bytes!(
            "../../tests/data/invalid_offset2.zip"
        )))
        .visit(&mut DummyVisitor)
        .unwrap_err();
    }

    #[test]
    fn zip_read_streaming() {
        let reader =
            ZipStreamReader::new(Cursor::new(include_bytes!("../../tests/data/mimetype.zip")));

        #[derive(Default)]
        struct V {
            filenames: BTreeSet<Box<str>>,
        }
        impl ZipStreamVisitor for V {
            fn visit_file<R: Read>(&mut self, file: &mut ZipFile<'_, R>) -> ZipResult<()> {
                if file.is_file() {
                    self.filenames.insert(file.name().into());
                }

                Ok(())
            }
            fn visit_additional_metadata(
                &mut self,
                metadata: &ZipStreamFileMetadata,
            ) -> ZipResult<()> {
                if metadata.is_file() {
                    assert!(
                        self.filenames.contains(metadata.name()),
                        "{} is missing its file content",
                        metadata.name()
                    );
                }

                Ok(())
            }
        }

        reader.visit(&mut V::default()).unwrap();
    }

    #[test]
    fn file_and_dir_predicates() {
        let reader = ZipStreamReader::new(Cursor::new(include_bytes!(
            "../../tests/data/files_and_dirs.zip"
        )));

        #[derive(Default)]
        struct V {
            filenames: BTreeSet<Box<str>>,
        }
        impl ZipStreamVisitor for V {
            fn visit_file<R: Read>(&mut self, file: &mut ZipFile<'_, R>) -> ZipResult<()> {
                let full_name = file.enclosed_name().unwrap();
                let file_name = full_name.file_name().unwrap().to_str().unwrap();
                assert!(
                    (file_name.starts_with("dir") && file.is_dir())
                        || (file_name.starts_with("file") && file.is_file())
                );

                if file.is_file() {
                    self.filenames.insert(file.name().into());
                }

                Ok(())
            }
            fn visit_additional_metadata(
                &mut self,
                metadata: &ZipStreamFileMetadata,
            ) -> ZipResult<()> {
                if metadata.is_file() {
                    assert!(
                        self.filenames.contains(metadata.name()),
                        "{} is missing its file content",
                        metadata.name()
                    );
                }

                Ok(())
            }
        }

        reader.visit(&mut V::default()).unwrap();
    }

    /// test case to ensure we don't preemptively over allocate based on the
    /// declared number of files in the CDE of an invalid zip when the number of
    /// files declared is more than the alleged offset in the CDE
    #[test]
    fn invalid_cde_number_of_files_allocation_smaller_offset() {
        ZipStreamReader::new(Cursor::new(include_bytes!(
            "../../tests/data/invalid_cde_number_of_files_allocation_smaller_offset.zip"
        )))
        .visit(&mut DummyVisitor)
        .unwrap_err();
    }

    /// test case to ensure we don't preemptively over allocate based on the
    /// declared number of files in the CDE of an invalid zip when the number of
    /// files declared is less than the alleged offset in the CDE
    #[test]
    fn invalid_cde_number_of_files_allocation_greater_offset() {
        ZipStreamReader::new(Cursor::new(include_bytes!(
            "../../tests/data/invalid_cde_number_of_files_allocation_greater_offset.zip"
        )))
        .visit(&mut DummyVisitor)
        .unwrap_err();
    }

    /// Symlinks being extracted shouldn't be followed out of the destination directory.
    #[test]
    fn test_cannot_symlink_outside_destination() -> ZipResult<()> {
        use std::fs::create_dir;

        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.add_symlink("symlink/", "../dest-sibling/", SimpleFileOptions::default())?;
        writer.start_file("symlink/dest-file", SimpleFileOptions::default())?;
        let reader = ZipStreamReader::new(writer.finish()?);
        let dest_parent = TempDir::with_prefix("stream__cannot_symlink_outside_destination")?;
        let dest_sibling = dest_parent.path().join("dest-sibling");
        create_dir(&dest_sibling)?;
        let dest = dest_parent.path().join("dest");
        create_dir(&dest)?;
        assert!(reader.extract(dest).is_err());
        assert!(!dest_sibling.join("dest-file").exists());
        Ok(())
    }

    #[test]
    fn test_can_create_destination() -> ZipResult<()> {
        let mut v = Vec::new();
        v.extend_from_slice(include_bytes!("../../tests/data/mimetype.zip"));
        let reader = ZipStreamReader::new(v.as_slice());
        let dest = TempDir::with_prefix("stream_test_can_create_destination").unwrap();
        reader.extract(&dest)?;
        assert!(dest.path().join("mimetype").exists());
        Ok(())
    }
}
