# Published Schema Producer Audit

This is the reviewed inventory of every JSON Schema published under `schemas/`,
the producer that emits the bytes each schema claims to describe, and the
authority that proves the two still agree.

A schema file existing, parsing, or declaring a pinned `schema_version` is not
proof that current producer output validates against it. Only a registered
contract in `xtask/src/verification_contracts.rs` — schema, canonical subject,
and a negative mutation that must fail — establishes that.

Run the registered contracts with:

```bash
cargo xtask check-verification-contracts --check
cargo test -p xtask verification_contracts
```

## Producer state vocabulary

```text
live         a currently reachable public producer emits these bytes
reserved     the shape is versioned and design-only; no producer emits it
unreachable  no producer and no reserving owner
```

A `reserved` producer is never treated as validated. It carries an explicit
exemption naming the narrower authority that replaces fixture validation, and
routes the missing producer proof to its owning issue.

## Audit table

| schema path | schema_version | producer authority | producer state | canonical fixture | verification command | negative mutation | current status | exemption / reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `schemas/badges/shields-endpoint.schema.json` | `1` | `cargo xtask badges` → `crates/ripr/src/output/badge` | live | `tests/fixtures/verification/badge/ripr-plus.valid.json` | `cargo xtask check-verification-contracts --check` | `invalid_type_reports_actionable_path` (numeric `message`) | registered | — |
| `schemas/ripr/pr-evidence.schema.json` | `0.1` | `cargo xtask ripr-pr` → `crates/ripr/src/output/pr_evidence_ledger.rs` | live | `tests/fixtures/verification/ripr/pr-evidence.valid.json` | `cargo xtask check-verification-contracts --check` | `valid_pr_evidence_fixture_matches_schema` guards the positive; drift is caught by `additionalProperties: false` | registered | — |
| `schemas/ripr/review-comments.schema.json` | `0.1` | `ripr review-comments --format json` → `crates/ripr/src/output/review_comments` | live | `tests/fixtures/verification/ripr/review-comments.valid.json` | `cargo xtask check-verification-contracts --check`; also validated at generation time by `xtask/src/reports/review_comments.rs` | `assertion_guidance_schema_rejects_invalid_state_shapes` | registered | — |
| `schemas/ripr/gate-decision.schema.json` | `0.1` | `ripr gate evaluate --format json` → `crates/ripr/src/output/gate` | live | `tests/fixtures/verification/ripr/gate-decision.valid.json` | `cargo xtask check-verification-contracts --check` | covered by the shared unknown-field and `const` rejections | registered | — |
| `schemas/ripr/check.schema.json` | `0.2` | `ripr check --format json` → `crates/ripr/src/output/json` | live | `tests/fixtures/verification/ripr/check-complete.valid.json`, `…/check-limited.valid.json` | `cargo xtask check-verification-contracts --check` | `check_schema_rejects_unknown_top_level_field`, `check_schema_rejects_negative_fractional_confidence` | registered | — |
| `schemas/ripr/rust-repair-trust-corpus.schema.json` | `0.1` | `metrics/rust-repair-trust/corpus.json`, the corpus of record read by `xtask/src/reports/rust_repair_trust.rs` | live | `metrics/rust-repair-trust/corpus.json` (the artifact itself, not a copy) | `cargo xtask check-verification-contracts --check` | `rust_repair_trust_corpus_rejects_a_non_sha_analyzed_head` | registered (attempt subschema unexercised) | The corpus `cases` array is empty today (24 exclusions, 12 observations, 0 cases), so the per-attempt subschema is registered but consumes no subject. The negative mutation exercises `exclusions[0].analyzed_head_sha`, not an attempt. Attempt-level validation becomes real evidence only once a case exists. |
| `schemas/ripr/repair-assurance.schema.json` `#/$defs/command_spec` | `1` | `crates/ripr/src/agent/command_specs.rs` → `command_specs` in `ripr agent packet --json` | live | `fixtures/boundary_gap/expected/editor-agent-loop/agent-packet.json` `/packets/0/evidence_record/canonical_item/command_specs/receipt` | `cargo xtask check-verification-contracts --check` | `command_spec_contract_rejects_an_absolute_working_directory` | registered | — |
| `schemas/ripr/repair-assurance.schema.json` `#/$defs/verification_command_spec` | `1` | same producer, verify route | live | same golden, `…/command_specs/verify` | `cargo xtask check-verification-contracts --check` | `verification_command_spec_contract_rejects_a_receipt_route` | registered | — |
| `schemas/ripr/repair-assurance.schema.json` `#/$defs/execution_result` | `1` | `ripr agent verify-execute` → `crates/ripr/src/app/verification_execution.rs`, `ripr::domain::VerificationExecutionResultV1` | live | none committed — the producer only emits bytes from a real bounded execution | `cargo test -p ripr --test verification_result`; `cargo test -p ripr --test cli_smoke agent_verify_execute` | `serialized_result_conforms_to_the_published_schema` fails on a serde-defaulted field becoming schema-required | **exempt (partial)** | No committed producer-byte artifact exists. The narrower authorities are the serde↔schema field/required parity test and `VerificationExecutionResultV1::validate_against`, which enforce root, HEAD, digest, and disposition bindings the schema cannot express. Capturing a real `verify-execute` result as a committed fixture is deferred. |
| `schemas/ripr/repair-assurance.schema.json` (envelope) | `1` | none — `implementation_state` is pinned to `const "design_only"` | reserved | `fixtures/assurance_vocabulary/assurance/corpus.json` `/cases/*/record` (design corpus, **not** producer bytes) | `cargo xtask check-verification-contracts --check` | `assurance_corpus_rejects_an_absolute_command_working_directory` | registered as design corpus | No producer emits a whole `RepairAssuranceV1` record. `RIPR-SPEC-0135` and ADR 0021 reserve the envelope for a future slice that joins the static-movement, verification, receipt, and runtime-mutation axes. The corpus makes the vocabulary claim enforceable; it does not make the envelope producer-backed. |
| `schemas/ripr/ripr-agent-capability.schema.json` | `0.1` | `crates/ripr/src/lsp/agent_protocol.rs` — only `ripr/listActionableItems` is implemented | reserved | none | none | none | **exempt — out of scope** | Routed to #3009 (`0.13`) after #1599/#1602/#1603 establish live handler authority. Freezing the shape now would ratify a protocol that has no handler. |
| `schemas/ripr/ripr-agent-request.schema.json` | `0.1` | same | reserved | none | none | none | **exempt — out of scope** | #3009. |
| `schemas/ripr/ripr-agent-success.schema.json` | `0.1` | same | reserved | none | none | none | **exempt — out of scope** | #3009. |
| `schemas/ripr/ripr-agent-error.schema.json` | `0.1` | same | reserved | none | none | none | **exempt — out of scope** | #3009. |

