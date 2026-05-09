# Evidence Record Contract Corpus

This fixture corpus pins representative `evidence_record` JSON records for
RIPR-SPEC-0021.

The corpus is intentionally schema-focused. It does not rerun analyzer logic,
generate tests, call providers, execute mutation testing, change gate policy, or
edit source. `cargo xtask check-fixture-contracts` validates the required case
matrix and field shape, while `cargo xtask check-output-contracts` validates the
`schema_version` lock.

Current v0.1 calibration entries use the placeholder:

```json
{
  "availability": "not_imported",
  "confidence": "unknown",
  "agreement": "no_runtime_data"
}
```

Runtime-supported confidence labels are intentionally left for the later
static/runtime confidence-label slice.
