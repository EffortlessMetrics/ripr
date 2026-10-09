# Seam inventory scaling benchmark

`cargo xtask seam-inventory-scaling-benchmark` generates deterministic
synthetic Rust workspaces at several file counts (default 200/800/2000
production modules, three small functions each, no tests directory) under
`target/` and times cold `ripr check` inventory runs
(`repo-seams-json` plus `repo-exposure-json`) at each size.

There is no committed input corpus: the generator is fixed-template, so a
run is reproducible from the recorded revision without storing thousands of
fixture files. Generated workspaces are removed after the run unless
`--keep-workspaces` is passed.

The receipt (`target/ripr/reports/seam-inventory-scaling-benchmark.{json,md}`)
records the per-size cost curve and the coarse ms-per-file slope. That curve
is the baseline the seam-inventory construction-cap issues (#1887, #4996, and #4997)
need: a cap-during-construction fix should bend the slope, not just
shift one point. Benchmark claims are limited to the recorded repository
revision and runner class.

Child runs pin `RIPR_REPO_EXPOSURE_SEAM_LIMIT=10000` (the product default),
and each sample records the child `run_status`, so a capped run cannot pass
silently as an uncapped baseline. Within each size, format samples interleave
in alternating order and each sample records its execution order. A sample
whose run exits successfully but reports a missing or zero inventory is
recorded as `empty_inventory`, which makes the report `inconclusive` instead
of a silent pass. (The cap check runs first: a capped sample with a missing
or zero inventory is recorded as `capped`, not `empty_inventory`. Both
statuses make the report `inconclusive`.)
