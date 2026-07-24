#!/usr/bin/env python3
"""Run source-promotion preflight and persist a bounded failure receipt."""

from __future__ import annotations

import subprocess
import sys
import traceback

from source_promotion_preflight import OUTPUT_DIR, ROOT, main, write_json


def configure_disposable_identity() -> None:
    """Configure the local identity Git requires to prepare a no-commit merge."""
    for key, value in (
        ("user.name", "RIPR Source Promotion Preflight"),
        ("user.email", "source-promotion-preflight@example.invalid"),
    ):
        subprocess.run(
            ["git", "config", "--local", key, value],
            cwd=ROOT,
            check=True,
            text=True,
        )


def run() -> int:
    try:
        configure_disposable_identity()
        return main()
    except Exception as error:  # noqa: BLE001 - boundary must retain every failure
        OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
        failure = {
            "schema_version": "1.0",
            "kind": "ripr_source_promotion_preflight_failure",
            "status": "failed",
            "error_type": type(error).__name__,
            "message": str(error),
            "traceback": traceback.format_exc().splitlines()[-20:],
            "authority_boundary": (
                "This failure receipt describes preflight execution only. It does not "
                "authorize a merge, version change, tag, publication, signing, "
                "marketplace mutation, release-secret use, or support-tier promotion."
            ),
        }
        write_json(OUTPUT_DIR / "failure.json", failure)
        print(f"source-promotion preflight failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(run())
