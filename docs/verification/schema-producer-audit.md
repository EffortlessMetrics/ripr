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
| `schemas/ripr/pr-evidence.schema.json` | `0.1` | `cargo xtask ripr-pr` → `xtask/src/reports/pr_evidence.rs` (compatibility producer); `ripr pr-evidence` → `crates/ripr/src/app/pr_evidence.rs` | live | `tests/fixtures/verification/ripr/pr-evidence.valid.json` | `cargo xtask check-verification-contracts --check` | `valid_pr_evidence_fixture_matches_schema` guards the positive; drift is caught by `additionalProperties: false` | registered | — |
| `schemas/ripr/review-comments.schema.json` | `0.1` | `ripr review-comments --format json` → `crates/ripr/src/output/review_comments` | live | `tests/fixtures/verification/ripr/review-comments.valid.json` | `cargo xtask check-verification-contracts --check`; also validated at generation time by `xtask/src/reports/review_comments.rs` | `assertion_guidance_schema_rejects_invalid_state_shapes` | registered | — |
| `schemas/ripr/gate-decision.schema.json` | `0.1` | `ripr gate evaluate --format json` → `crates/ripr/src/output/gate` | live | `tests/fixtures/verification/ripr/gate-decision.valid.json` | `cargo xtask check-verification-contracts --check` | covered by the shared unknown-field and `const` rejections | registered | — |
| `schemas/ripr/check.schema.json` | `0.2` | `ripr check --format json` → `crates/ripr/src/output/json` | live | `tests/fixtures/verification/ripr/check-complete.valid.json`, `…/check-limited.valid.json` | `cargo xtask check-verification-contracts --check` | `check_schema_rejects_unknown_top_level_field`, `check_schema_rejects_negative_fractional_confidence` | registered | — |
| `schemas/ripr/rust-repair-trust-corpus.schema.json` | `0.1` | `metrics/rust-repair-trust/corpus.json`, the corpus of record read by `xtask/src/reports/rust_repair_trust.rs` | live | `metrics/rust-repair-trust/corpus.json` (the artifact itself, not a copy) | `cargo xtask check-verification-contracts --check` | `rust_repair_trust_corpus_rejects_a_non_sha_analyzed_head` | registered (attempt subschema unexercised) | The corpus `cases` array is empty today (24 exclusions, 15 observations, 0 cases), so the per-attempt subschema is registered but consumes no subject. The negative mutation exercises `exclusions[0].analyzed_head_sha`, not an attempt. Attempt-level validation becomes real evidence only once a case exists. |
| `schemas/ripr/repair-assurance.schema.json` `#/$defs/command_spec` | `1` | `crates/ripr/src/agent/command_specs.rs` → `command_specs` in `ripr agent packet --json` | live | `fixtures/boundary_gap/expected/editor-agent-loop/agent-packet.json` `/packets/0/evidence_record/canonical_item/command_specs/receipt` | `cargo xtask check-verification-contracts --check` | `command_spec_contract_rejects_an_absolute_working_directory` | registered | — |
| `schemas/ripr/repair-assurance.schema.json` `#/$defs/verification_command_spec` | `1` | same producer, verify route | live | same golden, `…/command_specs/verify` | `cargo xtask check-verification-contracts --check` | `verification_command_spec_contract_rejects_a_receipt_route` | registered | — |
| `schemas/ripr/repair-assurance.schema.json` `#/$defs/execution_result` | `1` | `ripr agent verify-execute` → `crates/ripr/src/app/verification_execution.rs`, `ripr::domain::VerificationExecutionResultV1` | live | none committed — the producer only emits bytes from a real bounded execution | `cargo test -p ripr --test verification_result`; `cargo test -p ripr --test cli_smoke agent_verify_execute` | `serialized_result_conforms_to_the_published_schema` fails on a serde-defaulted field becoming schema-required | **exempt (partial)** | No committed producer-byte artifact exists. The narrower authorities are the serde↔schema field/required parity test and `VerificationExecutionResultV1::validate_against`, which enforce root, HEAD, digest, and disposition bindings the schema cannot express. Capturing a real `verify-execute` result as a committed fixture is deferred. |
| `schemas/ripr/repair-assurance.schema.json` (envelope) | `1` | none — `implementation_state` is pinned to `const "design_only"` | reserved | `fixtures/assurance_vocabulary/assurance/corpus.json` `/cases/*/record` (design corpus, **not** producer bytes) | `cargo xtask check-verification-contracts --check` | `assurance_corpus_rejects_an_absolute_command_working_directory` | registered as design corpus | No producer emits a whole `RepairAssuranceV1` record. `RIPR-SPEC-0135` and ADR 0021 reserve the envelope for a future slice that joins the static-movement, verification, receipt, and runtime-mutation axes. The corpus makes the vocabulary claim enforceable; it does not make the envelope producer-backed. |
| `schemas/ripr/repair-attempt.schema.json` | `0.1` | `ripr agent repair --phase before` and `--phase after` → `crates/ripr/src/app/repair_attempt.rs` (`write_repair_attempt_manifest`) | live | none committed — the manifest is a per-attempt runtime artifact at `target/ripr/repair-attempts/<attempt-id>/attempt.json` | `cargo test -p ripr --test python_repair_attempt` | `published_schema_conformance_rejects_manifests_the_schema_forbids`, and a schema requirement the producer does not emit fails `clean_test_only_edit_binds_prepare_and_apply` | **exempt (partial)** | No committed producer-byte artifact exists, so the xtask contract table has nothing to register. The narrower authority is `assert_manifest_matches_published_schema` in `crates/ripr/tests/python_repair_attempt.rs`, which checks bytes the real `before` and `after` phases wrote — both branches of the state-to-`after` conditional — against the published closed property set, required fields, and pinned `const` and `enum` values. It does not evaluate the schema's `pattern` constraints. Those are enforced instead by `validate_manifest` in the same module, which every read and write of a manifest passes through: it pins the `schema_version` and `kind` constants, runs `RepairAttemptId::parse` for the `repair-attempt-` plus 24-hex identity, requires a 40-character hexadecimal `repository_head`, runs `is_sha256_digest` over every artifact digest, and rejects the same state-to-`after` boundary the schema declares. The `after` block's own `attempt_id`, `repository_head`, `delta_sha256`, and `packet_sha256` are **not** covered by that validator; those shapes hold by construction only. `repair_attempt_schema_carries_terminal_after_contract` asserts what the schema declares, not that a producer matches it, so it is not a producer binding on its own. |
| `schemas/ripr/ripr-agent-capability.schema.json` | `0.2` | `crates/ripr/src/lsp/agent_protocol.rs` — only `ripr/listActionableItems` is implemented | live (capability only) | `fixtures/lsp_agent_protocol/capability.json` | `capability_output_matches_the_published_schema` + `capability_advertises_only_implemented_handlers` in `crates/ripr/src/lsp/agent_protocol.rs` (structural producer↔schema parity) | none registered (structural parity only) | **exempt — narrower authority named** | The live `server_capability()` projection and the committed fixture are pinned against the schema by the structural parity tests above, which fail on any producer↔schema drift. Producer-state vocabulary: the capability producer is live; only the envelope schemas below stay reserved. |
| `schemas/ripr/ripr-agent-request.schema.json` | `0.2` | same | reserved | none | none | none | **exempt — out of scope** | #3009. |
| `schemas/ripr/ripr-agent-success.schema.json` | `0.2` | none — the live `ripr/listActionableItems` handler (`crates/ripr/src/lsp/backend.rs`) emits an interim response shape, NOT this full envelope | reserved | none for this schema; the interim shape is pinned by `list_actionable_items_interim_response_shape_is_closed` in `crates/ripr/src/lsp/backend.rs` and documented in `docs/OUTPUT_SCHEMA.md` | none | none | **exempt — out of scope (narrower authorities named)** | No live producer emits the full envelope, so a registered contract would resolve no subject; registration stays routed to #3009. The narrower authorities covering this schema today: the route-coherence conditionals are pinned by the resolver tests in `crates/ripr/src/lsp/agent_protocol.rs` (`readiness_must_agree_with_route_and_spec`, `schema_0_2_payload_omitting_route_field_is_rejected`, `command_spec_decoding_is_closed_to_the_published_shape`), and the envelope example in `fixtures/lsp_agent_protocol/success-envelope.json` tracks the producer-shaped contract. The live handler's interim response shape is separately pinned by the closed-shape test and the `docs/OUTPUT_SCHEMA.md` record. |
| `schemas/ripr/ripr-agent-error.schema.json` | `0.2` | none for this envelope — the live handler's fail-closed error payloads are a three-field `error` object, NOT this full envelope | reserved | none | none | none | **exempt — out of scope** | #3009. The live fail-closed error shape (`{ "error": { "kind", "message", "recovery_route" } }`) is pinned by `list_actionable_items_interim_response_shape_is_closed` in `crates/ripr/src/lsp/backend.rs` and documented in `docs/OUTPUT_SCHEMA.md`. |
| `schemas/ripr/executed-control.schema.json` | `1` | `crates/ripr/src/domain/executed_control.rs` validates; `crates/ripr/src/output/executed_control.rs` projects JSON/Markdown | live (library contract) | `fixtures/executed-control-contract/expected/valid-pass-after-repair.json` | `cargo xtask check-verification-contracts --check`; `cargo test -p ripr --lib domain::executed_control output::executed_control` | domain negatives: ordinary positive test, review prose, other head, unexercised subject, duplicate ids, unknown obligation | registered | Corpus negatives are advertised `invalid` and skipped by the schema contract; the domain validator is the satisfaction authority. #3858/#4063 is a `not_proven` documentation fixture, not pass. |

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

