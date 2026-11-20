- Conform to the ZIP file specification at https://support.pkware.com/pkzip/appnote.
- PR titles must conform to [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) and start
  with one of the types specified by the [Angular convention](https://github.com/angular/angular/blob/22b96b9/CONTRIBUTING.md#type).
  The type prefix will usually be `fix:`, `feat:`, `perf:`, `docs:` or `chore(deps):`.
- When adding a new compression or decompression method, add a feature flag for it.
- Make sure that if new code has a dependency that's gated on a feature flag, then so is the new code.
- Carefully consider the configurations of feature flags your change will affect, and test accordingly.
- Optimize your code's performance for archives that contain many files *and* archives that contain large files.
- Consider whether your change affects ZIP64 files. If so, test it on 4GiB files both with and without compression.
  Generate these files by copying repeatedly to or from the same buffer, so that the test doesn't occupy or allocate
  4GiB of memory.
- Wait and see whether the tests for your PR pass in CI. Fix them if they fail.
- Rebase your PR whenever it has a merge conflict. If you can't fix the rebase conflicts, rewrite it from scratch.
- Always run `cargo fmt --all` and `cargo clippy --all-features --all-targets`.