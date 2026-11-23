/// Provides high level API for reading from a stream.
pub mod stream {
    pub use crate::read::stream::*;
}
/// Types for creating ZIP archives.
pub mod write {
    use crate::write::FileOptions;
    /// Unstable methods for [`FileOptions`].
    pub trait FileOptionsExt {
        /// Write the file with the given password using the deprecated ZipCrypto algorithm.
        ///
        /// This is not recommended for new archives, as ZipCrypto is not secure.
        fn with_deprecated_encryption(self, password: &[u8]) -> Self;
    }
    impl FileOptionsExt for FileOptions {
        fn with_deprecated_encryption(self, password: &[u8]) -> Self {
            self.with_deprecated_encryption(password)
        }
    }
}
