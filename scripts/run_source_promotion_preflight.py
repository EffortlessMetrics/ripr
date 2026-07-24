#!/usr/bin/env python3
"""Run source-promotion preflight and persist a bounded failure receipt."""

from __future__ import annotations

import sys
import traceback

from source_promotion_preflight import OUTPUT_DIR, main, write_json


def run() -> int:
    try:
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
