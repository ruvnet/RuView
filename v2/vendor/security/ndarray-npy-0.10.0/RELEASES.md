# 0.10.0

## Breaking Changes

* Updated to `ndarray` 0.17.1.
* Updated to `zip` 6.
* Updated minimum Rust version to 1.84.
* Changed the `array` parameter of `NpzWriter::add_array` from
  `&ArrayBase<S, D>` to `T: WriteNpyExt + ?Sized` to support the new
  `ArrayRef` type in `ndarray` 0.17.1.

## New Features

* Added new `create_new_npy` convenience function which uses
  `File::create_new`. (In contrast, the existing `write_npy` function
  uses `File::create`.)
* Added `NpzWriter::add_array_with_options`, which overrides the
  options for that specific array.
* Publicly exposed the `npy` module to provide access to
  header-related functionality in the `npy::header` module. In most
  cases, users do not need this functionality, but some users have
  requested access to it for specific use cases.
* Implemented `WriteNpyExt` for the new `ArrayRef` type in `ndarray`
  0.17.1.
* Removed the implicit `Sized` bound from `write_npy` to support the
  new `ArrayRef` type in `ndarray` 0.17.1.

# 0.9.1

* Removed broken links from docs.

# 0.9.0

## Breaking Changes

* Updated to `ndarray` 0.16.
* Updated to `zip` 2.
* Updated minimum Rust version to 1.64.
* Removed `unsafe` from `WritableElement` trait declaration.
* Altered the behavior regarding `.npy` extensions of the file names within
  `.npz` archives to match NumPy's behavior, by @jturner314. See
  [#76](https://github.com/jturner314/ndarray-npy/pull/76). Specifically,
  - `NpzWriter::add_array()` now appends `.npy` to the name.
  - `NpzReader::names()` now strips a single `.npy` (if present) from the end
    of each name.
  - `NpzReader::by_name()` now tries `{name}.npy` if `{name}` is not present in
    the archive.

## New Features

* Added `NpzWriter::new_with_options()`, which allows for different compression
  options, etc., when writing `.npz` files. By @DCNick3. See
  [#66](https://github.com/jturner314/ndarray-npy/pull/66) for the motivation.

# 0.8.1

* Added support for complex floating point element types with the new
  `num-complex-0_4` feature.

# 0.8.0

* Updated to `ndarray` 0.15.
* Updated to `py_literal` 0.4.
* Updated to Rust 1.49.

# 0.7.2

* Added a `.finish()` method to `NpzWriter`. All users of `NpzWriter` should
  call this method, after the write of the last array, in order to properly
  handle errors. (Without calling `.finish()`, dropping will automatically
  attempt to finish the file, but errors will be silently ignored.)
* Changed the `write_npy` convenience function and the `NpzWriter::add_array`
  method to buffer writes using `BufWriter`. This significantly improves write
  performance for arrays which are not in standard or Fortran layout. The docs
  of `WriteNpyExt::write_npy` have also been updated to recommend that users
  wrap the writer in a `BufWriter`. Thanks to [@bluss] for reporting the
  unbuffered writes issue and reviewing the PR.
* Changed `WriteNpyExt::write_npy` to always call `.flush()` before returning.
  This is convenient when the writer passed to `WriteNpyExt::write_npy` is
  wrapped in a `BufWriter`.

# 0.7.1

* Added support for viewing byte slices as `.npy` files, primarily for use with
  memory-mapped files. See the `ViewNpyExt` and `ViewMutNpyExt` extension
  traits. By [@n3vu0r] and [@jturner314].
* Added support for creating files larger than available memory with
  `write_zeroed_npy`.
* Improved handling of overflow in the number of bytes to read as specified by
  the shape and element type in the `.npy` file header. Before, if the number
  of bytes of data was more than `isize::MAX`, the implementation would attempt
  to create the array anyway and panic. Now, it detects this case before
  attempting to create the array and returns
  `Err(ReadNpyError::LengthOverflow)` instead.

# 0.7.0

* Updated to `ndarray` 0.14.
* Updated to Rust 1.42.

# 0.6.0

* Changed `write_npy` to take the array by reference instead of by value, by
  [@KarelPeeters].

# 0.5.0

* Updated to `ndarray` 0.13.
* Updated to Rust 1.38.
* Added `read_npy` and `write_npy` convenience functions.
* Added support for `npy` format version 3.0.
* Renamed `ReadableElement::read_vec` to `::read_to_end_exact_vec`.
* Refactored the error types and variants, including removing the associated
  `Error` type from `Readable/WritableElement` and updating to the new style of
  `std::error::Error` implementation.
* Updated the padding calculation to make the total header length be divisible
  by 64 instead of just 16 when writing files. (See
  [numpy/numpy#9025](https://github.com/numpy/numpy/pull/9025).)
* Fixed determination of file format version when the addition of padding
  changes the required version when writing the file.
* Fixed miscellaneous bugs related to overflow and error handling.

# 0.4.0

* Added support for reading/writing arrays of `bool`, by [@tobni] and
  [@jturner314].
* Updated to `zip` 0.5.
* Updated to Rust 1.32.
* Renamed the `compressed_npz_default` feature to `compressed_npz` because the
  `zip` crate no longer allows the user to select the compression backend.

# 0.3.0

* Updated to `ndarray` 0.12.
* Updated to `num-traits` 0.2 (replacing dependency on `num`).
* Updated to `py_literal` 0.2.

# 0.2.0

* Updated to `zip` 0.4.
* Made the compression backend for compressed `.npz` files user-selectable.
* Reworked how feature selection works. This should only affect you if you use
  `default-features = false, features = ["compressed_npz"]`.

# 0.1.1

* Improved crate documentation (no functional changes).

# 0.1.0

* Initial release.

[@bluss]: https://github.com/bluss/
[@jturner314]: https://github.com/jturner314/
[@KarelPeeters]: https://github.com/KarelPeeters/
[@n3vu0r]: https://github.com/n3vu0r/
[@tobni]: https://github.com/tobni/
