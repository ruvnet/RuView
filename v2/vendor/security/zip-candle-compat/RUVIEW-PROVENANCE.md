# ZIP API adapter provenance

The adapter source is authored for RuView under the repository's MIT OR
Apache-2.0 license. No upstream ZIP implementation is copied or modified here.

Its archive implementation is the crates.io package `zip = 9.0.5`, licensed MIT.
The published archive SHA-256 recorded and verified by Cargo is
`6e04976da629d3b04204ba8105b396a193af8ad2583b2c79bfb3ab8af580a11e`.
Both the root workspace and the standalone guard workspace retain the upstream
registry identity, exact version and checksum in their respective lockfiles.

Upstream: <https://github.com/zip-rs/zip2>.
Versioned API: <https://docs.rs/zip/9.0.5/zip/>.

The local package's `7.2.0+ruview.zip9.0.5` version is explicitly a compatibility
selector for Candle 0.9's dependency requirement, not an upstream ZIP release.
The adapter validates fallible filename decoding before exposing Candle's
infallible filename access. Reading, writing, header validation and extraction
execute the upstream ZIP 9 implementation. Only the feature-free API currently
used by Candle is supported; this is not a general ZIP 7 compatibility promise.

Path packages are excluded from registry advisory matching. The absence of
findings on this adapter alone is therefore not security evidence. The registry
ZIP 9 dependency must remain visible to `cargo audit`, the standalone archive
guards must pass, and the real Candle tensor IO and consumer tests must pass.
The CI workflow runs the guards explicitly with their checked-in lockfile.
