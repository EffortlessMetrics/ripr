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

Corpus cases that carry a `record` and are not marked `invalid` are validated
against `schemas/ripr/repair-assurance.schema.json` by
`cargo xtask check-verification-contracts --check`, which registers
`/cases/*/record` as a contract subject.

Two kinds of case are deliberately outside that walk, and neither is silently
dropped. A case exposing only `record_patch` — `malformed_command_spec` — has no
`/record` to select; its negative proof is
`assurance_corpus_rejects_an_absolute_command_working_directory`. A case marked
`invalid` is an advertised negative rather than a positive subject; counting it
as passing would claim coverage the walk does not perform, so
`assurance_corpus_invalid_cases_declare_their_authority` requires each one to
name either a schema-rejectable patch or the narrower authority that rejects
it.
The typed `VerificationExecutionResultV1` validator covers the command-spec,
root, HEAD, disposition, and commitment boundaries; this PR does not execute
any command from the corpus.

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
