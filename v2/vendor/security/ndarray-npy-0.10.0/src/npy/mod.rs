//! Functionality for `.npy` files.
//!
//! Most of this functionality is reexported at the top level of the crate.

mod elements;
pub mod header;

use self::header::{
    FormatHeaderError, Header, Layout, ParseHeaderError, ReadHeaderError, WriteHeaderError,
};
use ndarray::prelude::*;
use ndarray::{Data, DataOwned, IntoDimension};
use py_literal::Value as PyValue;
use std::convert::TryInto;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, BufWriter, Seek};
use std::mem;

/// Read an `.npy` file located at the specified path.
///
/// This is a convience function for using `File::open` followed by
/// [`ReadNpyExt::read_npy`](trait.ReadNpyExt.html#tymethod.read_npy).
///
/// # Example
///
/// ```
/// use ndarray::Array2;
/// use ndarray_npy::read_npy;
/// # use ndarray_npy::ReadNpyError;
///
/// let arr: Array2<i32> = read_npy("resources/array.npy")?;
/// # println!("arr = {}", arr);
/// # Ok::<_, ReadNpyError>(())
/// ```
pub fn read_npy<P, T>(path: P) -> Result<T, ReadNpyError>
where
    P: AsRef<std::path::Path>,
    T: ReadNpyExt,
{
    T::read_npy(File::open(path)?)
}

/// Writes an array to an `.npy` file at the specified path.
///
/// This function will create the file if it does not exist, or overwrite it if
/// it does.
///
/// This is a convenience function for `BufWriter::new(File::create(path)?)`
/// followed by [`WriteNpyExt::write_npy`].
///
/// # Example
///
/// ```no_run
/// use ndarray::array;
/// use ndarray_npy::write_npy;
/// # use ndarray_npy::WriteNpyError;
///
/// let arr = array![[1, 2, 3], [4, 5, 6]];
/// write_npy("array.npy", &arr)?;
/// # Ok::<_, WriteNpyError>(())
/// ```
pub fn write_npy<P, T>(path: P, array: &T) -> Result<(), WriteNpyError>
where
    P: AsRef<std::path::Path>,
    T: WriteNpyExt + ?Sized,
{
    array.write_npy(BufWriter::new(File::create(path)?))
}

/// Writes an array to a new `.npy` file at the specified path; error if the file exists.
///
/// This is a convenience function for `BufWriter::new(File::create_new(path)?)` followed by
/// [`WriteNpyExt::write_npy`].
///
/// # Example
///
/// ```no_run
/// use ndarray::array;
/// use ndarray_npy::create_new_npy;
/// # use ndarray_npy::WriteNpyError;
///
/// let arr = array![[1, 2, 3], [4, 5, 6]];
/// create_new_npy("new_array.npy", &arr)?;
/// assert!(create_new_npy("new_array.npy", &arr).is_err());
/// # Ok::<_, WriteNpyError>(())
/// ```
pub fn create_new_npy<P, T>(path: P, array: &T) -> Result<(), WriteNpyError>
where
    P: AsRef<std::path::Path>,
    T: WriteNpyExt + ?Sized,
{
    array.write_npy(BufWriter::new(File::create_new(path)?))
}

