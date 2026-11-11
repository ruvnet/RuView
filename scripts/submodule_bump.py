#!/usr/bin/env python3
"""Per-submodule validation for the vendored gitlink updates.

Issue #1743 asks the automated submodule bump to stop proposing a bundled,
unbounded multi-submodule branch-tip sweep that carries no machine-verifiable
upstream or consumer validation. The old workflow advanced every gitlink in
one commit and opened one pull request whose body was a single sentence.

This module produces the receipt the workflow records on every
per-submodule pull request:

- the old and the new commit, and the comparison link between them;
- whether the target commit is reachable from the configured tracked branch,
  so a moved or rewritten branch cannot be picked up silently;
- the upstream check runs and the combined status of that exact target
  commit, so a reviewer sees the upstream's own verdict and not only the
  superproject's;
- the RuView checks that must pass on the proposed gitlink set.

Everything except the git and GitHub API calls is pure, and unit tested in
`python/tests/test_submodule_bump.py`.
"""

from __future__ import annotations

import argparse
import configparser
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

# The RuView checks that must pass on a pull request that changes a gitlink.
# The workflow cannot make a pull request unmergeable on its own: that is
# branch protection, a repository setting. The receipt names the checks so
# the requirement is explicit and reviewable.
REQUIRED_RUVIEW_CHECKS = (
    "Continuous Integration",
    "Security Scanning",
    "CSI data policy (ADR-299)",
    "Fix-Marker Regression Guard",
)

# A tracked submodule without `branch =` follows the remote's default branch.
DEFAULT_BRANCH_LABEL = "default HEAD"


class SubmoduleError(RuntimeError):
    """A submodule could not be resolved or validated."""


@dataclass(frozen=True)
class Submodule:
    """One tracked submodule and the branch it follows."""

    path: str
    url: str
    branch: str | None

    @property
    def branch_label(self) -> str:
        return self.branch or DEFAULT_BRANCH_LABEL

    @property
    def name(self) -> str:
        return self.path.replace("/", "-")


@dataclass(frozen=True)
class UpstreamVerdict:
    """The upstream's own verdict on one exact commit."""

    context: str
    total: int = 0
    passed: int = 0
    failed: int = 0
    pending: int = 0
    skipped: int = 0
    missing: bool = False

    @property
    def has_failures(self) -> bool:
        """True when the upstream reported at least one failing check."""
        return self.failed > 0

    @property
    def has_verdict(self) -> bool:
        """True when the upstream reported anything at all for this commit."""
        return not self.missing

    def as_dict(self) -> dict:
        return {
            "context": self.context,
            "total": self.total,
            "passed": self.passed,
            "failed": self.failed,
            "pending": self.pending,
            "skipped": self.skipped,
            "missing": self.missing,
            "has_failures": self.has_failures,
            "has_verdict": self.has_verdict,
        }


@dataclass
class BumpPlan:
    """One submodule's proposed move, with its validation."""

    submodule: Submodule
    old_commit: str
    new_commit: str
    reachable: bool
    branch_tip: str
    upstream: UpstreamVerdict = field(default_factory=lambda: UpstreamVerdict("unknown"))

    @property
    def changed(self) -> bool:
        return self.old_commit != self.new_commit

    @property
    def comparison_url(self) -> str:
        return comparison_url(self.submodule.url, self.old_commit, self.new_commit)

    @property
    def blocking(self) -> list[str]:
        """Reasons the proposed move must not be merged as it stands.

        A pointer that does not move has nothing to block. A pending upstream
        check is reported but does not block: a branch tip whose own CI is
        still running is not a broken commit. A failing upstream check does
        block, because the move would import a commit the upstream itself
        rejects.
        """
        if not self.changed:
            return []
        reasons: list[str] = []
        if not self.reachable:
            reasons.append(
                f"the target commit is not reachable from the tracked branch "
                f"`{self.submodule.branch_label}`"
            )
        if self.upstream.has_failures:
            reasons.append(
                f"the upstream reports {self.upstream.failed} failing check(s) "
                "for this exact commit"
            )
        return reasons


# --------------------------------------------------------------------------
# Pure helpers
# --------------------------------------------------------------------------


def parse_gitmodules(text: str) -> list[Submodule]:
    """Parse `.gitmodules` into the tracked submodule set, in file order.

    A submodule without a `branch` key follows the remote's default branch.
    """
    parser = configparser.ConfigParser(strict=False, interpolation=None)
    parser.read_string(text)
    tracked: list[Submodule] = []
    for section in parser.sections():
        if not section.startswith("submodule "):
            continue
        path = parser.get(section, "path", fallback=None)
        url = parser.get(section, "url", fallback=None)
        if not path or not url:
            continue
        branch = parser.get(section, "branch", fallback=None)
        tracked.append(Submodule(path=path.strip(), url=url.strip(), branch=(branch or None)))
    return tracked


