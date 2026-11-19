<!--
Title: use Conventional Commits, e.g. "fix(calibration): keep expired status".
Keep the PR focused on one change. See CONTRIBUTING.md for the full process.
-->

## Summary

<!-- What changed, and why? One or two short paragraphs. -->

## Related issues

<!-- "Fixes #123" closes the issue on merge; "Refs #123" just links it. -->

## Type of change

- [ ] `feat`: new capability
- [ ] `fix`: bug fix
- [ ] `docs`: documentation only
- [ ] `refactor` / `perf`: no behaviour change / performance
- [ ] `test`: tests only
- [ ] `ci` / `chore`: workflows, tooling, dependencies
- [ ] Breaking change (API, wire protocol, firmware, or config)

## Validation

<!--
List the exact commands you ran and their results, e.g.
  cd v2 && cargo test -p wifi-densepose-signal   -> ok, 0 failed
See the validation matrix in CONTRIBUTING.md.
-->

```text

```

## Evidence

<!--
Required for any accuracy or performance claim. Tag each claim:
  MEASURED (with reproducer), CLAIMED, or SYNTHETIC.
Hardware claims need a captured device log, not just a build or simulator run.
Delete this section if the PR makes no such claims.
-->

## Screenshots / logs

<!-- UI changes: before/after screenshots or a recording. Firmware: boot/runtime log. -->

## Checklist

- [ ] The PR title follows Conventional Commits
- [ ] Focused change, with no unrelated edits or mass reformatting
- [ ] Tests added or updated (a bug fix includes a test that would have caught it)
- [ ] Docs, README, or an ADR updated where behaviour or architecture changed
- [ ] `CHANGELOG.md` has an `[Unreleased]` entry for user-visible changes
- [ ] No secrets, `.env` files, credentials, CSI or personal data, or raw agent transcripts
- [ ] Claims are evidence-labelled (`MEASURED` / `CLAIMED` / `SYNTHETIC`)