/// Writes an `.npy` file (sparse if possible) with bitwise-zero-filled data.
///
/// The `.npy` file represents an array with element type `A` and shape
/// specified by `shape`, with all elements of the array represented by an
/// all-zero byte-pattern. The file is written starting at the current cursor
/// location and truncated such that there are no additional bytes after the
/// `.npy` data.
///
/// This function is primarily useful for creating an `.npy` file for an array
/// larger than available memory. The file can then be memory-mapped and
/// modified using [`ViewMutNpyExt`].
///
/// # Panics
///
/// May panic if any of the following overflow `isize` or `u64`:
///
/// - the number of elements in the array
/// - the size of the array in bytes
/// - the size of the resulting file in bytes
///
/// # Considerations
///
/// ## Data is zeroed bytes
///
/// The data consists of all zeroed bytes, so this function is useful only for
/// element types for which an all-zero byte-pattern is a valid representation.
///
/// ## Sparse file
///
/// On filesystems which support [sparse files], most of the data should be
/// handled by empty blocks, i.e. not allocated on disk. If you plan to
/// memory-map the file to modify it and know that most blocks of the file will
/// ultimately contain some nonzero data, then it may be beneficial to allocate
/// space for the file on disk before modifying it in order to avoid
/// fragmentation. For example, on POSIX-like systems, you can do this by
/// calling `fallocate` on the file.
///
/// [sparse files]: https://en.wikipedia.org/wiki/Sparse_file
///
/// ## Alternatives
///
/// If all you want to do is create an array larger than the available memory
/// and don't care about actually writing the data to disk, it may be worth
/// considering alternative options:
///
/// - Add more swap space to your system, using swap file(s) if necessary, so
///   that you can allocate the array as normal.
///
/// - If you know the data will be sparse:
///
///   - Use a sparse data structure instead of `ndarray`'s array types. For
///     example, the [`sprs`](https://crates.io/crates/sprs) crate provides
///     sparse matrices.
///
///   - Rely on memory overcommitment. In other words, configure the operating
///     system to allocate more memory than actually exists. However, this
///     risks the system running out of memory if the data is not as sparse as
///     you expect.
///
/// # Example
///
/// In this example, a file containing 64 GiB of zeroed `f64` elements is
/// created. Then, an `ArrayViewMut` is created by memory-mapping the file.
/// Modifications to the data in the `ArrayViewMut` will be applied to the
/// backing file. This works even on systems with less than 64 GiB of physical
/// memory. On filesystems which support [sparse files], the disk space that's
/// actually used depends on how much data is modified.
///
/// ```no_run
/// use memmap2::MmapMut;
/// use ndarray::ArrayViewMut3;
/// use ndarray_npy::{write_zeroed_npy, ViewMutNpyExt};
/// use std::fs::{File, OpenOptions};
///
/// let path = "array.npy";
///
/// // Create a (sparse if supported) file containing 64 GiB of zeroed data.
/// let file = File::create(path)?;
/// write_zeroed_npy::<f64, _>(&file, (1024, 2048, 4096))?;
///
/// // Memory-map the file and create the mutable view.
/// let file = OpenOptions::new().read(true).write(true).open(path)?;
/// let mut mmap = unsafe { MmapMut::map_mut(&file)? };
/// let mut view_mut = ArrayViewMut3::<f64>::view_mut_npy(&mut mmap)?;
///
/// // Modify an element in the view.
/// view_mut[[500, 1000, 2000]] = 3.14;
/// #
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn write_zeroed_npy<A, D>(mut file: &File, shape: D) -> Result<(), WriteNpyError>
where
    A: WritableElement,
    D: IntoDimension,
{
    let dim = shape.into_dimension();
    let data_bytes_len: u64 = dim
        .size_checked()
        .expect("overflow computing number of elements")
        .checked_mul(mem::size_of::<A>())
        .expect("overflow computing length of data")
        .try_into()
        .expect("overflow converting length of data to u64");
    Header {
        type_descriptor: A::type_descriptor(),
        layout: Layout::Standard,
        shape: dim.as_array_view().to_vec(),
    }
    .write(file)?;
    let current_offset = file.stream_position()?;
    // First, truncate the file to the current offset.
    file.set_len(current_offset)?;
    // Then, zero-extend the length to represent the data (sparse if possible).
    file.set_len(
        current_offset
            .checked_add(data_bytes_len)
            .expect("overflow computing file length"),
    )?;
    Ok(())
}

/// An error writing array data.
#[derive(Debug)]
pub enum WriteDataError {
    /// An error caused by I/O.
    Io(io::Error),
    /// An error formatting the data.
    FormatData(Box<dyn Error + Send + Sync + 'static>),
}

impl Error for WriteDataError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            WriteDataError::Io(err) => Some(err),
            WriteDataError::FormatData(err) => Some(&**err),
        }
    }
}

impl fmt::Display for WriteDataError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WriteDataError::Io(err) => write!(f, "I/O error: {}", err),
            WriteDataError::FormatData(err) => write!(f, "error formatting data: {}", err),
        }
    }
}

impl From<io::Error> for WriteDataError {
    fn from(err: io::Error) -> WriteDataError {
        WriteDataError::Io(err)
    }
}

/// An array element type that can be written to an `.npy` or `.npz` file.
pub trait WritableElement: Sized {
    /// Returns a descriptor of the type that can be used in the header.
    fn type_descriptor() -> PyValue;

    /// Writes a single instance of `Self` to the writer.
    fn write<W: io::Write>(&self, writer: W) -> Result<(), WriteDataError>;

    /// Writes a slice of `Self` to the writer.
    fn write_slice<W: io::Write>(slice: &[Self], writer: W) -> Result<(), WriteDataError>;
}

