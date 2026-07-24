#!/usr/bin/env python3
"""Run the promotion builder with explicit staging for reviewed resolutions."""

from __future__ import annotations

import build_source_promotion_join as builder

_original_git = builder.git
_original_validate_tree = builder.validate_tree


def git_with_forced_exclusions(
    *args: str,
    check: bool = True,
):
    """Allow reviewed excluded paths to be removed after Git stages their addition."""
    if len(args) >= 2 and args[0] == "rm" and args[1] == "--ignore-unmatch":
        args = ("rm", "-f", *args[1:])
    return _original_git(*args, check=check)


def validate_staged_tree(plan):
    """Mark every reviewed ours/theirs/manual resolution as resolved before checks."""
    builder.git("add", "-A")
    return _original_validate_tree(plan)


builder.git = git_with_forced_exclusions
builder.validate_tree = validate_staged_tree
raise SystemExit(builder.main())