## Release producer-corpus sweep

The registered contracts validate one chosen subject per row. The release sweep
validates every committed producer document instead, so a qualification run does
not depend on which subjects someone remembered to register (#3919, #3920):

```bash
cargo xtask schema-producer-sweep [--rev REV] \
  [--artifact SCHEMA[#POINTER]=FILE[#POINTER]]...
```

Every schema and document it validates is read from one commit (`--rev`,
default `HEAD`) through `git archive`, never from the working tree, so
uncommitted edits and untracked files such as live artifacts cannot change the
result. It uses the same validator as the registered contracts and writes
`target/ripr/reports/schema-producer-sweep.json`: per schema row, the schema
digest, discovered and validated producer counts, edge fixtures, excluded
stimulus, live artifacts, every subject's byte digest, the commit, and a `rows_sha256` digest over all
rows. The binding table in `xtask/src/schema_producer_sweep.rs` is the reviewed
rule for which bytes count:

- **Corpus rows** bind by file name or path (`check.json`, `gate-decision.json`,
  the committed badge endpoints, the trust corpus) or, for the command-spec
  subschemas, every `command_specs` entry in any committed document. Binding by
  name rather than shape means a producer that drops a required field fails its
  own golden instead of falling out of the sweep. A row that binds nothing fails.
- **Excluded stimulus** is a path prefix with a stated reason, such as the
  evidence-promotion `source_report` inputs. A document outside every binding
  that still carries a schema's `const` discriminators and required fields fails
  as an unbound lookalike until it is bound or excluded.
- **Runtime-only rows** (`pr-evidence`, `review-comments`, `repair-attempt`,
  the `execution_result` subschema) have no committed producer bytes. They report
  `not_run` unless live artifacts are passed with `--artifact`.
- **Exempt rows** are the reserved envelopes and the capability schema, whose
  narrower authorities are named in the audit table above.

Hand-written edge fixtures are counted apart from producer bytes, so they can
never be a row's only subjects.

The sweep is release-only. It is not a required check; replacing the required
gate's subject selection is #3919, deferred past the 0.11 cut. Unit tests pin
the sweep's own failure modes and that its binding table names exactly the
published inventory.

To feed the runtime-only rows on a candidate, run the producers in a scratch Git
repository with one committed behavior change (for example
`fixtures/boundary_gap/input` with `diff.patch` applied as the head commit) and
pass the outputs:

```bash
ripr pr-evidence --base "$BASE" --head "$HEAD"
ripr review-comments --base "$BASE" --head "$HEAD" --out rc.json
cargo xtask schema-producer-sweep \
  --artifact schemas/ripr/pr-evidence.schema.json=target/ripr/pr/repo-exposure.json \
  --artifact schemas/ripr/review-comments.schema.json=rc.json
```

`repair-attempt` manifests come from `ripr agent repair --phase before|after`
(`target/ripr/repair-attempts/<id>/attempt.json`), and `execution_result` from
`ripr agent verify-execute --result-json FILE`, passed as
`schemas/ripr/repair-assurance.schema.json#/$defs/execution_result=FILE#/result`.

The `review-comments --gap-ledger` route (#3913) does not need a live artifact
on the sweep. Two `cargo test -p xtask` tests in `xtask/src/reports/review_comments.rs`
run `ripr reports gap-ledger` and `ripr review-comments --gap-ledger` in process
on every tree and validate the bytes they write against the published schema:
`gap_ledger_route_bytes_from_check_goldens_match_published_schema` covers the
`--check-output` ledgers from the Rust and Python `boundary_gap` check goldens,
and `eligible_gap_ledger_route_cards_match_published_schema` covers inline,
summary-only and suppressed gap-record cards, with mutations the schema must
reject.

## The inventory this table covers

`cargo xtask check-verification-contracts --check` reads every
`schemas/**/*.schema.json` file from disk and requires each one to appear both
here and in [the verification README](README.md). The inventory is not a list
kept inside the checker, because a list is a second place to remember and the
schema it forgets is the one no reader learns is unaudited. That is how
`schemas/ripr/repair-attempt.schema.json` — a schema with a live producer —
sat outside this table while the gate reported success.

A schema whose producer state is `reserved` still needs a row. The row is what
records that no producer emits it.

## Claim boundary

This audit records that each currently reachable non-`riprAgent` published
schema is either bound to producer or corpus-of-record bytes with a
discriminating negative mutation, or carries an explicit exemption naming the
narrower authority that replaces fixture validation. Table completeness is
enforced; the strength of each individual row is not. It does not establish the
semantic correctness of the analysis behind those bytes, and it establishes
nothing about the reserved `riprAgent` protocol.
