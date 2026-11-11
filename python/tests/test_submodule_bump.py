"""Tests for the per-submodule bump receipt (RuView#1743)."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "scripts" / "submodule_bump.py"

spec = importlib.util.spec_from_file_location("submodule_bump", MODULE_PATH)
assert spec and spec.loader
submodule_bump = importlib.util.module_from_spec(spec)
sys.modules["submodule_bump"] = submodule_bump
spec.loader.exec_module(submodule_bump)


GITMODULES = """
[submodule "vendor/midstream"]
\tpath = vendor/midstream
\turl = https://github.com/ruvnet/midstream
\tbranch = main
[submodule "vendor/rufield"]
\tpath = vendor/rufield
\turl = https://github.com/ruvnet/rufield
[submodule "v2/crates/worldgraph"]
\tpath = v2/crates/worldgraph
\turl = https://github.com/ruvnet/worldgraph.git
\tbranch = main
"""


def test_parse_gitmodules_reads_path_url_and_branch():
    tracked = submodule_bump.parse_gitmodules(GITMODULES)
    assert [sub.path for sub in tracked] == [
        "vendor/midstream",
        "vendor/rufield",
        "v2/crates/worldgraph",
    ]
    assert tracked[0].branch == "main"
    assert tracked[0].url == "https://github.com/ruvnet/midstream"


def test_a_submodule_without_a_branch_follows_the_default_head():
    tracked = submodule_bump.parse_gitmodules(GITMODULES)
    rufield = tracked[1]
    assert rufield.branch is None
    assert rufield.branch_label == submodule_bump.DEFAULT_BRANCH_LABEL


def test_parse_github_url_accepts_plain_https_ssh_and_dot_git():
    assert submodule_bump.parse_github_url("https://github.com/ruvnet/ruvector") == (
        "ruvnet",
        "ruvector",
    )
    assert submodule_bump.parse_github_url("https://github.com/ruvnet/ruvector.git") == (
        "ruvnet",
        "ruvector",
    )
    assert submodule_bump.parse_github_url("git@github.com:ruvnet/ruvector.git") == (
        "ruvnet",
        "ruvector",
    )


def test_comparison_url_points_at_the_upstream_range():
    url = submodule_bump.comparison_url(
        "https://github.com/ruvnet/ruvector", "aaaa1111", "bbbb2222"
    )
    assert url == "https://github.com/ruvnet/ruvector/compare/aaaa1111...bbbb2222"


def test_reachability_verdict_is_explicit_when_it_fails():
    assert submodule_bump.reachability_verdict(True) == "yes"
    assert "no" in submodule_bump.reachability_verdict(False)


def test_upstream_verdict_counts_check_runs():
    payload = {
        "check_runs": [
            {"status": "completed", "conclusion": "success"},
            {"status": "completed", "conclusion": "success"},
            {"status": "completed", "conclusion": "failure"},
            {"status": "in_progress", "conclusion": None},
            {"status": "completed", "conclusion": "skipped"},
        ]
    }
    verdict = submodule_bump.upstream_verdict_from_payloads(payload, None)
    assert verdict.total == 5
    assert verdict.passed == 2
    assert verdict.failed == 1
    assert verdict.pending == 1
    assert verdict.skipped == 1
    assert verdict.has_failures is True
    assert verdict.has_verdict is True


def test_upstream_verdict_treats_a_clean_run_as_clean():
    payload = {"check_runs": [{"status": "completed", "conclusion": "success"}]}
    verdict = submodule_bump.upstream_verdict_from_payloads(payload, None)
    assert verdict.has_failures is False
    assert verdict.has_verdict is True


def test_upstream_verdict_is_missing_when_the_upstream_reports_nothing():
    verdict = submodule_bump.upstream_verdict_from_payloads({}, None)
    assert verdict.missing is True
    assert verdict.has_verdict is False
    assert verdict.has_failures is False


def test_upstream_verdict_folds_the_combined_status():
    payload = {"check_runs": []}
    combined = {"state": "success"}
    verdict = submodule_bump.upstream_verdict_from_payloads(payload, combined)
    assert verdict.total == 1
    assert verdict.passed == 1
    assert verdict.has_failures is False


def test_a_pending_upstream_check_is_reported_but_does_not_block():
    payload = {"check_runs": [{"status": "in_progress", "conclusion": None}]}
    verdict = submodule_bump.upstream_verdict_from_payloads(payload, None)
    assert verdict.pending == 1
    assert verdict.has_failures is False
    plan = _plan(
        old_commit="aaaa1111",
        new_commit="bbbb2222",
        upstream=verdict,
    )
    assert plan.blocking == []


def _plan(**overrides):
    sub = submodule_bump.Submodule(
        path="vendor/ruvector",
        url="https://github.com/ruvnet/ruvector",
        branch="main",
    )
    defaults = {
        "submodule": sub,
        "old_commit": "aaaa1111",
        "new_commit": "bbbb2222",
        "reachable": True,
        "branch_tip": "bbbb2222",
        "upstream": submodule_bump.UpstreamVerdict(
            context="check runs", total=2, passed=2
        ),
    }
    defaults.update(overrides)
    return submodule_bump.BumpPlan(**defaults)


def test_the_receipt_records_the_old_and_new_commits_and_the_comparison():
    body = submodule_bump.render_pr_body(_plan())
    assert "aaaa1111" in body
    assert "bbbb2222" in body
    assert "https://github.com/ruvnet/ruvector/compare/aaaa1111...bbbb2222" in body
    assert "vendor/ruvector" in body


def test_the_receipt_names_the_required_ruview_checks():
    body = submodule_bump.render_pr_body(_plan())
    for check in submodule_bump.REQUIRED_RUVIEW_CHECKS:
        assert check in body


def test_the_receipt_flags_a_target_that_is_not_reachable():
    body = submodule_bump.render_pr_body(_plan(reachable=False))
    assert "not reachable" in body
    assert "not ready to merge" in body


def test_the_receipt_flags_an_upstream_without_any_verdict():
    body = submodule_bump.render_pr_body(
        _plan(upstream=submodule_bump.UpstreamVerdict(context="check runs", missing=True))
    )
    assert "no checks reported" in body


def test_a_clean_plan_states_the_merge_condition():
    body = submodule_bump.render_pr_body(_plan())
    assert "not ready to merge" not in body
    assert "must still pass on this pull request" in body


def test_a_plan_without_a_change_is_not_blocking():
    plan = _plan(old_commit="aaaa1111", new_commit="aaaa1111")
    assert plan.changed is False
    assert plan.blocking == []


def test_a_failing_upstream_check_blocks_the_move():
    plan = _plan(
        upstream=submodule_bump.UpstreamVerdict(
            context="check runs", total=3, passed=2, failed=1
        )
    )
    assert plan.blocking
    assert "failing check" in plan.blocking[0]


def test_the_commit_message_carries_both_commits_and_the_comparison():
    message = submodule_bump.render_commit_message(_plan())
    assert message.startswith("chore: update vendor/ruvector to bbbb2222")
    assert "Old commit: aaaa1111" in message
    assert "New commit: bbbb2222" in message
    assert "compare/aaaa1111...bbbb2222" in message


def test_the_real_gitmodules_file_parses():
    tracked = submodule_bump.parse_gitmodules(
        (REPO_ROOT / ".gitmodules").read_text(encoding="utf-8")
    )
    assert len(tracked) >= 9
    paths = {sub.path for sub in tracked}
    assert "vendor/ruvector" in paths
    assert "vendor/metaharness" in paths
    assert "v2/crates/worldgraph" in paths


@pytest.mark.parametrize("path", ["vendor/ruvector", "vendor/metaharness"])
def test_a_tracked_submodule_has_a_github_url(path):
    tracked = submodule_bump.parse_gitmodules(
        (REPO_ROOT / ".gitmodules").read_text(encoding="utf-8")
    )
    sub = next(sub for sub in tracked if sub.path == path)
    owner, repo = submodule_bump.parse_github_url(sub.url)
    assert owner == "ruvnet"
    assert repo