/// An error writing a `.npy` file.
#[derive(Debug)]
pub enum WriteNpyError {
    /// An error caused by I/O.
    Io(io::Error),
    /// An error formatting the header.
    FormatHeader(FormatHeaderError),
    /// An error formatting the data.
    FormatData(Box<dyn Error + Send + Sync + 'static>),
}

impl Error for WriteNpyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            WriteNpyError::Io(err) => Some(err),
            WriteNpyError::FormatHeader(err) => Some(err),
            WriteNpyError::FormatData(err) => Some(&**err),
        }
    }
}

impl fmt::Display for WriteNpyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WriteNpyError::Io(err) => write!(f, "I/O error: {}", err),
            WriteNpyError::FormatHeader(err) => write!(f, "error formatting header: {}", err),
            WriteNpyError::FormatData(err) => write!(f, "error formatting data: {}", err),
        }
    }
}

impl From<io::Error> for WriteNpyError {
    fn from(err: io::Error) -> WriteNpyError {
        WriteNpyError::Io(err)
    }
}

impl From<WriteHeaderError> for WriteNpyError {
    fn from(err: WriteHeaderError) -> WriteNpyError {
        match err {
            WriteHeaderError::Io(err) => WriteNpyError::Io(err),
            WriteHeaderError::Format(err) => WriteNpyError::FormatHeader(err),
        }
    }
}

impl From<FormatHeaderError> for WriteNpyError {
    fn from(err: FormatHeaderError) -> WriteNpyError {
        WriteNpyError::FormatHeader(err)
    }
}

impl From<WriteDataError> for WriteNpyError {
    fn from(err: WriteDataError) -> WriteNpyError {
        match err {
            WriteDataError::Io(err) => WriteNpyError::Io(err),
            WriteDataError::FormatData(err) => WriteNpyError::FormatData(err),
        }
    }
}

/// Extension trait for writing [`ArrayBase`] to `.npy` files.
///
/// If writes are expensive (e.g. for a file or network socket) and the layout
/// of the array is not known to be in standard or Fortran layout, it is
/// strongly recommended to wrap the writer in a [`BufWriter`]. For the sake of
/// convenience, this method calls [`.flush()`](io::Write::flush) on the writer
/// before returning.
///
/// # Example
///
/// ```no_run
/// use ndarray::{array, Array2};
/// use ndarray_npy::WriteNpyExt;
/// use std::fs::File;
/// use std::io::BufWriter;
/// # use ndarray_npy::WriteNpyError;
///
/// let arr: Array2<i32> = array![[1, 2, 3], [4, 5, 6]];
/// let writer = BufWriter::new(File::create("array.npy")?);
/// arr.write_npy(writer)?;
/// # Ok::<_, WriteNpyError>(())
/// ```
pub trait WriteNpyExt {
    /// Writes the array to `writer` in [`.npy`
    /// format](https://docs.scipy.org/doc/numpy/reference/generated/numpy.lib.format.html).
    ///
    /// This function is the Rust equivalent of
    /// [`numpy.save`](https://docs.scipy.org/doc/numpy/reference/generated/numpy.save.html).
    fn write_npy<W: io::Write>(&self, writer: W) -> Result<(), WriteNpyError>;
}

impl<A, D> WriteNpyExt for ArrayRef<A, D>
where
    A: WritableElement,
    D: Dimension,
{
    fn write_npy<W: io::Write>(&self, mut writer: W) -> Result<(), WriteNpyError> {
        let write_contiguous = |mut writer: W, layout: Layout| {
            Header {
                type_descriptor: A::type_descriptor(),
                layout,
                shape: self.shape().to_owned(),
            }
            .write(&mut writer)?;
            A::write_slice(self.as_slice_memory_order().unwrap(), &mut writer)?;
            writer.flush()?;
            Ok(())
        };
        if self.is_standard_layout() {
            write_contiguous(writer, Layout::Standard)
        } else if self.view().reversed_axes().is_standard_layout() {
            write_contiguous(writer, Layout::Fortran)
        } else {
            Header {
                type_descriptor: A::type_descriptor(),
                layout: Layout::Standard,
                shape: self.shape().to_owned(),
            }
            .write(&mut writer)?;
            for elem in self.iter() {
                elem.write(&mut writer)?;
            }
            writer.flush()?;
            Ok(())
        }
    }
}

impl<A, S, D> WriteNpyExt for ArrayBase<S, D>
where
    A: WritableElement,
    S: Data<Elem = A>,
    D: Dimension,
{
    fn write_npy<W: io::Write>(&self, writer: W) -> Result<(), WriteNpyError> {
        let arr: &ArrayRef<A, D> = self;
        arr.write_npy(writer)
    }
}

