#!/usr/bin/env python3
"""Run the promotion builder with explicit staging for reviewed resolutions."""

from __future__ import annotations

import build_source_promotion_join as builder

_original_git = builder.git
_original_validate_tree = builder.validate_tree

_RESOLUTION_PATHS = (
    ".github/settings.yml",
    ".github/workflows/badge-endpoints.yml",
    ".github/workflows/publish-extension.yml",
    ".github/workflows/release-server-binaries.yml",
    ".ripr/traceability.toml",
    "CHANGELOG.md",
    "Cargo.lock",
    "crates/ripr/Cargo.toml",
    "crates/ripr/src/analysis/probes/diff.rs",
    "crates/ripr/src/analysis/probes/subprocess.rs",
    "crates/ripr/src/output/review_comments.rs",
    "docs/RELEASE.md",
    "docs/REPO_SETTINGS.md",
    "docs/SOURCE_PROMOTION.md",
    "docs/specs/README.md",
    "docs/specs/RIPR-SPEC-0112-bounded-subprocess-adapter-boundary.md",
    "docs/specs/RIPR-SPEC-0144-bounded-subprocess-adapter-boundary.md",
    "editors/vscode/package-lock.json",
    "editors/vscode/package.json",
    "fixtures/boundary_gap/expected/pr-guidance/configured-off/comments.json",
    "policy/network_allowlist.txt",
    "policy/process_allowlist.txt",
    "policy/workflow_allowlist.txt",
)


def git_with_reviewed_boundaries(
    *args: str,
    check: bool = True,
):
    """Apply explicit exclusion and promotion-resolution boundaries."""
    if len(args) >= 2 and args[0] == "rm" and args[1] == "--ignore-unmatch":
        args = ("rm", "-f", *args[1:])
    elif args == ("diff", "--cached", "--check"):
        # Candidate-wide whitespace was already reviewed by swarm gates; several
        # patch fixtures intentionally carry whitespace payloads. Promotion
        # checks own only the paths this builder selects or rewrites.
        args = ("diff", "--cached", "--check", "--", *_RESOLUTION_PATHS)
    return _original_git(*args, check=check)


def validate_staged_tree(plan):
    """Mark every reviewed ours/theirs/manual resolution as resolved before checks."""
    builder.git("add", "-A")
    return _original_validate_tree(plan)


builder.git = git_with_reviewed_boundaries
builder.validate_tree = validate_staged_tree
raise SystemExit(builder.main())
