# Evidence Record Contract Corpus

This fixture corpus pins representative `evidence_record` JSON records for
RIPR-SPEC-0021.

The corpus is intentionally schema-focused. It does not rerun analyzer logic,
generate tests, call providers, execute mutation testing, change gate policy, or
edit source. `cargo xtask check-fixture-contracts` validates the required case
matrix and field shape, while `cargo xtask check-output-contracts` validates the
`schema_version` lock.

Related-test entries include `oracle_semantics` so the contract pins what the
oracle observes, what discriminator remains missing, and which assertion upgrade
is available when RIPR can name one.

Actionable canonical items include a structured `repair_route` with
`repair_kind`, `target_test_type`, and `suggested_assertion`. The
predicate-boundary case pins the fixture-backed `add_boundary_assertion` route
so Lane 1 can count it as actionable work rather than prose-only guidance.

Current v0.1 calibration entries use the placeholder:

```json
{
  "availability": "not_imported",
  "confidence": "unknown",
  "agreement": "no_runtime_data"
}
```

Imported static/runtime confidence labels are emitted by the separate mutation
calibration reports. They do not enter the `evidence_record` v0.1 contract
until a future schema revision explicitly adds runtime-backed calibration
context to the shared record.
