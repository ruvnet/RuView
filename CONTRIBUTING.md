# Contributing to RuView

Thanks for helping improve RuView. This guide covers how to propose a change,
how to validate it, and the rules every pull request must follow.

The same rules bind human and AI contributors. [`CLAUDE.md`](CLAUDE.md) and
[`AGENTS.md`](AGENTS.md) are the authoritative agent contracts; this file is the
human-readable version of the same expectations.

## Ground rules

These are non-negotiable and reviewers will block a PR that breaks them:

- **Honest evidence.** Never present WiFi sensing as camera-grade. Every
  accuracy or performance statement is tagged `MEASURED` (with a reproducer),
  `CLAIMED`, or `SYNTHETIC`. Pose PCK numbers also need the mean-pose baseline
  and a leakage-free held-out split.
- **Real hardware means real logs.** A successful build or a simulator run is
  not hardware validation. Hardware claims need captured evidence from the
  target device, normally a boot or runtime log.
- **Never commit** credentials, `.env` files, WiFi passwords, raw agent
  transcripts, private memory or vector indexes, CSI or personal data, or
  unreviewed generated artifacts. Use [`example.env`](example.env) as the
  template for local configuration.
- **Smallest coherent change.** Read before editing, keep unrelated changes out
  of the PR, and avoid broad mechanical rewrites (mass reformatting, renames)
  unless that is the stated purpose of the PR.
- **Least authority.** Validate untrusted input at every process, network,
  hardware, FFI, MCP, and file boundary. Don't add permission or sandbox bypass
  flags.

## Before you start

1. **Search existing issues and pull requests** so you don't duplicate work.
2. **Open an issue first for large changes.** New subsystems, wire-protocol
   changes, new dependencies, or anything touching security boundaries should
   be discussed before you write a lot of code.