/// An error reading array data.
#[derive(Debug)]
pub enum ReadDataError {
    /// An error caused by I/O.
    Io(io::Error),
    /// The type descriptor does not match the element type.
    WrongDescriptor(PyValue),
    /// The file does not contain all the data described in the header.
    MissingData,
    /// Extra bytes are present between the end of the data and the end of the
    /// file.
    ExtraBytes(usize),
    /// An error parsing the data.
    ParseData(Box<dyn Error + Send + Sync + 'static>),
}

impl Error for ReadDataError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ReadDataError::Io(err) => Some(err),
            ReadDataError::WrongDescriptor(_) => None,
            ReadDataError::MissingData => None,
            ReadDataError::ExtraBytes(_) => None,
            ReadDataError::ParseData(err) => Some(&**err),
        }
    }
}

impl fmt::Display for ReadDataError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ReadDataError::Io(err) => write!(f, "I/O error: {}", err),
            ReadDataError::WrongDescriptor(desc) => {
                write!(f, "incorrect descriptor ({}) for this type", desc)
            }
            ReadDataError::MissingData => write!(f, "reached EOF before reading all data"),
            ReadDataError::ExtraBytes(num_extra_bytes) => {
                write!(f, "file had {} extra bytes before EOF", num_extra_bytes)
            }
            ReadDataError::ParseData(err) => write!(f, "error parsing data: {}", err),
        }
    }
}

impl From<io::Error> for ReadDataError {
    /// Performs the conversion.
    ///
    /// If the error kind is `UnexpectedEof`, the `MissingData` variant is
    /// returned. Otherwise, the `Io` variant is returned.
    fn from(err: io::Error) -> ReadDataError {
        if err.kind() == io::ErrorKind::UnexpectedEof {
            ReadDataError::MissingData
        } else {
            ReadDataError::Io(err)
        }
    }
}

/// An array element type that can be read from an `.npy` or `.npz` file.
pub trait ReadableElement: Sized {
    /// Reads to the end of the `reader`, creating a `Vec` of length `len`.
    ///
    /// This method should return `Err(_)` in at least the following cases:
    ///
    /// * if the `type_desc` does not match `Self`
    /// * if the `reader` has fewer elements than `len`
    /// * if the `reader` has extra bytes after reading `len` elements
    fn read_to_end_exact_vec<R: io::Read>(
        reader: R,
        type_desc: &PyValue,
        len: usize,
    ) -> Result<Vec<Self>, ReadDataError>;
}

/// An error reading a `.npy` file.
#[derive(Debug)]
pub enum ReadNpyError {
    /// An error caused by I/O.
    Io(io::Error),
    /// An error parsing the file header.
    ParseHeader(ParseHeaderError),
    /// An error parsing the data.
    ParseData(Box<dyn Error + Send + Sync + 'static>),
    /// Overflow while computing the length of the array (in units of bytes or
    /// the number of elements) from the shape described in the file header.
    LengthOverflow,
    /// An error caused by incorrect `Dimension` type.
    WrongNdim(Option<usize>, usize),
    /// The type descriptor does not match the element type.
    WrongDescriptor(PyValue),
    /// The file does not contain all the data described in the header.
    MissingData,
    /// Extra bytes are present between the end of the data and the end of the
    /// file.
    ExtraBytes(usize),
}

impl Error for ReadNpyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ReadNpyError::Io(err) => Some(err),
            ReadNpyError::ParseHeader(err) => Some(err),
            ReadNpyError::ParseData(err) => Some(&**err),
            ReadNpyError::LengthOverflow => None,
            ReadNpyError::WrongNdim(_, _) => None,
            ReadNpyError::WrongDescriptor(_) => None,
            ReadNpyError::MissingData => None,
            ReadNpyError::ExtraBytes(_) => None,
        }
    }
}

impl fmt::Display for ReadNpyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ReadNpyError::Io(err) => write!(f, "I/O error: {}", err),
            ReadNpyError::ParseHeader(err) => write!(f, "error parsing header: {}", err),
            ReadNpyError::ParseData(err) => write!(f, "error parsing data: {}", err),
            ReadNpyError::LengthOverflow => write!(f, "overflow computing length from shape"),
            ReadNpyError::WrongNdim(expected, actual) => write!(
                f,
                "ndim {} of array did not match Dimension type with NDIM = {:?}",
                actual, expected
            ),
            ReadNpyError::WrongDescriptor(desc) => {
                write!(f, "incorrect descriptor ({}) for this type", desc)
            }
            ReadNpyError::MissingData => write!(f, "reached EOF before reading all data"),
            ReadNpyError::ExtraBytes(num_extra_bytes) => {
                write!(f, "file had {} extra bytes before EOF", num_extra_bytes)
            }
        }
    }
}

