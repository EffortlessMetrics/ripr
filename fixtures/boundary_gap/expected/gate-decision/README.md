# Calibrated Gate Decision Fixtures

These fixtures pin `ripr gate evaluate` output for the boundary-gap corpus.
They are static policy fixtures: they reuse checked PR guidance and imported
calibration-shaped inputs without running mutation tools, posting comments, or
changing generated workflow defaults.

Cases:

- `advisory`: visible-only mode records the gap without blocking.
- `acknowledged`: acknowledgeable mode keeps a waived gap visible.
- `baseline-check`: an explicit baseline keeps an existing gap advisory.
- `fail-on-new-high-confidence-gap`: calibrated mode blocks a new supported
  gap.
- `suppression`: suppressed/configured-off candidates remain visible and
  non-blocking.
- `missing-input`: a baseline-required mode reports a deterministic config
  error when the baseline is omitted.
- `calibration-agrees`: imported mutation calibration supports the static gap.
- `calibration-disagrees`: calibration evidence keeps the candidate advisory.

