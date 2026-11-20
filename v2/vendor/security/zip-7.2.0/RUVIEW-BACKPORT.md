# RuView ZIP 7.2.0 security backport

This is the official crates.io ZIP 7.2.0 source package with the final upstream
[ZIP #984](https://github.com/zip-rs/zip2/pull/984) fix adapted to its archive and
stream readers. Version and licenses remain unchanged. Source/archive/fix hashes
are recorded in `ruview-provenance.json`; the production diff is
`RUSTSEC-2026-0429.patch` (two files). The only additional manifest entry enables
our self-contained public-API regression target. No Candle/framework snapshot or
RuView submodule revision changes are required.

The patch rejects symlink target lengths over4096 before allocation. The exact
112-byte RUSTSEC-2026-0429 example aborts pristine7.2.0 and is rejected with
InvalidArchive here. Dedicated tests cover hostile lengths, the stream flag,
ordinary files and valid relative symlinks. A removed-guard mutation fails.

Run from the repository root:

```sh
cargo test --manifest-path v2/vendor/security/zip-7.2.0/Cargo.toml --locked --no-default-features --features deflate --test ruview_0429_backport
```

The registry package omits large upstream test archives. Therefore this package
is excluded from the parent workspace's unit-test membership; the explicit
regression target above is mandatory in security CI. Full upstream library and
selected integration tests were separately executed with fixtures from package
VCS46dc29cda6749736ddb1ec502aff06a65ffe54f8; they are not claimed as part of this
small committed fixture set.

cargo-audit0.22.2 excludes path sources from registry advisory matching. A green
audit alone does not establish this patch's correctness; executable regressions,
provenance and review do. No advisory policy or exception was added or broadened.
This addresses0429 only; existing0367/0368/0369 reachability decisions remain
unchanged. Retire this backport when a reviewed compatible upstream upgrade is
available. Current Candle0.9 requires ZIP7, while moving the swarm submodule to
its new upstream workspace would be a separate integration change.