def parse_github_url(url: str) -> tuple[str, str]:
    """Return `(owner, repo)` for a GitHub URL, with or without `.git`.

    Both the https form (`https://github.com/owner/repo`) and the SSH form
    (`git@github.com:owner/repo.git`) are accepted.
    """
    cleaned = url.strip()
    cleaned = re.sub(r"^https?://", "", cleaned)
    cleaned = re.sub(r"^git@", "", cleaned)
    if ":" in cleaned:
        # The SSH form separates the host from the path with a colon.
        cleaned = cleaned.split(":", 1)[1]
    cleaned = re.sub(r"\.git$", "", cleaned)
    cleaned = cleaned.rstrip("/")
    parts = [part for part in cleaned.split("/") if part]
    if len(parts) < 2:
        raise SubmoduleError(f"not a GitHub URL: {url}")
    if len(parts) > 2 and "." in parts[0]:
        # An https URL still carries the host as its first segment.
        parts = parts[1:]
    return parts[0], parts[1]


def comparison_url(url: str, old_commit: str, new_commit: str) -> str:
    """The upstream comparison link between two commits."""
    owner, repo = parse_github_url(url)
    return f"https://github.com/{owner}/{repo}/compare/{old_commit}...{new_commit}"


def reachability_verdict(reachable: bool) -> str:
    """The wording used for the reachability row of the receipt."""
    return "yes" if reachable else "**no**"


def upstream_verdict_from_payloads(
    check_runs: list[dict] | None,
    combined_status: dict | None,
) -> UpstreamVerdict:
    """Fold the two GitHub status endpoints into one verdict.

    `check_runs` is the payload of `GET /repos/{owner}/{repo}/commits/{sha}/check-runs`
    and `combined_status` the payload of `GET /repos/{owner}/{repo}/commits/{sha}/status`.
    """
    runs = (check_runs or {}).get("check_runs") or []
    total = 0
    passed = 0
    failed = 0
    pending = 0
    skipped = 0
    for run in runs:
        total += 1
        conclusion = run.get("conclusion")
        status = run.get("status")
        if conclusion == "success":
            passed += 1
        elif conclusion == "skipped":
            skipped += 1
        elif conclusion in ("failure", "timed_out", "cancelled", "action_required", "stale"):
            failed += 1
        elif conclusion is None or status != "completed":
            pending += 1
        else:
            failed += 1
    if combined_status is not None:
        state = combined_status.get("state")
        total += 1
        if state == "success":
            passed += 1
        elif state in ("pending", "expected"):
            pending += 1
        else:
            failed += 1
    return UpstreamVerdict(
        context="check runs",
        total=total,
        passed=passed,
        failed=failed,
        pending=pending,
        skipped=skipped,
        missing=total == 0,
    )


def render_pr_body(
    plan: BumpPlan,
    required_checks: tuple[str, ...] = REQUIRED_RUVIEW_CHECKS,
) -> str:
    """The pull request body: the receipt a reviewer reads before merging."""
    sub = plan.submodule
    upstream = plan.upstream
    if upstream.missing:
        upstream_row = (
            "**no checks reported for this commit** — the upstream has no "
            "attributable workflow run or combined status for it"
        )
    else:
        upstream_row = (
            f"{upstream.total} run, {upstream.passed} passed, "
            f"{upstream.failed} failed, {upstream.pending} pending"
            + (f", {upstream.skipped} skipped" if upstream.skipped else "")
        )
    checks = "\n".join(f"- {check}" for check in required_checks)
    blocking = plan.blocking
    if blocking:
        gate = "\n".join(f"- {reason}" for reason in blocking)
        gate = (
            "This proposal is not ready to merge:\n\n"
            f"{gate}\n\n"
            "Re-run the bump after the upstream settles, or hold the gitlink "
            "where it is."
        )
    else:
        gate = (
            "The target commit is reachable from the tracked branch and the "
            "upstream reports no failing check for that exact commit. The "
            "checks below must still pass on this pull request before it is "
            "merged."
        )
    return f"""## Automated submodule update: `{sub.path}`

One submodule per pull request. This pull request changes **only** this
gitlink, so a failure in one vendored dependency cannot be attributed to
another.

| | |
|---|---|
| Submodule | `{sub.path}` |
| Upstream | {sub.url} |
| Tracked branch | `{sub.branch_label}` |
| Old commit | `{plan.old_commit}` |
| New commit | `{plan.new_commit}` |
| Comparison | {plan.comparison_url} |
| Reachable from `{sub.branch_label}` | {reachability_verdict(plan.reachable)} |

### Upstream status for `{plan.new_commit}`

| | |
|---|---|
| Check runs and combined status | {upstream_row} |

### Required on this pull request

{checks}

{gate}
"""


