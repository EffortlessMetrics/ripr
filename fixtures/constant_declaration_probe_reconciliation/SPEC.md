# Fixture: constant_declaration_probe_reconciliation

Spec: RIPR-SPEC-0046

Issue: #3719

## Given

The exact #3719 shape: `pub(crate) const OBSERVATION_SCHEMA_GENERATION: u32 = 3;`
at the top of a lib crate, observed by a test that reads the constant
(`assert_eq!(OBSERVATION_SCHEMA_GENERATION, 3)`). Controls in the same input:
an actual deletable call (`dispatch_event` -> `log_line`), an actual field
construction (`CoverageRow { total, skipped }`), and a call-initializer
constant form covered by the unit tests in
`crates/ripr/src/analysis/probes/lexical.rs`. Before the fix, the declaration's
`pub(crate)` parentheses and `: Type` colon made `classify_changed_line` emit
both `call_deletion` and `field_construction` probes for the constant line.

The diff touches only the constant line (`= 4` -> `= 3`, the #3719 value);
blank context lines carry no leading space (`git diff --check` hygiene,
matching the error_variant fixture family).

## When

```bash
cargo xtask fixtures constant_declaration_probe_reconciliation
```

or:

```bash
ripr check --root fixtures/constant_declaration_probe_reconciliation/input \
           --diff fixtures/constant_declaration_probe_reconciliation/diff.patch \
           --mode fast
```

## Then

The constant line yields exactly one probe with family `static_unknown` — no
`call_deletion`, no `field_construction` — and classifies `static_unknown`
with no exposure (the observing test does not create reach for a
StaticUnknown probe; unknown flow is recorded as such).

The check output's `finding_alignment` section carries one canonical item:
`canonical_gap_id: config_or_policy_constant::OBSERVATION_SCHEMA_GENERATION`,
`canonical_item_kind: limitation`, `evidence_class: config_or_policy_constant`,
`gap_state: static_limitation`, `actionability: inspect_config_flow`, with the
`config_policy_flow_unknown` limitation category ("Changed config or policy
constant could not be traced to or away from a supported output, schema,
validation, or behavior sink"). Local-path note (documented honestly): the
finding's own `static_limit_kind` field stays `None` in this path — the
typed limitation rides on the canonical alignment item (`gap_state:
static_limitation`), not on the finding field; the wrapper-seam limiter from
Issue #3700 is a different, class-gated route.


## Initializer coverage

The input also carries initializer-variant constants so the
declaration gate is exercised across initializer shapes: a
call-initializer (`const LIMIT: usize = compute_limit(64);`), a
`static` binding, and the diffed numeric literal. Those plain shapes
classify `static_unknown` with no call/field/return/effect families.
Behavioral families are not banned outright: they come from code spans
of the initializer only (a threshold comparison still reads
`Predicate`), while string/comment contents never mint families
(`" > "` stays `static_unknown` alone). Declaration syntax never reads
as `call_deletion`/`field_construction`, whatever the initializer.

## Must Not

- Emit `call_deletion` or `field_construction` probes for a constant
  declaration, whatever its visibility or initializer.
- Classify the fixture's plain-literal constant lines as anything other than
  `static_unknown` in fast mode (behavioral code-span initializers keep
  their families alongside `static_unknown` per the contract above), or
  claim exposure/reach from the observing constant-read test.
- Prescribe a deleted-call or field-construction repair for the declaration.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
