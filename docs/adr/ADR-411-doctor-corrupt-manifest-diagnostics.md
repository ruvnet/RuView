# ADR-411: Preserve structured doctor diagnostics for package corruption

- Status: proposed
- Date: 2026-10-11
- Related: ADR-370 (doctor), ADR-394 (SDK strict diagnostic contract)
- Promotion: extend existing doctor PR #2208; no separate publication unit

## Context

The package-integrity diagnostic reports hash drift as a structured warning, but
malformed manifest JSON or a missing outer checksum throws before the doctor
report is assembled. The public SDK therefore cannot return the promised
checks/remedies for those ordinary package-corruption cases. A copied-package
fixture reproduces both exceptions without touching the installed manifest.

## Decision

Convert manifest read/parse and packaged-file read failures into integrity
findings. Report a missing or unreadable checksum explicitly. Preserve existing
hash comparison, file counts, diagnostic report shape and warning policy. SDK
strict mode continues rejecting failed reports through the existing checked
path; integrity warnings retain their current warning behavior. No additional
manifest schema or trust policy is introduced.

## Verification

The actual public SDK is imported from an owned temporary copy containing the
package source and reviewed brain. Strict doctor with groups=harness must return
an integrity warning and reinstall remedy for malformed JSON, a missing or
unreadable checksum, and unreadable packaged data. The valid-copy control retains successful integrity status. Existing
doctor and strict failed-report controls run alongside these tests. No canonical
manifest changes, credentials, network requests or hardware operations occur.
