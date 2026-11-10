use crate::{ReadNpyError, ReadNpyExt, ReadableElement, WriteNpyError, WriteNpyExt};
use ndarray::prelude::*;
use ndarray::DataOwned;
use std::error::Error;
use std::fmt;
use std::io::{BufWriter, Read, Seek, Write};
use zip::result::ZipError;
use zip::write::{FileOptionExtension, FileOptions, SimpleFileOptions};
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// An error writing a `.npz` file.
#[derive(Debug)]
pub enum WriteNpzError {
    /// An error caused by the zip file.
    Zip(ZipError),
    /// An error caused by writing an inner `.npy` file.
    Npy(WriteNpyError),
}

impl Error for WriteNpzError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            WriteNpzError::Zip(err) => Some(err),
            WriteNpzError::Npy(err) => Some(err),
        }
    }
}

impl fmt::Display for WriteNpzError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WriteNpzError::Zip(err) => write!(f, "zip file error: {}", err),
            WriteNpzError::Npy(err) => write!(f, "error writing npy file to npz archive: {}", err),
        }
    }
}

impl From<ZipError> for WriteNpzError {
    fn from(err: ZipError) -> WriteNpzError {
        WriteNpzError::Zip(err)
    }
}

impl From<WriteNpyError> for WriteNpzError {
    fn from(err: WriteNpyError) -> WriteNpzError {
        WriteNpzError::Npy(err)
    }
}

/// Writer for `.npz` files.
///
/// Note that the inner [`ZipWriter`] is wrapped in a [`BufWriter`] when
/// writing each array with [`.add_array()`](NpzWriter::add_array). If desired,
/// you could additionally buffer the innermost writer (e.g. the
/// [`File`](std::fs::File) when writing to a file) by wrapping it in a
/// [`BufWriter`]. This may be somewhat beneficial if the arrays are large and
/// have non-standard layouts but may decrease performance if the arrays have
/// standard or Fortran layout, so it's not recommended without testing to
/// compare.
///
/// # Example
///
/// ```no_run
/// use ndarray::{array, aview0, Array1, Array2};
/// use ndarray_npy::NpzWriter;
/// use std::fs::File;
///
/// let mut npz = NpzWriter::new(File::create("arrays.npz")?);
/// let a: Array2<i32> = array![[1, 2, 3], [4, 5, 6]];
/// let b: Array1<i32> = array![7, 8, 9];
/// npz.add_array("a", &a)?;
/// npz.add_array("b", &b)?;
/// npz.add_array("c", &aview0(&10))?;
/// npz.finish()?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub struct NpzWriter<W: Write + Seek> {
    zip: ZipWriter<W>,
    options: SimpleFileOptions,
}

impl<W: Write + Seek> NpzWriter<W> {
    /// Create a new `.npz` file without compression. See [`numpy.savez`].
    ///
    /// [`numpy.savez`]: https://docs.scipy.org/doc/numpy/reference/generated/numpy.savez.html
    pub fn new(writer: W) -> NpzWriter<W> {
        NpzWriter {
            zip: ZipWriter::new(writer),
            options: SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        }
    }

    /// Creates a new `.npz` file with [`Deflated`](CompressionMethod::Deflated) compression. See
    /// [`numpy.savez_compressed`].
    ///
    /// For other compression algorithms, use [`NpzWriter::new_with_options`] or
    /// [`NpzWriter::add_array_with_options`].
    ///
    /// [`numpy.savez_compressed`]: https://docs.scipy.org/doc/numpy/reference/generated/numpy.savez_compressed.html
    #[cfg(feature = "compressed_npz")]
    pub fn new_compressed(writer: W) -> NpzWriter<W> {
        NpzWriter {
            zip: ZipWriter::new(writer),
            options: SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        }
    }

    /// Creates a new `.npz` file with the specified options to be used for each array.
    ///
    /// This allows you to use a custom compression method, such as zstd, or set other options.
    ///
    /// Make sure to enable the relevant features of the `zip` crate.
    pub fn new_with_options(writer: W, options: SimpleFileOptions) -> NpzWriter<W> {
        NpzWriter {
            zip: ZipWriter::new(writer),
            options,
        }
    }

    /// Adds an array with the specified `name` to the `.npz` file.
    ///
    /// This uses the file options passed to the `NpzWriter` constructor.
    ///
    /// Note that a `.npy` extension will be appended to `name`; this matches NumPy's behavior.
    ///
    /// To write a scalar value, create a zero-dimensional array using [`arr0`](ndarray::arr0) or
    /// [`aview0`](ndarray::aview0).
    pub fn add_array<N, T>(&mut self, name: N, array: &T) -> Result<(), WriteNpzError>
    where
        N: Into<String>,
        T: WriteNpyExt + ?Sized,
    {
        self.add_array_with_options(name, array, self.options)
    }

