# Fixture: assurance_vocabulary

Spec: RIPR-SPEC-0135

## Given

The analyzer receives a format-only diff and therefore has no changed
behavior movement to report. The adjacent `assurance/corpus.json` contains
design-only `RepairAssuranceV1` examples for static movement, explicit command
execution, receipt issuance, and external runtime evidence.

## When

```bash
cargo xtask fixtures assurance_vocabulary
```

The assurance corpus is inspected against
`schemas/ripr/repair-assurance.schema.json` by the future verification/receipt
slice. This PR does not execute any command from the corpus.

## Then

The normal fixture remains a zero-finding control, while the corpus keeps
static movement, command state, receipt state, and runtime mutation boundary
as separate fields. Static improvement with `verification_not_run` remains
valid but visibly static-only.

## Must Not

- Treat static movement as executed test verification.
- Infer a passing command from `verify_command`, `commands_run`, or receipt presence.
- Treat a successful command as proof that a static gap closed.
- Treat externally supplied mutation evidence as RIPR-produced runtime mutation outcome data.
- Allow malformed, fabricated, stale, wrong-root, or timed-out examples to become a passing state.
