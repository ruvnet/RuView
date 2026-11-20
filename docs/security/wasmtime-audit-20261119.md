# Wasmtime 36.x patch and advisory metadata reconciliation

## Problem and actual patch

RustSec database7133866a reported five entries against core wasmtime36.0.20.
Primary upstream advisory HTML and repository advisory API distinguish the actual
package/version scopes from those raw matches:

| RustSec ID | Upstream scope | Result for this workspace |
|---|---|---|
|0569|[Core below36.0.21](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-hj27-mpw9-ff4c)|Version-applicable; patch to36.0.21|
|0567|[Core branches starting at39](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-2674-w29h-wqvj)|36.x outside affected ranges|
|0573|[Core branches starting at47](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-wvgq-g5hf-6gm6)|36.x outside affected ranges|
|0566|[wasmtime-wasi](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-5m8g-xhq4-4h93)|Package absent; RustSec matched the wrong package|
|0571|[wasmtime-wasi-http](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-f8mm-ggr3-89wh)|Package absent; RustSec matched the wrong package|

The compatible36.0.21 patch updates32 Wasmtime/Cranelift family lock entries,
retains Rust1.89 compatibility and keeps the optional backend default-off. The
internal cache is re-vendored from its actual36.0.21 archive, preserving the
reviewed zstd0.14.3 manifest correction and byte-identical upstream executable
source/tests/licenses. Existing audit.toml ignores are unchanged.

An independent controlled imported/re-exported host-function regression panics in
36.0.20's get_module_export implementation. The same test passes with36.0.21.
It is retained under the optional Wasmtime feature. This tests the dependency
patch; it is not a claim that the existing application invokes that faulty API or
that a deployed exploit was demonstrated.

## Upstream correction and native audit — 2026-11-20

RustSec commit `074ea9a7ed619f511b5b7374c7a97e98d1e98be4` corrected both
range errors by recording `<39.0.0` and `<47.0.0` as unaffected for0567/0573.
With the patched lockfile, the CI-pinned cargo-audit0.22.2 now exits0 with zero
native registry findings under the unchanged audit policy.

The temporary selector reconciliation, fixtures and live GitHub advisory API
calls have therefore been removed. CI again retains only the unmodified native
JSON report and uses its exit status, with pipefail preserving auditor failure.
No custom classification, extra token, offline database or advisory exception
is needed. Any future native advisory failure remains a job failure.

Invoke `cargo-audit audit --file Cargo.lock --json` directly from `v2/`.
Direct binary invocation avoids executing a project-defined Cargo audit alias;
the installed tool remains pinned to0.22.2. An actual controlled Cargo-alias
negative probe established that the old dispatcher could execute a benign marker
and return artificial clean data; the direct auditor ignored that alias. Those
negative-control reports are not real security evidence.

The dependency patch remains necessary:36.x before36.0.21 is still affected by
0569. The controlled API regression and archive provenance checks above remain.
Historical raw reports and reconciliation receipts describe the earlier database,
not the current native result. cargo-audit0.22.1 separately reports local path ZIP
packages as registry matches; it is not the pinned CI tool. Actual registry ZIP9
and adapter tests remain separately relevant to the held server integration.

No plugin is enabled, helper replaced, app installed or physical sensor qualified
by this change. Merge and release remain separate maintainer decisions.
