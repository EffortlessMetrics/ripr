#!/usr/bin/env python3
"""Run the promotion builder with explicit force-removal for staged exclusions."""

from __future__ import annotations

import build_source_promotion_join as builder

_original_git = builder.git


def git_with_forced_exclusions(
    *args: str,
    check: bool = True,
):
    """Allow reviewed excluded paths to be removed after Git stages their addition."""
    if len(args) >= 2 and args[0] == "rm" and args[1] == "--ignore-unmatch":
        args = ("rm", "-f", *args[1:])
    return _original_git(*args, check=check)


builder.git = git_with_forced_exclusions
raise SystemExit(builder.main())
