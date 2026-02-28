//! Kernel and initramfs extraction from RVF files.
//!
//! Opens the RVF store read-only, locates the KERNEL_SEG, parses the
//! KernelHeader, and writes the kernel image and optional initramfs to
//! temporary files that persist until the returned handles are dropped.

use std::io::Write;
use std::path::{Path, PathBuf};

use rvf_runtime::RvfStore;
use rvf_types::kernel::KernelHeader;

use crate::error::LaunchError;

/// Extracted kernel artifacts ready for QEMU consumption.
pub struct ExtractedKernel {
    /// Path to the extracted kernel image (bzImage or equivalent).
    pub kernel_path: PathBuf,
    /// Path to the extracted initramfs, if present in the KERNEL_SEG.
    pub initramfs_path: Option<PathBuf>,
    /// The parsed KernelHeader.
    pub header: KernelHeader,
    /// The kernel command line string.
    pub cmdline: String,
    /// Temp directory holding the extracted files (kept alive via ownership).
    _tempdir: tempfile::TempDir,
}

impl std::fmt::Debug for ExtractedKernel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtractedKernel")
            .field("kernel_path", &self.kernel_path)
            .field("initramfs_path", &self.initramfs_path)
            .field("cmdline", &self.cmdline)
            .finish_non_exhaustive()
    }
}

/// Extract kernel and initramfs from an RVF file to temporary files.
///
/// The returned `ExtractedKernel` owns a `TempDir`; the files are cleaned
/// up when it is dropped.
pub fn extract_kernel(rvf_path: &Path) -> Result<ExtractedKernel, LaunchError> {
    let store = RvfStore::open_readonly(rvf_path)
        .map_err(|e| LaunchError::KernelExtraction(format!("failed to open store: {e:?}")))?;

    let (header_bytes, remainder) = store
        .extract_kernel()
        .map_err(|e| LaunchError::KernelExtraction(format!("segment read error: {e:?}")))?
        .ok_or_else(|| LaunchError::NoKernelSegment {
            path: rvf_path.to_path_buf(),
        })?;

    if header_bytes.len() < 128 {
        return Err(LaunchError::KernelExtraction(
            "KernelHeader too short".into(),
        ));
    }

    let mut hdr_array = [0u8; 128];
    hdr_array.copy_from_slice(&header_bytes[..128]);
    let header = KernelHeader::from_bytes(&hdr_array)
        .map_err(|e| LaunchError::KernelExtraction(format!("bad KernelHeader: {e:?}")))?;

    // The wire format after the 128-byte KernelHeader (which is already
    // split off into `header_bytes`) is:
    //
    //   For simple embed_kernel (no binding):
    //     kernel_image || cmdline
    //
    //   For embed_kernel_with_binding:
    //     KernelBinding(128) || cmdline || kernel_image
    //
    // We determine the layout from header.image_size which tells us the
    // kernel image length. The cmdline is header.cmdline_length bytes.

    let image_size = header.image_size as usize;
    let cmdline_length = header.cmdline_length as usize;

    // Simple format: image comes first in the remainder, then cmdline
    let (kernel_image, cmdline) = if image_size > 0 && image_size <= remainder.len() {
        let img = &remainder[..image_size];
        let cmd = if cmdline_length > 0 && image_size + cmdline_length <= remainder.len() {
            String::from_utf8_lossy(&remainder[image_size..image_size + cmdline_length])
                .into_owned()
        } else {
            String::new()
        };
        (img, cmd)
    } else {
        // Fallback: treat entire remainder as kernel image
        (&remainder[..], String::new())
    };

    // Write to temp files
    let tempdir = tempfile::tempdir().map_err(LaunchError::TempFile)?;

    let kernel_file_path = tempdir.path().join("vmlinuz");
    {
        let mut f = std::fs::File::create(&kernel_file_path).map_err(LaunchError::TempFile)?;
        f.write_all(kernel_image).map_err(LaunchError::TempFile)?;
        f.sync_all().map_err(LaunchError::TempFile)?;
    }

    // For now we do not split out a separate initramfs from the kernel
    // image. A future version could detect an appended initramfs using
    // the standard Linux trailer magic (0x6d65736800000000).
    let initramfs_path = None;

    Ok(ExtractedKernel {
        kernel_path: kernel_file_path,
        initramfs_path,
        header,
        cmdline,
        _tempdir: tempdir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rvf_runtime::options::RvfOptions;
    use rvf_types::kernel::KernelArch;

    #[test]
    fn extract_from_store_with_kernel() {
        let dir = tempfile::tempdir().unwrap();
        let rvf_path = dir.path().join("test.rvf");

        let opts = RvfOptions {
            dimension: 4,
            ..Default::default()
        };
        let mut store = RvfStore::create(&rvf_path, opts).unwrap();

        let image = b"MZ\x00fake-kernel-image-for-testing";
        store
            .embed_kernel(
                KernelArch::X86_64 as u8,
                0x01,
                0,
                image,
                8080,
                Some("console=ttyS0"),
            )
            .unwrap();
        store.close().unwrap();

        let extracted = extract_kernel(&rvf_path).unwrap();
        assert!(extracted.kernel_path.exists());

        let on_disk = std::fs::read(&extracted.kernel_path).unwrap();
        assert_eq!(on_disk, image);
        assert_eq!(extracted.header.api_port, 8080);
        assert_eq!(extracted.cmdline, "console=ttyS0");
    }

    #[test]
    fn extract_kernel_no_cmdline() {
        let dir = tempfile::tempdir().unwrap();
        let rvf_path = dir.path().join("no_cmd.rvf");

        let opts = RvfOptions {
            dimension: 4,
            ..Default::default()
        };
        let mut store = RvfStore::create(&rvf_path, opts).unwrap();

        let image = b"fake-kernel";
        store
            .embed_kernel(KernelArch::X86_64 as u8, 0x01, 0, image, 9090, None)
            .unwrap();
        store.close().unwrap();

        let extracted = extract_kernel(&rvf_path).unwrap();
        let on_disk = std::fs::read(&extracted.kernel_path).unwrap();
        assert_eq!(on_disk, image);
        assert!(extracted.cmdline.is_empty());
    }

    #[test]
    fn extract_returns_error_when_no_kernel() {
        let dir = tempfile::tempdir().unwrap();
        let rvf_path = dir.path().join("no_kernel.rvf");

        let opts = RvfOptions {
            dimension: 4,
            ..Default::default()
        };
        let store = RvfStore::create(&rvf_path, opts).unwrap();
        store.close().unwrap();

        let result = extract_kernel(&rvf_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            LaunchError::NoKernelSegment { .. } => {}
            other => panic!("expected NoKernelSegment, got: {other}"),
        }
    }
}