impl From<io::Error> for ReadNpyError {
    fn from(err: io::Error) -> ReadNpyError {
        ReadNpyError::Io(err)
    }
}

impl From<ReadHeaderError> for ReadNpyError {
    fn from(err: ReadHeaderError) -> ReadNpyError {
        match err {
            ReadHeaderError::Io(err) => ReadNpyError::Io(err),
            ReadHeaderError::Parse(err) => ReadNpyError::ParseHeader(err),
        }
    }
}

impl From<ParseHeaderError> for ReadNpyError {
    fn from(err: ParseHeaderError) -> ReadNpyError {
        ReadNpyError::ParseHeader(err)
    }
}

impl From<ReadDataError> for ReadNpyError {
    fn from(err: ReadDataError) -> ReadNpyError {
        match err {
            ReadDataError::Io(err) => ReadNpyError::Io(err),
            ReadDataError::WrongDescriptor(desc) => ReadNpyError::WrongDescriptor(desc),
            ReadDataError::MissingData => ReadNpyError::MissingData,
            ReadDataError::ExtraBytes(nbytes) => ReadNpyError::ExtraBytes(nbytes),
            ReadDataError::ParseData(err) => ReadNpyError::ParseData(err),
        }
    }
}

/// Extension trait for reading `Array` from `.npy` files.
///
/// # Example
///
/// ```
/// use ndarray::Array2;
/// use ndarray_npy::ReadNpyExt;
/// use std::fs::File;
/// # use ndarray_npy::ReadNpyError;
///
/// let reader = File::open("resources/array.npy")?;
/// let arr = Array2::<i32>::read_npy(reader)?;
/// # println!("arr = {}", arr);
/// # Ok::<_, ReadNpyError>(())
/// ```
pub trait ReadNpyExt: Sized {
    /// Reads the array from `reader` in [`.npy`
    /// format](https://docs.scipy.org/doc/numpy/reference/generated/numpy.lib.format.html).
    ///
    /// This function is the Rust equivalent of
    /// [`numpy.load`](https://docs.scipy.org/doc/numpy/reference/generated/numpy.load.html)
    /// for `.npy` files.
    fn read_npy<R: io::Read>(reader: R) -> Result<Self, ReadNpyError>;
}

impl<A, S, D> ReadNpyExt for ArrayBase<S, D>
where
    A: ReadableElement,
    S: DataOwned<Elem = A>,
    D: Dimension,
{
    fn read_npy<R: io::Read>(mut reader: R) -> Result<Self, ReadNpyError> {
        let header = Header::from_reader(&mut reader)?;
        let shape = header.shape.into_dimension();
        let ndim = shape.ndim();
        let len = shape_length_checked::<A>(&shape).ok_or(ReadNpyError::LengthOverflow)?;
        let data = A::read_to_end_exact_vec(&mut reader, &header.type_descriptor, len)?;
        ArrayBase::from_shape_vec(shape.set_f(header.layout.is_fortran()), data)
            .unwrap()
            .into_dimensionality()
            .map_err(|_| ReadNpyError::WrongNdim(D::NDIM, ndim))
    }
}

/// An error viewing a `.npy` file.
#[derive(Debug)]
#[non_exhaustive]
pub enum ViewNpyError {
    /// An error caused by I/O.
    Io(io::Error),
    /// An error parsing the file header.
    ParseHeader(ParseHeaderError),
    /// Some of the data is invalid for the element type.
    InvalidData(Box<dyn Error + Send + Sync + 'static>),
    /// Overflow while computing the length of the array (in units of bytes or
    /// the number of elements) from the shape described in the file header.
    LengthOverflow,
    /// An error caused by incorrect `Dimension` type.
    WrongNdim(Option<usize>, usize),
    /// The type descriptor does not match the element type.
    WrongDescriptor(PyValue),
    /// The type descriptor does not match the native endianness.
    NonNativeEndian,
    /// The start of the data is not properly aligned for the element type.
    MisalignedData,
    /// The file does not contain all the data described in the header.
    MissingBytes(usize),
    /// Extra bytes are present between the end of the data and the end of the
    /// file.
    ExtraBytes(usize),
}

impl Error for ViewNpyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ViewNpyError::Io(err) => Some(err),
            ViewNpyError::ParseHeader(err) => Some(err),
            ViewNpyError::InvalidData(err) => Some(&**err),
            ViewNpyError::LengthOverflow => None,
            ViewNpyError::WrongNdim(_, _) => None,
            ViewNpyError::WrongDescriptor(_) => None,
            ViewNpyError::NonNativeEndian => None,
            ViewNpyError::MisalignedData => None,
            ViewNpyError::MissingBytes(_) => None,
            ViewNpyError::ExtraBytes(_) => None,
        }
    }
}