def render_commit_message(plan: BumpPlan) -> str:
    """The commit message for one submodule's gitlink move."""
    sub = plan.submodule
    return (
        f"chore: update {sub.path} to {plan.new_commit[:12]}\n\n"
        f"Old commit: {plan.old_commit}\n"
        f"New commit: {plan.new_commit}\n"
        f"Tracked branch: {sub.branch_label}\n"
        f"Comparison: {plan.comparison_url}\n"
    )


# --------------------------------------------------------------------------
# Git and GitHub access
# --------------------------------------------------------------------------


def run_git(args: list[str], cwd: Path | None = None) -> str:
    """Run a git command and return its stdout, or raise."""
    result = subprocess.run(
        ["git", *args],
        cwd=str(cwd) if cwd else None,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise SubmoduleError(
            f"git {' '.join(args)} failed ({result.returncode}): {result.stderr.strip()}"
        )
    return result.stdout.strip()


def gitlink_commit(repo: Path, path: str, ref: str = "HEAD") -> str:
    """The commit a gitlink points at, in the superproject index or a commit."""
    entry = run_git(["ls-tree", ref, "--", path], cwd=repo)
    match = re.search(r"^160000 commit ([0-9a-f]{40})", entry, re.MULTILINE)
    if not match:
        raise SubmoduleError(f"{path} is not a gitlink in {ref}")
    return match.group(1)


def fetch_upstream_tip(submodule: Submodule, repo: Path) -> tuple[str, str]:
    """Fetch the tracked branch of one submodule and return `(tip, label)`.

    The tip is the commit the tracked branch points at, fetched into the
    submodule so the reachability check and the pointer move use the same
    commit.
    """
    submodule_dir = repo / submodule.path
    if not submodule_dir.is_dir():
        raise SubmoduleError(f"{submodule.path} is not checked out")
    if submodule.branch:
        run_git(["fetch", "--depth", "1", "origin", submodule.branch], cwd=submodule_dir)
        return run_git(["rev-parse", "FETCH_HEAD"], cwd=submodule_dir), submodule.branch
    run_git(["fetch", "--depth", "1", "origin", "HEAD"], cwd=submodule_dir)
    return run_git(["rev-parse", "FETCH_HEAD"], cwd=submodule_dir), "origin/HEAD"


def is_ancestor(submodule: Submodule, repo: Path, commit: str, tip: str) -> bool:
    """Whether `commit` is reachable from `tip` in the submodule."""
    submodule_dir = repo / submodule.path
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", commit, tip],
        cwd=str(submodule_dir),
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return True
    if result.returncode == 1:
        return False
    raise SubmoduleError(
        f"git merge-base --is-ancestor failed in {submodule.path}: {result.stderr.strip()}"
    )


def _api_get(path: str, token: str | None) -> dict:
    request = urllib.request.Request(
        f"https://api.github.com{path}",
        headers={
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "ruview-submodule-bump",
            **({"Authorization": f"Bearer {token}"} if token else {}),
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return {}
        raise SubmoduleError(f"GitHub API {path} failed: {error}") from error
    except urllib.error.URLError as error:
        raise SubmoduleError(f"GitHub API {path} unreachable: {error}") from error


def upstream_status(submodule: Submodule, commit: str, token: str | None) -> UpstreamVerdict:
    """The upstream's own verdict on one exact commit."""
    owner, repo = parse_github_url(submodule.url)
    check_runs = _api_get(
        f"/repos/{owner}/{repo}/commits/{commit}/check-runs?per_page=100", token
    )
    combined = _api_get(f"/repos/{owner}/{repo}/commits/{commit}/status", token)
    return upstream_verdict_from_payloads(check_runs, combined or None)


# --------------------------------------------------------------------------
# Planning and applying
# --------------------------------------------------------------------------


def plan_submodule(submodule: Submodule, repo: Path, token: str | None) -> BumpPlan:
    """Resolve and validate one submodule's proposed move."""
    old_commit = gitlink_commit(repo, submodule.path)
    tip, _label = fetch_upstream_tip(submodule, repo)
    new_commit = tip
    reachable = is_ancestor(submodule, repo, new_commit, "FETCH_HEAD")
    upstream = upstream_status(submodule, new_commit, token)
    return BumpPlan(
        submodule=submodule,
        old_commit=old_commit,
        new_commit=new_commit,
        reachable=reachable,
        branch_tip=tip,
        upstream=upstream,
    )


def build_plan(repo: Path, token: str | None, only: str | None = None) -> list[BumpPlan]:
    """The plan for every tracked submodule, or for one of them."""
    tracked = parse_gitmodules((repo / ".gitmodules").read_text(encoding="utf-8"))
    if only:
        tracked = [sub for sub in tracked if sub.path == only]
        if not tracked:
            raise SubmoduleError(f"{only} is not a tracked submodule")
    plans: list[BumpPlan] = []
    for submodule in tracked:
        try:
            plans.append(plan_submodule(submodule, repo, token))
        except SubmoduleError as error:
            print(f"::warning::{submodule.path}: {error}", file=sys.stderr)
    return plans


def apply_plan(plan: BumpPlan, repo: Path, token: str | None) -> str | None:
    """Move one gitlink, open its pull request, and return the branch name."""
    if not plan.changed:
        return None
    if plan.blocking:
        for reason in plan.blocking:
            print(f"::warning::{plan.submodule.path}: {reason}", file=sys.stderr)
        return None
    sub = plan.submodule
    branch = f"chore/update-submodule-{sub.name}-{os.environ.get('RUN_TIMESTAMP', 'manual')}"
    run_git(["submodule", "update", "--remote", "--", sub.path], cwd=repo)
    run_git(["checkout", "-b", branch], cwd=repo)
    run_git(["add", "--", sub.path], cwd=repo)
    run_git(["commit", "-m", render_commit_message(plan)], cwd=repo)
    run_git(["push", "origin", branch], cwd=repo)
    body = render_pr_body(plan)
    env = dict(os.environ)
    if token:
        env["GH_TOKEN"] = token
    result = subprocess.run(
        [
            "gh", "pr", "create",
            "--title", f"chore: update {sub.path} to {plan.new_commit[:12]}",
            "--body", body,
            "--base", "main",
            "--head", branch,
        ],
        cwd=str(repo),
        check=False,
        capture_output=True,
        text=True,
        env=env,
    )
    if result.returncode != 0:
        print(
            f"::warning::pushed {branch} but the workflow token cannot open pull "
            f"requests ({result.stderr.strip()}). Open it manually: "
            f"gh pr create --base main --head {branch}",
            file=sys.stderr,
        )
        return branch
    print(f"pull request opened for {branch}")
    return branch


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=str(REPO_ROOT), help="repository root")
    parser.add_argument("--submodule", default=None, help="restrict to one submodule path")
    parser.add_argument("--list-tracked", action="store_true", help="list the tracked submodules")
    parser.add_argument("--paths", action="store_true", help="print only the tracked paths")
    parser.add_argument("--plan", action="store_true", help="print the plan as JSON")
    parser.add_argument("--body", action="store_true", help="print the pull request body")
    parser.add_argument("--apply", action="store_true", help="open one pull request per submodule")
    args = parser.parse_args(argv)

    repo = Path(args.repo).resolve()
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if not args.submodule:
        # The workflow passes the optional manual input through the
        # environment, so a single command covers every tracked submodule.
        requested = (os.environ.get("SUBMODULE") or "").strip()
        args.submodule = requested or None

    if args.list_tracked:
        tracked = parse_gitmodules((repo / ".gitmodules").read_text(encoding="utf-8"))
        if args.paths:
            print("\n".join(sub.path for sub in tracked))
        else:
            for sub in tracked:
                print(f"{sub.path}\t{sub.url}\t{sub.branch_label}")
        return 0

    if args.plan:
        plans = build_plan(repo, token, args.submodule)
        print(json.dumps([_plan_as_dict(plan) for plan in plans], indent=2))
        return 0

    if args.body:
        for plan in build_plan(repo, token, args.submodule):
            print(render_pr_body(plan))
        return 0

    if args.apply:
        plans = build_plan(repo, token, args.submodule)
        for plan in plans:
            apply_plan(plan, repo, token)
        return 0

    parser.print_help()
    return 1


def _plan_as_dict(plan: BumpPlan) -> dict:
    return {
        "path": plan.submodule.path,
        "url": plan.submodule.url,
        "branch": plan.submodule.branch_label,
        "old_commit": plan.old_commit,
        "new_commit": plan.new_commit,
        "changed": plan.changed,
        "reachable": plan.reachable,
        "comparison_url": plan.comparison_url,
        "upstream": plan.upstream.as_dict(),
        "blocking": plan.blocking,
    }


if __name__ == "__main__":
    sys.exit(main())
