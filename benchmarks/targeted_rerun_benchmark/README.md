# Targeted rerun benchmark fixture

This controlled Rust workspace is used only by
`cargo xtask targeted-rerun-benchmark`. It contains one selected test owner and
many unrelated production seams so the cold full inventory and warm targeted
path exercise measurably different amounts of evidence work.

The fixture is not a product-quality or coverage corpus. Benchmark claims are
limited to the recorded repository revision and runner class.