impl fmt::Display for ViewNpyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ViewNpyError::Io(err) => write!(f, "I/O error: {}", err),
            ViewNpyError::ParseHeader(err) => write!(f, "error parsing header: {}", err),
            ViewNpyError::InvalidData(err) => write!(f, "invalid data for element type: {}", err),
            ViewNpyError::LengthOverflow => write!(f, "overflow computing length from shape"),
            ViewNpyError::WrongNdim(expected, actual) => write!(
                f,
                "ndim {} of array did not match Dimension type with NDIM = {:?}",
                actual, expected
            ),
            ViewNpyError::WrongDescriptor(desc) => {
                write!(f, "incorrect descriptor ({}) for this type", desc)
            }
            ViewNpyError::NonNativeEndian => {
                write!(f, "descriptor does not match native endianness")
            }
            ViewNpyError::MisalignedData => write!(
                f,
                "start of data is not properly aligned for the element type"
            ),
            ViewNpyError::MissingBytes(num_missing_bytes) => write!(
                f,
                "missing {} bytes of data specified in header",
                num_missing_bytes
            ),
            ViewNpyError::ExtraBytes(num_extra_bytes) => {
                write!(f, "file had {} extra bytes before EOF", num_extra_bytes)
            }
        }
    }
}

impl From<ReadHeaderError> for ViewNpyError {
    fn from(err: ReadHeaderError) -> ViewNpyError {
        match err {
            ReadHeaderError::Io(err) => ViewNpyError::Io(err),
            ReadHeaderError::Parse(err) => ViewNpyError::ParseHeader(err),
        }
    }
}

impl From<ParseHeaderError> for ViewNpyError {
    fn from(err: ParseHeaderError) -> ViewNpyError {
        ViewNpyError::ParseHeader(err)
    }
}

impl From<ViewDataError> for ViewNpyError {
    fn from(err: ViewDataError) -> ViewNpyError {
        match err {
            ViewDataError::WrongDescriptor(desc) => ViewNpyError::WrongDescriptor(desc),
            ViewDataError::NonNativeEndian => ViewNpyError::NonNativeEndian,
            ViewDataError::Misaligned => ViewNpyError::MisalignedData,
            ViewDataError::MissingBytes(nbytes) => ViewNpyError::MissingBytes(nbytes),
            ViewDataError::ExtraBytes(nbytes) => ViewNpyError::ExtraBytes(nbytes),
            ViewDataError::InvalidData(err) => ViewNpyError::InvalidData(err),
        }
    }
}

/// Extension trait for creating an [`ArrayView`] from a buffer containing an
/// `.npy` file.
///
/// The primary use-case for this is viewing a memory-mapped `.npy` file.
///
/// # Notes
///
/// - For types for which not all bit patterns are valid, such as `bool`, the
///   implementation iterates over all of the elements when creating the view
///   to ensure they have a valid bit pattern.
///
/// - Viewing an `.npy` file has more restrictions than reading it, due to
///   memory layout. Specifically:
///
///   - An error is returned if the data in the buffer is not properly aligned
///     for the element type. Typically, this should not be a concern for
///     memory-mapped files (unless an option like `MAP_FIXED` is used), since
///     memory mappings are usually aligned to a page boundary, and the `.npy`
///     format has padding such that the header size is a multiple of 64 bytes.
///
///   - An error is returned if the endianness of the data does not match the
///     endianness of the target element type. For example, multi-byte
///     primitive types such as `f32` require that the data in the file match
///     the native endianness of the machine.
///
/// # Example
///
/// This is an example of opening a readonly memory-mapped file as an
/// [`ArrayView`].
///
/// This example uses the [`memmap2`](https://crates.io/crates/memmap2) crate
/// because that appears to be the best-maintained memory-mapping crate at the
/// moment, but `view_npy` takes a `&[u8]` instead of a file so that you can
/// use the memory-mapping crate you're most comfortable with.
///
/// ```
/// # // Miri doesn't support mmap, and the file is in little endian format.
/// # if !cfg!(miri) && cfg!(target_endian = "little") {
/// use memmap2::Mmap;
/// use ndarray::ArrayView2;
/// use ndarray_npy::ViewNpyExt;
/// use std::fs::File;
///
/// let file = File::open("resources/array.npy")?;
/// let mmap = unsafe { Mmap::map(&file)? };
/// let view = ArrayView2::<i32>::view_npy(&mmap)?;
/// # println!("view = {}", view);
/// # }
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub trait ViewNpyExt<'a>: Sized {
    /// Creates an `ArrayView` from a buffer containing an `.npy` file.
    fn view_npy(buf: &'a [u8]) -> Result<Self, ViewNpyError>;
}

