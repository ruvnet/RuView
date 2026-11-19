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

## Why native cargo-audit still reports two matches

With36.0.21, the native report drops from five to two. Its remaining0567/0573
entries omit the upstream lower bounds and therefore incorrectly match36.x.
The two wrongly named WASI entries stop matching after the patch; no special
coverage for those entries is implemented.

The existing security workflow uses audit_rust_advisories.py to retain the exact
native report and reconcile only these two reviewed range errors. This is not a
native audit-clean result: native exit1/two findings remain recorded separately
from zero unresolved findings after verified metadata reconciliation.

A correction applies only when ALL of the following hold:

- The RustSec selector fingerprint matches the checked-in ID/package/aliases/URL,
  affected-function/platform selectors and patched/unaffected ranges.
- The finding is registry core wasmtime exactly36.0.21, with its checksum matching
  both the verified registry archive and current Cargo.lock. Other versions, sources or checksums are not covered.
- Neither wasmtime-wasi nor wasmtime-wasi-http appears in the reviewed lockfile.
  A different graph requires reassessment; those packages cannot gain coverage.
- A fresh read of the fixed official GitHub repository advisory endpoint matches
  the reviewed primary package/version-range fingerprint. Network failure,
  changed identity/ranges or missing evidence cannot fall back to the fixture.
- The report's existing ignore settings match the unchanged local audit policy.

All unknown findings remain unresolved, including the version-applicable0569 if
it reappears. Malformed or incomplete reports fail closed. Fingerprints deliberately
cover scope selectors, not cosmetic prose. The primary scopes are checked live;
fixtures are used only by offline tests and never as a successful CI fallback.

## Evidence and operating boundary

The job retains cargo-audit.json (unmodified native output),
cargo-audit-classification.json (including native exit and unresolved findings),
and cargo-audit-upstream.json (live scopes and retrieval time). The existing
read-only GitHub Actions token is used by gh to read public published advisories;
no extra token, permission, secret or write operation is introduced.

Offline negative tests exercise unknown advisories, changed RustSec/primary
selectors, affected/future versions, wrong registry/checksum, missing lock records,
WASI additions, policy injection, malformed reports, unreviewed network IDs and
live-provider failure. Existing ignores remain explicit; no blanket ignore,
severity-threshold change, raw report rewrite or advisory database edit is made.

Native audit reproduction on snapshot7133866a after the patch reports two entries;
live primary verification classifies those two scope mismatches and leaves zero
unresolved entries. Remove this reconciliation when RustSec's ranges are corrected.
A future version or altered scope must be reassessed, not silently covered.

No plugin is enabled, helper replaced, application installed, physical sensor
qualified or production configuration changed by this source repair. Merge/release
remain the maintainers' separate decisions.

The CLI invokes cargo-audit directly with a fixed argument vector and shell=False.
It requires a standard Cargo.lock basename and has no alternate-database switch;
Cargo aliases and CLI-selected advisory arguments do not enter the process command.
Local pinned-snapshot evidence is reproduced by calling the native tool directly,
then exercising the pure classifier with fresh upstream evidence.

A controlled negative probe confirmed the previous Cargo dispatcher could execute
a project-defined audit alias and accept an artificial clean result. The direct
auditor ignored that alias. This is a negative-control fixture, not a real clean
audit or a claim that any repository alias was used maliciously.