    /// Adds an array with the specified `name` and options to the `.npz` file.
    ///
    /// The specified options override those passed to the [`NpzWriter`] constructor (if any).
    ///
    /// Note that a `.npy` extension will be appended to `name`; this matches NumPy's behavior.
    ///
    /// To write a scalar value, create a zero-dimensional array using [`arr0`](ndarray::arr0) or
    /// [`aview0`](ndarray::aview0).
    pub fn add_array_with_options<N, T, U>(
        &mut self,
        name: N,
        array: &T,
        options: FileOptions<'_, '_, U>,
    ) -> Result<(), WriteNpzError>
    where
        N: Into<String>,
        T: WriteNpyExt + ?Sized,
        U: FileOptionExtension,
    {
        fn inner<W, T, U>(
            npz_zip: &mut ZipWriter<W>,
            name: String,
            array: &T,
            options: FileOptions<'_, '_, U>,
        ) -> Result<(), WriteNpzError>
        where
            W: Write + Seek,
            T: WriteNpyExt + ?Sized,
            U: FileOptionExtension,
        {
            npz_zip.start_file(name + ".npy", options)?;
            // Buffering when writing individual arrays is beneficial even when the
            // underlying writer is `Cursor<Vec<u8>>` instead of a real file. The
            // only exception I saw in testing was the "compressed, in-memory
            // writer, standard layout case". See
            // https://github.com/jturner314/ndarray-npy/issues/50#issuecomment-812802481
            // for details.
            array.write_npy(BufWriter::new(npz_zip))?;
            Ok(())
        }

        inner(&mut self.zip, name.into(), array, options)
    }

    /// Calls [`.finish()`](ZipWriter::finish) on the zip file and
    /// [`.flush()`](Write::flush) on the writer, and then returns the writer.
    ///
    /// This finishes writing the remaining zip structures and flushes the
    /// writer. While dropping will automatically attempt to finish the zip
    /// file and (for writers that flush on drop, such as
    /// [`BufWriter`](std::io::BufWriter)) flush the writer, any errors that
    /// occur during drop will be silently ignored. So, it's necessary to call
    /// `.finish()` to properly handle errors.
    pub fn finish(self) -> Result<W, WriteNpzError> {
        let mut writer = self.zip.finish()?;
        writer.flush().map_err(ZipError::from)?;
        Ok(writer)
    }
}

/// An error reading a `.npz` file.
#[derive(Debug)]
pub enum ReadNpzError {
    /// An error caused by the zip archive.
    Zip(ZipError),
    /// An error caused by reading an inner `.npy` file.
    Npy(ReadNpyError),
}

impl Error for ReadNpzError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ReadNpzError::Zip(err) => Some(err),
            ReadNpzError::Npy(err) => Some(err),
        }
    }
}

impl fmt::Display for ReadNpzError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ReadNpzError::Zip(err) => write!(f, "zip file error: {}", err),
            ReadNpzError::Npy(err) => write!(f, "error reading npy file in npz archive: {}", err),
        }
    }
}

impl From<ZipError> for ReadNpzError {
    fn from(err: ZipError) -> ReadNpzError {
        ReadNpzError::Zip(err)
    }
}

impl From<ReadNpyError> for ReadNpzError {
    fn from(err: ReadNpyError) -> ReadNpzError {
        ReadNpzError::Npy(err)
    }
}

/// Reader for `.npz` files.
///
/// # Example
///
/// ```no_run
/// use ndarray::{Array1, Array2};
/// use ndarray_npy::NpzReader;
/// use std::fs::File;
///
/// let mut npz = NpzReader::new(File::open("arrays.npz")?)?;
/// let a: Array2<i32> = npz.by_name("a")?;
/// let b: Array1<i32> = npz.by_name("b")?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub struct NpzReader<R: Read + Seek> {
    zip: ZipArchive<R>,
}

impl<R: Read + Seek> NpzReader<R> {
    /// Creates a new `.npz` file reader.
    pub fn new(reader: R) -> Result<NpzReader<R>, ReadNpzError> {
        Ok(NpzReader {
            zip: ZipArchive::new(reader)?,
        })
    }

    /// Returns `true` iff the `.npz` file doesn't contain any arrays.
    pub fn is_empty(&self) -> bool {
        self.zip.len() == 0
    }

    /// Returns the number of arrays in the `.npz` file.
    pub fn len(&self) -> usize {
        self.zip.len()
    }

    /// Returns the names of all of the arrays in the file.
    ///
    /// Note that a single ".npy" suffix (if present) will be stripped from each name; this matches
    /// NumPy's behavior.
    pub fn names(&mut self) -> Result<Vec<String>, ReadNpzError> {
        Ok((0..self.zip.len())
            .map(|i| {
                let file = self.zip.by_index(i)?;
                let name = file.name()?;
                let stripped = name.strip_suffix(".npy").unwrap_or(name.as_ref());
                Ok(stripped.to_owned())
            })
            .collect::<Result<_, ZipError>>()?)
    }

    /// Reads an array by name.
    ///
    /// Note that this first checks for `name` in the `.npz` file, and if that is not present,
    /// checks for `format!("{name}.npy")`. This matches NumPy's behavior.
    pub fn by_name<S, D>(&mut self, name: &str) -> Result<ArrayBase<S, D>, ReadNpzError>
    where
        S::Elem: ReadableElement,
        S: DataOwned,
        D: Dimension,
    {
        // TODO: Combine the two cases into a single `let file = match { ... }` once
        // https://github.com/rust-lang/rust/issues/47680 is resolved.
        match self.zip.by_name(name) {
            Ok(file) => return Ok(ArrayBase::<S, D>::read_npy(file)?),
            Err(ZipError::FileNotFound) => {}
            Err(err) => return Err(err.into()),
        };
        Ok(ArrayBase::<S, D>::read_npy(
            self.zip.by_name(&format!("{name}.npy"))?,
        )?)
    }

    /// Reads an array by index in the `.npz` file.
    pub fn by_index<S, D>(&mut self, index: usize) -> Result<ArrayBase<S, D>, ReadNpzError>
    where
        S::Elem: ReadableElement,
        S: DataOwned,
        D: Dimension,
    {
        Ok(ArrayBase::<S, D>::read_npy(self.zip.by_index(index)?)?)
    }
}