/// Extension trait for creating an [`ArrayViewMut`] from a mutable buffer
/// containing an `.npy` file.
///
/// The primary use-case for this is modifying a memory-mapped `.npy` file.
/// Modifying the elements in the view will modify the file. Modifying the
/// shape/strides of the view will *not* modify the shape/strides of the array
/// in the file.
///
/// Notes:
///
/// - For types for which not all bit patterns are valid, such as `bool`, the
///   implementation iterates over all of the elements when creating the view
///   to ensure they have a valid bit pattern.
///
/// - Viewing an `.npy` file has more restrictions than reading it, due to
///   memory layout. Specifically:
///
///   - An error is returned if the data in the buffer is not properly aligned
///     for the element type. Typically, this should not be a concern for
///     memory-mapped files (unless an option like `MAP_FIXED` is used), since
///     memory mappings are usually aligned to a page boundary, and the `.npy`
///     format has padding such that the header size is a multiple of 64 bytes.
///
///   - An error is returned if the endianness of the data does not match the
///     endianness of the target element type. For example, multi-byte
///     primitive types such as `f32` require that the data in the file match
///     the native endianness of the machine.
///
/// # Example
///
/// This is an example of opening a writable memory-mapped file as an
/// [`ArrayViewMut`]. Changes to the data in the view will modify the
/// underlying file.
///
/// This example uses the [`memmap2`](https://crates.io/crates/memmap2) crate
/// because that appears to be the best-maintained memory-mapping crate at the
/// moment, but `view_mut_npy` takes a `&mut [u8]` instead of a file so that
/// you can use the memory-mapping crate you're most comfortable with.
///
/// ```
/// # // Miri doesn't support mmap, and the file is in little endian format.
/// # if !cfg!(miri) && cfg!(target_endian = "little") {
/// use memmap2::MmapMut;
/// use ndarray::ArrayViewMut2;
/// use ndarray_npy::ViewMutNpyExt;
/// use std::fs;
///
/// let file = fs::OpenOptions::new()
///     .read(true)
///     .write(true)
///     .open("resources/array.npy")?;
/// let mut mmap = unsafe { MmapMut::map_mut(&file)? };
/// let view_mut = ArrayViewMut2::<i32>::view_mut_npy(&mut mmap)?;
/// # println!("view_mut = {}", view_mut);
/// # }
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub trait ViewMutNpyExt<'a>: Sized {
    /// Creates an `ArrayViewMut` from a mutable buffer containing an `.npy`
    /// file.
    fn view_mut_npy(buf: &'a mut [u8]) -> Result<Self, ViewNpyError>;
}

impl<'a, A, D> ViewNpyExt<'a> for ArrayView<'a, A, D>
where
    A: ViewElement,
    D: Dimension,
{
    fn view_npy(buf: &'a [u8]) -> Result<Self, ViewNpyError> {
        let mut reader = buf;
        let header = Header::from_reader(&mut reader)?;
        let shape = header.shape.into_dimension();
        let ndim = shape.ndim();
        let len = shape_length_checked::<A>(&shape).ok_or(ViewNpyError::LengthOverflow)?;
        let data = A::bytes_as_slice(reader, &header.type_descriptor, len)?;
        ArrayView::from_shape(shape.set_f(header.layout.is_fortran()), data)
            .unwrap()
            .into_dimensionality()
            .map_err(|_| ViewNpyError::WrongNdim(D::NDIM, ndim))
    }
}