3. **Write an ADR for architectural decisions.** See
   [Architecture decisions](#architecture-decisions) below.

## Repository map

| Path | What lives there |
|---|---|
| `v2/crates/` | Rust workspace: production crates and their tests (the active implementation) |
| `python/` | `wifi-densepose` PyPI wheel (PyO3 + maturin) |
| `archive/v1/` | Frozen Python reference pipeline and the deterministic proof |
| `firmware/esp32-csi-node/` | ESP32-S3 / ESP32-C6 firmware and provisioning |
| `harness/ruview/`, `harness/homecore/` | Contributor CLI / MCP harnesses |
| `ui/`, `dashboard/` | Web UI, dashboard PWA, and mobile app (`ui/mobile/`) |
| `docs/adr/` | Architecture Decision Records |
| `.github/workflows/` | CI and release gates (authoritative) |

Several directories under `vendor/` and `v2/crates/` are git submodules; see
[`.gitmodules`](.gitmodules).

## Development setup

```bash
# Fork on GitHub, then clone your fork with submodules
git clone --recurse-submodules https://github.com/<your-username>/RuView.git
cd RuView
git remote add upstream https://github.com/ruvnet/RuView.git
```

Install only what the area you are working on needs:

| Area | Prerequisites |
|---|---|
| Rust (`v2/`) | [rustup](https://rustup.rs). The toolchain is pinned in [`v2/rust-toolchain.toml`](v2/rust-toolchain.toml) and installs automatically. |
| Python wheel (`python/`) | Python 3.10+, `maturin`, and the Rust toolchain above |
| Harnesses (`harness/`) | Node.js 20+ |
| Firmware (`firmware/`) | ESP-IDF; follow [`firmware/esp32-csi-node/README.md`](firmware/esp32-csi-node/README.md) |
| Docker demo | Docker; see the [User Guide](docs/user-guide.md) |

[`docs/build-guide.md`](docs/build-guide.md) has full build-from-source
instructions.

## Making a change

### 1. Branch from an up-to-date `main`

```bash
git fetch upstream
git switch -c feat/short-description upstream/main
```

Use a prefix that matches the change: `feat/`, `fix/`, `docs/`, `refactor/`,
`test/`, `ci/`, or `chore/`.

### 2. Commit with Conventional Commits

```text
<type>(<optional scope>): <imperative summary>
```

| Type | Use for |
|---|---|
| `feat` | New user-visible capability |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `refactor` | Code change with no behaviour change |
| `test` | Adding or fixing tests |
| `perf` | Performance improvement |
| `ci` | Workflow or CI configuration |
| `chore` | Maintenance (dependencies, tooling) |

Scope by subsystem, for example `fix(calibration): …`, `feat(firmware): …`,
`docs(adr-380): …`. Keep each commit focused; "stuff" or "updates" are not
reviewable.

### 3. Validate at the nearest boundary

Run the focused checks for what you touched first, then the wider gate if your
change affects shared contracts, release paths, or security boundaries.

| You changed | Run |
|---|---|
| A Rust crate | `cd v2 && cargo test -p <crate>` while iterating, then `cargo test --workspace --no-default-features` |
| Rust code style | `cargo clippy -p <crate> --all-targets -- -D warnings` |
| `python/` | In a virtualenv: `cd python && maturin develop --features sota && pytest tests/ -q` (test dependencies are listed in [`python-ci.yml`](.github/workflows/python-ci.yml)) |
| `archive/v1/` | `python archive/v1/data/proof/verify.py` (must print `VERDICT: PASS`), then `cd archive/v1 && python -m pytest tests/ -x -q` |
| `harness/ruview/` | `cd harness/ruview && npm ci --ignore-scripts && npm test && npm run test:security` |
| `harness/homecore/` | `cd harness/homecore && npm ci --ignore-scripts && npm test && npm run test:security` |
| Firmware | Build for your target per the firmware README; attach a captured device log for any hardware claim |

[`CLAUDE.md`](CLAUDE.md#validation-matrix) holds the complete validation matrix.

**Formatting:** format the files you change (`rustfmt`, Ruff for `python/`,
ESLint in `ui/`). Many existing files predate these tools, so don't reformat
files you aren't otherwise changing. A pure formatting change belongs in its own
PR.

### 4. Update docs and the changelog

- User-visible changes get an entry under `## [Unreleased]` in
  [`CHANGELOG.md`](CHANGELOG.md) (Keep a Changelog format).
- Update the README or `docs/` when behaviour, commands, or flags change.
- Don't hardcode volatile counts (crates, ADRs, tests) in docs. Derive them
  when you need them.

### 5. Open the pull request

Push your branch to your fork and open a PR against `ruvnet/RuView:main`. The
PR template asks for:

- what changed and why, with linked issues (`Fixes #123`)
- the exact validation commands you ran and their results
- evidence labels for any accuracy or performance claim
- screenshots or recordings for UI changes, and device logs for hardware changes

Keep PRs small enough to review in one sitting. Respond to review by pushing
new commits rather than force-pushing over the discussion.

## Architecture decisions

Substantive design changes need an Architecture Decision Record in
[`docs/adr/`](docs/adr/README.md):

1. Name the file `ADR-<number>-<short-slug>.md`. Check `main` **and open PRs**
   for the next free number. If two branches take the same number, the one that
   merges second renumbers.
2. Use the sections recent ADRs use: **Status**, **Context**, **Decision**,
   **Consequences**, **Validation**.
3. Treat accepted ADRs as authoritative; supersede them with a new ADR rather
   than rewriting history.

## AI-assisted contributions

AI tools are welcome. The repository ships Claude Code and Codex harnesses
(see [`harness/ruview/README.md`](harness/ruview/README.md)). Whoever opens the
PR is accountable for every line in it: review generated code, run the
validation yourself, and don't submit claims the evidence doesn't support.

## Reporting bugs and security issues

- Bugs and feature requests: open an issue using the templates.
- **Security vulnerabilities: do not open a public issue.** Follow
  [`SECURITY.md`](SECURITY.md).

## Code of conduct

Participation in this project is governed by the
[Code of Conduct](CODE_OF_CONDUCT.md).

## License

By contributing, you agree that your contributions are licensed under the
project's [MIT License](LICENSE).
