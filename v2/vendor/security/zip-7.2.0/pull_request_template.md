<!-- 
We welcome your pull request, but because this crate is downloaded about 1.7 million times per month (see https://crates.io/crates/zip),
and because ZIP file processing has caused security issues in the past (see 
https://www.cvedetails.com/vulnerability-search.php?f=1&vendor=&product=zip&cweid=&cvssscoremin=&cvssscoremax=&publishdatestart=&publishdateend=&updatedatestart=&updatedateend=&cisaaddstart=&cisaaddend=&cisaduestart=&cisadueend=&page=1
for the gory details), we have some requirements that help ensure we maintain developers' and their clients' trust.
This implies some requirements that a lot of PRs don't initially meet.

This crate doesn't filter out "ZIP bombs" because extreme compression ratios and shallow file copies have legitimate uses; but
I expect the tools the crate provides for checking that extraction is safe, such as the `ZipArchive::decompressed_size` method in
https://github.com/zip-rs/zip2/blob/master/src/read.rs, to remain reliably effective. I also expect all the crate's methods to
remain panic-free, so that this crate can be used on servers without creating a denial-of-service vulnerability.

These are our requirements for PRs, in addition to the usual functionality and readability requirements:
- This codebase sometimes changes rapidly. Please rebase your branch before opening a pull request, and 
  grant @Pr0methean write access to the source branch (so I can fix later conflicts without being subject 
  to the limitations of the web UI) if EITHER of the following apply:
  - It has been at least 24 hours since you forked the repo or previously rebased the branch; or
  - 5 or more pull requests are already open at https://github.com/zip-rs/zip2/pulls. PRs are merged in the order they become
    eligible (reviewed, passing CI tests, and no conflicts with the base branch). I will attempt to fix merge
    conflicts, but this is best-effort.
- Please make sure your PR's target repo is `zip-rs/zip2` and not `zip-rs/zip-old`. The latter
  repo is no longer maintained, and I will archive it after closing the pre-existing issues.
- Your changes must build against the MSRV (see README.md) AND the latest stable Rust version AND the latest nightly Rust version.
- PRs must pass all the checks specified in `.github/workflows/ci.yaml`, which include:
  - Unit tests, run with `--no-default-features` AND with `--all-features` AND with the default features, each run
    against the MSRV (see README.md) AND the latest stable Rust version AND the latest nightly Rust version, on Windows, MacOS 
    AND Ubuntu (yes, that's a 3-dimensional matrix).
  - `cargo clippy --all-targets` and `cargo doc --no-deps` must pass with `--no-default-features` AND with `--all-features` 
    AND with the default features.
  - `cargo fmt --check --all` must pass.
- If the above checks force you to add a new `#[allow]` attribute, please place a comment on the same line or just above it, 
  explaining what the exception applies to and why it's needed.
- The PR title must conform to [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) and start 
  with one of the types specified by the [Angular convention](https://github.com/angular/angular/blob/22b96b9/CONTRIBUTING.md#type).
  This is also recommended for commit messages; but it's not required, because they'll be replaced when the PR is squash-merged.

Thanks in advance for submitting a bug fix or proposed feature that meets these requirements!
-->
