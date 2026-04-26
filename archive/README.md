# Archive

Frozen, no-longer-active components of RuView preserved for historical
reference, reproducibility, and load-bearing legacy paths the active
codebase still depends on.

## What lives here

| Path | What it is | Why it's archived | Still load-bearing? |
|------|------------|-------------------|---------------------|
| `v1/` | Original Python implementation of RuView (CSI processing, hardware adapters, services, FastAPI) | Superseded by the Rust workspace at `v2/`; ~810× slower in benchmarks. Kept rather than deleted because the deterministic proof bundle (`v1/data/proof/`) is part of the pre-merge witness verification process per ADR-011 / ADR-028. | **Yes — for the proof bundle only.** Active code lives in `v2/`. |

## What "archived" means

- **Do not add new features here.** New work goes in `v2/`.
- **Do not refactor or modernize the archived code beyond what is
  strictly necessary** to keep the load-bearing paths working. The
  Python proof bundle is intentionally frozen so that its SHA-256
  reproducibility holds across releases (per ADR-028's witness
  verification requirement).
- **Bug fixes inside archived code are allowed** when the bug affects a
  still-load-bearing path (currently: only the Python proof). All
  other "bugs" in archived code are out-of-scope — they are part of
  the historical record and any fix would unnecessarily churn the
  witness hashes.
- **CI continues to verify the load-bearing paths.**
  `.github/workflows/verify-pipeline.yml` runs the Python proof on
  every push and PR; if you change anything inside `archive/v1/src/`
  or `archive/v1/data/proof/`, expect the determinism check to flag
  it.

## Quick reference for the load-bearing paths

```bash
# Run the deterministic Python proof (must print VERDICT: PASS)
python archive/v1/data/proof/verify.py

# Regenerate the expected hash (only if numpy/scipy version legitimately changed)
python archive/v1/data/proof/verify.py --generate-hash

# Run the full Python test suite (legacy, still maintained)
cd archive/v1&& python -m pytest tests/ -x -q
```

## Why we keep `v1/` rather than delete it

1. **Trust kill-switch.** The proof at `v1/data/proof/verify.py` feeds
   a known reference signal through the full pipeline and hashes the
   output. If the active code's behavior drifts, the hash changes and
   CI fails. This is what stops accidental regression in the science
   layer of the codebase.

2. **Witness verification.** ADR-028's witness-bundle process bundles
   the proof, the rust workspace test results, and firmware hashes
   into a tarball recipients can self-verify. Removing v1 would break
   that chain.

3. **Historical reference.** ADR-011 documents the "no mocks in
   production code" decision; the original violations and their fixes
   live in this Python codebase. The ADRs reference these paths.

If the time comes to retire the proof bundle (e.g., a Rust port of
the proof exists and the Python version is no longer canonical), the
right move is a single follow-up that simultaneously: ports the
witness-bundle process, updates `verify-pipeline.yml`, and either
deletes `archive/v1/` or moves it to a separate read-only repository.
That decision belongs in its own ADR.

## See also

- `docs/adr/ADR-011-python-proof-of-reality-mock-elimination.md`
- `docs/adr/ADR-028-esp32-capability-audit.md`
- `archive/v1/data/proof/README.md` (if present)
- `docs/WITNESS-LOG-028.md`
