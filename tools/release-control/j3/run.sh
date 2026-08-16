#!/usr/bin/env bash
set -euo pipefail

root=$(git rev-parse --show-toplevel)
patched="$RUNNER_TEMP/ripr-j3-build-reviewed.sh"
python "$root/tools/release-control/j3/patch_build_v2.py" \
  "$root/tools/release-control/j3/build.sh" \
  "$patched"
bash -n "$patched"
bash "$patched"
