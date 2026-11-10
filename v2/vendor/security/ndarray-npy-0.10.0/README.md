# ndarray-npy

[![Dependencies status](https://deps.rs/repo/github/jturner314/ndarray-npy/status.svg)](https://deps.rs/repo/github/jturner314/ndarray-npy)
[![ndarray-npy at crates.io](https://img.shields.io/crates/v/ndarray-npy.svg)](https://crates.io/crates/ndarray-npy)
[![Documentation](https://docs.rs/ndarray-npy/badge.svg)](https://docs.rs/ndarray-npy)

This crate provides support for reading/writing [`ndarray`]'s `ArrayBase` type
from/to [`.npy`] and [`.npz`] files. See the
[documentation](https://docs.rs/ndarray-npy) for more information.

[`ndarray`]: https://github.com/bluss/ndarray
[`.npy`]: https://docs.scipy.org/doc/numpy/reference/generated/numpy.lib.format.html
[`.npz`]: https://docs.scipy.org/doc/numpy/reference/generated/numpy.savez.html

**This crate is a work-in-progress.** It currently supports only a subset of
`.npy` header descriptors and supports only primitive fixed-size integer,
primitive floating point, complex floating point, and `bool` types as the array
element type. You can implement the `*Element` traits for your own types, but
the next breaking release of this library will probably change those traits.

Future plans include support for:

* Memory-mapped `.npz` files. (Memory-mapped `.npy` files are already
  supported.)
* More element types (e.g. structs). If you need support for structs before
  this is implemented in `ndarray-npy`, check out the [`npy` crate].
* Possibly merging this with the [`npy` crate].

[`npy` crate]: https://crates.io/crates/npy

## Using with Cargo

To use with the default features:

```toml
[dependencies]
ndarray-npy = "0.10.0"
```

The `default` feature set includes the `compressed_npz` feature, which enables
support for uncompresssed and compressed `.npz` files, and the
`num-complex-0_4` feature, which enables support for complex floating point
element types provided by version 0.4 of the [`num-complex` crate]. The
`compressed_npz` feature requires a dependency on the [`zip` crate] and a
compression backend crate.

[`num-complex` crate]: https://crates.io/crates/num-complex

To use without the default features:

```toml
[dependencies]
ndarray-npy = { version = "0.10.0", default-features = false }
```

With `default-features = false`, `ndarray-npy` provides support only for `.npy`
files, not `.npz` files, and it does not provide support for complex number
elements. If you want support for `.npz` files or complex number elements, you
can select additional features:

* `npz` enables support for uncompressed `.npz` files. This requires a
  dependency on the [`zip` crate].
* `compressed_npz` enables support for uncompressed and compressed `.npz`
  files. This requires a dependency on the [`zip` crate] and a compression
  backend crate.
* `num-complex-0_4` enables support for complex floating point element types
  provided by version 0.4 of the [`num-complex` crate].

For example, you can use just the `npz` feature:

```toml
[dependencies.ndarray-npy]
version = "0.10.0"
default-features = false
features = ["npz"]
```

[`zip` crate]: https://crates.io/crates/zip

### Library authors

Library authors should specify their dependency on `ndarray-npy` like this:

```toml
[dependencies.ndarray-npy]
version = "0.10.0"
default-features = false
features = [FEATURES_LIST_HERE]
```

where the `features` list is one of the following:

* `[]` if your crate does not depend on `.npz` file support
* `["npz"]` if your crate depends on `.npz` file support but not compression
* `["compressed_npz"]` if your crate depends on `.npz` file support with compression
* `["num-complex-0_4"]` if your crate depends on support for complex element
  types but not `.npz` files
* `["num-complex-0_4", "npz"]` if your crate depends on support for complex
  element types and `.npz` files but not compression
* `["num-complex-0_4", "compressed_npz"]` if your crate depends on support for
  complex element types and `.npz` files with compression

## Releases

See [RELEASES.md](RELEASES.md).

## Contributing

Please feel free to create issues and submit PRs. PRs adding more tests would
be especially appreciated.

## License

Copyright 2018–2025 Jim Turner and `ndarray-npy` developers

Licensed under the [Apache License, Version 2.0](LICENSE-APACHE), or the [MIT
license](LICENSE-MIT), at your option. You may not use this project except in
compliance with those terms.