## What the registered contracts enforce

`xtask/src/verification_contracts.rs` is the single owning validator. Every
registered contract selects a subject — a whole document, one JSON pointer, or
every member of an array — and validates it against the schema or a named
subschema. A contract that resolves no subject fails, so a stale pointer cannot
be reported as a passing check.

The validator evaluates `$ref`, `allOf`, `anyOf`, `oneOf`, `if`/`then`/`else`,
`not`, `const`, `enum`, `type`, `required`, `additionalProperties`, `items`,
`minItems`, `maxItems`, `uniqueItems`, `minLength`, `maxLength`, `minimum`,
`maximum`, and `pattern`.

`pattern` support is fail-closed: `xtask/src/schema_pattern.rs` implements the
regular-expression subset the repository's own schemas use and reports any
other construct as a violation rather than assuming a match. Without it the
identity commitments — 40-character head SHAs, `sha256:` digests, and the
relative `working_directory` constraint — would be declared by the schema and
enforced by nothing.

## Command spec and trust corpus fields

The registered command-spec subjects pin the producer-owned route description:
`command_spec` requires `authority_boundary` and a relative `working_directory`,
and `verification_command_spec` additionally pins the verify role and the
verification-only authority boundary.

The trust corpus contract pins the corpus envelope — `schema_version`, `kind`,
`authorization`, `cases`, `exclusions`, and `observations` — together with the
per-attempt provenance fields, including the 40-hex `analyzed_head_sha` that
makes an attempt attributable to an exact revision.

The design-only assurance envelope keeps `implementation_state`,
`static_movement`, `verification`, `receipt_state`, `runtime_mutation`, and
`non_claims` as independent axes, so a static comparison can never be read as
an executed test or a runtime mutation outcome.

## Claim boundary

This audit proves that each currently reachable non-`riprAgent` published
schema is either bound to producer or corpus-of-record bytes with a
discriminating negative mutation, or carries an explicit exemption naming the
narrower authority that replaces fixture validation. It does not prove the
semantic correctness of the analysis behind those bytes, and it establishes
nothing about the reserved `riprAgent` protocol.