impl<'a, A, D> ViewMutNpyExt<'a> for ArrayViewMut<'a, A, D>
where
    A: ViewMutElement,
    D: Dimension,
{
    fn view_mut_npy(buf: &'a mut [u8]) -> Result<Self, ViewNpyError> {
        let mut reader = &*buf;
        let header = Header::from_reader(&mut reader)?;
        let shape = header.shape.into_dimension();
        let ndim = shape.ndim();
        let len = shape_length_checked::<A>(&shape).ok_or(ViewNpyError::LengthOverflow)?;
        let mid = buf.len() - reader.len();
        let data = A::bytes_as_mut_slice(&mut buf[mid..], &header.type_descriptor, len)?;
        ArrayViewMut::from_shape(shape.set_f(header.layout.is_fortran()), data)
            .unwrap()
            .into_dimensionality()
            .map_err(|_| ViewNpyError::WrongNdim(D::NDIM, ndim))
    }
}

/// An error viewing array data.
#[derive(Debug)]
#[non_exhaustive]
pub enum ViewDataError {
    /// The type descriptor does not match the element type.
    WrongDescriptor(PyValue),
    /// The type descriptor does not match the native endianness.
    NonNativeEndian,
    /// The start of the data is not properly aligned for the element type.
    Misaligned,
    /// The file does not contain all the data described in the header.
    MissingBytes(usize),
    /// Extra bytes are present between the end of the data and the end of the
    /// file.
    ExtraBytes(usize),
    /// Some of the data is invalid for the element type.
    InvalidData(Box<dyn Error + Send + Sync + 'static>),
}

impl Error for ViewDataError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ViewDataError::WrongDescriptor(_) => None,
            ViewDataError::NonNativeEndian => None,
            ViewDataError::Misaligned => None,
            ViewDataError::MissingBytes(_) => None,
            ViewDataError::ExtraBytes(_) => None,
            ViewDataError::InvalidData(err) => Some(&**err),
        }
    }
}

impl fmt::Display for ViewDataError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ViewDataError::WrongDescriptor(desc) => {
                write!(f, "incorrect descriptor ({}) for this type", desc)
            }
            ViewDataError::NonNativeEndian => {
                write!(f, "descriptor does not match native endianness")
            }
            ViewDataError::Misaligned => write!(
                f,
                "start of data is not properly aligned for the element type"
            ),
            ViewDataError::MissingBytes(num_missing_bytes) => write!(
                f,
                "missing {} bytes of data specified in header",
                num_missing_bytes
            ),
            ViewDataError::ExtraBytes(num_extra_bytes) => {
                write!(f, "file had {} extra bytes before EOF", num_extra_bytes)
            }
            ViewDataError::InvalidData(err) => write!(f, "invalid data for element type: {}", err),
        }
    }
}

/// An array element type that can be viewed (without copying) in an `.npy`
/// file.
pub trait ViewElement: Sized {
    /// Casts `bytes` into a slice of elements of length `len`.
    ///
    /// Returns `Err(_)` in at least the following cases:
    ///
    ///   * if the `type_desc` does not match `Self` with native endianness
    ///   * if the `bytes` slice is misaligned for elements of type `Self`
    ///   * if the `bytes` slice is too short for `len` elements
    ///   * if the `bytes` slice has extra bytes after `len` elements
    ///
    /// May panic if `len * size_of::<Self>()` overflows.
    fn bytes_as_slice<'a>(
        bytes: &'a [u8],
        type_desc: &PyValue,
        len: usize,
    ) -> Result<&'a [Self], ViewDataError>;
}

/// An array element type that can be mutably viewed (without copying) in an
/// `.npy` file.
pub trait ViewMutElement: Sized {
    /// Casts `bytes` into a mutable slice of elements of length `len`.
    ///
    /// Returns `Err(_)` in at least the following cases:
    ///
    ///   * if the `type_desc` does not match `Self` with native endianness
    ///   * if the `bytes` slice is misaligned for elements of type `Self`
    ///   * if the `bytes` slice is too short for `len` elements
    ///   * if the `bytes` slice has extra bytes after `len` elements
    ///
    /// May panic if `len * size_of::<Self>()` overflows.
    fn bytes_as_mut_slice<'a>(
        bytes: &'a mut [u8],
        type_desc: &PyValue,
        len: usize,
    ) -> Result<&'a mut [Self], ViewDataError>;
}

/// Computes the length associated with the shape (i.e. the product of the axis
/// lengths), where the element type is `T`.
///
/// Returns `None` if the number of elements or the length in bytes would
/// overflow `isize`.
fn shape_length_checked<T>(shape: &IxDyn) -> Option<usize> {
    let len = shape.size_checked()?;
    if len > isize::MAX as usize {
        return None;
    }
    let bytes = len.checked_mul(mem::size_of::<T>())?;
    if bytes > isize::MAX as usize {
        return None;
    }
    Some(len)
}
