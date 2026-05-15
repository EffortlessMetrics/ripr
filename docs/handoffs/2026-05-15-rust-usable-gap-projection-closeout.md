# Handoff: Rust Usable Gap Projection Closeout

Date: 2026-05-15
Branch / PR: `campaign/rust-usable-gap-projection-closeout` / pending at authoring
Latest merged PR: #1003 `docs(status): promote Rust gap repair loop`
(commit `ecd780a`)

## Current Work Item

`campaign/rust-usable-gap-projection-closeout`

Rust Usable Gap Projection made `ripr` route reviewer- and agent-facing
interruptions through a typed gap decision layer:

```text
raw static evidence
-> finding-to-gap alignment
-> gap decision ledger
-> PR comments, first-action, PR ledger, RIPR Zero, badges, gates, LSP, agents
-> receipt or report-only state
```

The shipped loop is now product-facing as the Rust gap repair loop:

```text
one repairable Rust gap
-> one focused test or output proof outside ripr
-> verification command
-> movement receipt
```

This campaign stayed projection-first. It did not add runtime mutation
execution, coverage adequacy claims, generated tests, provider calls, automatic
source edits, branch protection changes, default CI blocking, preview-language
promotion, or a public crate split.

Durable restart context:

- [implementation plan](../../plans/rust-usable-gap-projection/implementation-plan.md)
- [agent route](../../plans/rust-usable-gap-projection/agent-context-route.md)
- [closed manifest](../../.ripr/goals/archive/2026-05-15-rust-usable-gap-projection.toml)

## Prompt-To-Artifact Audit

| Requirement | Artifact | Validation |
| --- | --- | --- |
| Evidence and projection are separated | #927 added `RIPR-SPEC-0045`; #933 added `RIPR-SPEC-0046` | `cargo xtask check-spec-format`, `cargo xtask check-doc-index`, `cargo xtask check-traceability` |
| Gap vocabulary is fixture-backed | #936 added the gap decision ledger corpus | `cargo xtask fixtures`, `cargo xtask goldens check`, `cargo xtask check-fixture-contracts` |
| Ledger is a public report | #939 added `ripr reports gap-ledger --records`; #997 added conservative `--repo-exposure` derivation; #1000 added `--check-output` for presentation/output repair | `cargo test -p ripr gap_decision_ledger --lib`, `cargo test -p ripr reports_gap_ledger --lib`, `cargo xtask check-output-contracts` |
| First-action and PR ledgers consume gap records | #945 routed first useful action; #949 routed PR evidence ledger | CLI smoke tests, fixture/golden checks, output-contract checks |
| RIPR Zero, badges, and gates consume policy-backed gaps | #953 routed RIPR Zero and repo badges; #960 routed gate candidates; #984 added gap-ledger badge endpoint targets | policy/gate tests, badge checks, `cargo xtask check-capabilities` |
| PR comments are repair cards | #961 rendered PR repair cards from gap records | review-comment fixture tests and generated CI checks |
| Agent packets consume the same gap records | #963 rendered `ripr agent packet --gap-ledger ... --gap-id ...` | agent packet tests and output-contract checks |
| Editor projection consumes gap records read-only | #969, #973, #976, #981, and #983 validated, diagnosed, summarized, hovered, and acted on gap records without prose parsing | LSP tests, `cargo xtask lsp-cockpit-report`, VS Code smoke in the editor closeout |
| Presentation/output text has a real repair route | #1000 routes supported presentation text to `MissingOutputContract` / `AddOutputGolden` with `cargo xtask goldens check` | gap-ledger tests, output-contract checks, static-language checks |
| Adoption path is user-readable | #1002 added `docs/FIRST_PR_WORKFLOW.md`; #1003 promoted the Rust gap repair loop to `usable` in support tiers | `cargo xtask markdown-links`, `cargo xtask check-doc-index`, `cargo xtask check-static-language` |

## PR Chain

