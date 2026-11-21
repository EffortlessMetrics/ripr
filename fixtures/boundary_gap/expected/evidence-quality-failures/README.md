# Lane 1 Evidence Quality Failure Corpus

This corpus pins the first audit-driven Lane 1 evidence-quality failure modes.
It is not a user-facing report and is not a golden for the full
`repo-exposure-json` artifact. Each case records the audit signal plus the
expected `seams[].evidence_record` subset that future analyzer work must
preserve or intentionally update.

The cases are fixture-first evidence for `RIPR-SPEC-0032`:

- duplicate canonical gap overcount;
- missing equality-boundary discriminator;
- activation static limitation without concrete values;
- side-effect or mock observer semantics guard;
- calibration gap when runtime data has not been imported.

Run `cargo xtask check-fixture-contracts` to validate the corpus structure.