- #927 `docs(spec): define finding-to-gap alignment`
- #928 `docs(proposal): add Rust usable gap projection`
- #933 `docs(spec): add gap decision ledger contract`
- #936 `fixtures(gap): add gap decision ledger corpus`
- #939 `output(gap): add gap decision ledger renderer`
- #945 `report(gap): route first action through gap records`
- #949 `report(gap): route PR ledger through gap records`
- #953 `policy(gap): route RIPR Zero and repo badges through gap targets`
- #960 `policy(gap): route gate candidates through gap decisions`
- #961 `comments(gap): render PR repair cards from gap records`
- #963 `agent(gap): render packets from gap records`
- #969 `lsp(gap): project gap records into editor diagnostics`
- #973 `lsp(gap): add read-only gap artifact validation`
- #976 `lsp(gap): project gap state in Show Status`
- #981 `lsp(gap): enrich hover repair route`
- #983 `lsp(gap): add bounded repair actions`
- #984 `policy(badges): support gap-ledger endpoint targets`
- #997 `report(gap): derive ledger records from repo exposure`
- #1000 `analysis(gap): route presentation output gaps`
- #1002 `docs(workflow): add first successful PR workflow`
- #1003 `docs(status): promote Rust gap repair loop`
- `campaign/rust-usable-gap-projection-closeout`

Related but closed separately:

- #999 `campaign(lane3): close editor gap cockpit`

## Verification Run

Representative validation across the final shipped slices:

```bash
cargo test -p ripr gap_decision_ledger --lib --jobs 1
cargo test -p ripr reports_gap_ledger --lib --jobs 1
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-static-language
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-doc-roles
cargo xtask check-pr
git diff --check
```

The final support-tier PR also passed GitHub `rust`, `rust-coverage`,
`rust-tests-junit`, CodeQL, Droid review, security, VS Code, and Codecov patch
checks before merge.

## Shipped User Surfaces

- `ripr reports gap-ledger --records`
- `ripr reports gap-ledger --repo-exposure`
- `ripr reports gap-ledger --check-output`
- `ripr first-action --gap-ledger`
- `ripr pr-ledger record --gap-ledger`
- `ripr zero status --gap-ledger`
- `ripr gate evaluate --gap-ledger`
- `ripr review-comments --gap-ledger`
- `ripr agent packet --gap-ledger --gap-id`
- LSP gap diagnostics, status, hover, and bounded actions from validated gap
  artifacts
- repo badge formats and badge endpoint refresh using gap-ledger targets when
  supplied
- `docs/FIRST_PR_WORKFLOW.md`
- support-tier row: `Rust gap repair loop` -> `usable`

## Remaining Limits

- Rust gap repair is usable, not stable.
- Static evidence remains advisory unless an explicit gate decision owns
  pass/fail authority.
- Runtime mutation testing remains the execution-backed confirmation step.
- Public badges are repo-scoped; they do not imply PR-local test adequacy.
- Preview TypeScript, JavaScript, and Python evidence remains opt-in,
  preview-labeled, advisory, and ineligible for RIPR Zero or default gates
  without a future explicit promotion lane.
- `static_unknown` remains report-only unless a configured repair route exists.
- Output/presentation text routes to output/golden proof only when the evidence
  supports that repair route.

## Next Work Item

No ready work item remains in Rust Usable Gap Projection after this closeout.
Future work should open a new proposal or scoped campaign if maintainers want
runtime calibration promotion, stricter gate adoption, preview-language policy
promotion, richer first-run demos, or release packaging.

## What Not To Do

- Do not make raw `ExposureClass` or generic confidence text drive reviewer
  interruptions.
- Do not make `ripr 0`, `ripr+`, gates, PR comments, LSP diagnostics, or agent
  packets consume raw finding counts when a gap ledger is available.
- Do not treat the support-tier `usable` label as stable, runtime adequate, or
  coverage adequate.
- Do not promote preview-language evidence into Rust gap-repair authority.
- Do not add source edits, generated tests, provider calls, mutation execution,
  branch protection, default CI blocking, or public crate splits as closeout
  cleanup.
