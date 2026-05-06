# Implementation Campaigns

This is the campaign-level plan for Codex Goals and long-context contributor
work. Campaigns are larger than one PR. Each campaign has an objective, an end
state, and work items that should each follow the
[scoped PR contract](SCOPED_PR_CONTRACT.md).

The operational checklist remains in [Implementation plan](IMPLEMENTATION_PLAN.md).
The machine-readable active campaign is `.ripr/goals/active.toml`.

## Campaign 1: Agentic DevEx Foundation

Campaign ID: `agentic-devex-foundation`

Status: complete

Objective:

```text
Make the repo safe for autonomous Codex Goals work and human review.
```

Why it matters:

`ripr` is being built for long-context, agent-assisted implementation. The repo
must reject ambiguous PRs before review and produce enough receipts for humans
to evaluate trusted change instead of chat transcripts.

End state:

- architecture guard exists
- output-contract checks exist
- first behavior fixtures exist
- docs-as-tests baseline exists
- test-oracle report exists
- dogfood report exists
- Codex Goals campaign docs exist

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `policy/architecture-guard` | done | Workspace, architecture, and public API guardrails exist. |
| `output/output-contract-check` | done | Output contract registry checks exist. |
| `docs/docs-index-checks` | done | Docs index checks exist. |
| `docs/codex-goals-campaigns` | done | Clarify Codex Goals as multi-PR campaigns. |
| `docs/readme-state-and-link-checks` | done | README state and repo-local Markdown links are checked. |
| `goals/manifest-check` | done | Active campaign manifest is validated and reportable. |
| `fixtures/runner-comparison-v1` | done | Fixture and golden commands run `ripr` and compare actual outputs. |
| `fixtures/first-two-goldens` | done | `boundary_gap` and `weak_error_oracle` fixtures exist with JSON and human goldens. |
| `testing/test-oracle-report` | done | Advisory report measures `ripr`'s own strong, medium, weak, and smoke test oracles. |
| `dogfood/static-self-check` | done | Advisory `ripr`-on-`ripr` report runs stable fixture diffs and records current output. |
| `campaign/agentic-devex-closeout` | done | Campaign 1 is complete and Campaign 2 is active. |

Dependencies:

- Do not start analyzer rewrites until fixture and golden scaffolding can record
  behavior.
- Do not treat test-oracle reports as blocking until baseline debt is measured.

Commands:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask pr-summary
cargo xtask fixtures
cargo xtask goldens check
cargo xtask test-oracle-report
cargo xtask dogfood
cargo xtask metrics
```

Blocking conditions:

- policy exception required
- architecture exception required
- output schema change required
- golden blessing needed without explicit review scope
- campaign item depends on an unmerged non-stackable PR

Review policy:

Work items should usually produce one scoped PR. Independent docs or reporting
items may be stackable when the campaign manifest marks them that way.

## Campaign 2: Syntax-Backed Analyzer Foundation

Campaign ID: `syntax-backed-analyzer-foundation`

Status: complete

Objective:

```text
Move the analyzer from lexical facts to syntax-backed facts.
```

Why it matters:

Current analyzer behavior still has line-oriented surfaces. `ripr` needs a
stable fact model and parser adapter boundary before replacing lexical checks.

End state:

- `FileFacts` model exists
- syntax adapter boundary exists
- Rust parser substrate is recorded in an ADR
- tests and oracles are extracted from syntax-backed facts
- probes attach to stable owner symbols
- current probe families are generated from syntax facts

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `analysis/file-facts-model` | done | FileFacts DTOs exist and the lexical scanner fills them without output drift. |
| `analysis/syntax-adapter-mvp` | done | RustSyntaxAdapter boundary exists with lexical adapter compatibility. |
| `design/rust-syntax-substrate` | done | ADR 0006 selects `ra_ap_syntax` behind the adapter and keeps parser types internal. |
| `analysis/ast-test-oracle-extraction` | done | Parser-backed facts identify test functions, assertion macros, and unwrap/expect smoke oracles. |
| `analysis/ast-probe-ownership` | done | Changed lines map to module- and impl-qualified owner symbols without cross-linking duplicate names. |
| `analysis/ast-probe-generation` | done | Current probe families are generated from parser-backed probe shape facts with lexical fallback. |

Dependencies:

- `analysis/file-facts-model` should merge before syntax adapter work.
- Parser-backed extraction should use the substrate decision in
  [ADR 0006](adr/0006-rust-syntax-substrate.md).
- Analyzer work items are non-stackable unless the manifest explicitly says
  otherwise.

Commands:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask pr-summary
```

Blocking conditions:

- output drift without golden evidence
- parser-specific types leaking outside the syntax adapter
- architecture exception required
- missing stop reason for new unknowns

Review policy:

Each analyzer work item should include spec, fixture or test, output contract
evidence when user-visible output changes, metrics movement when capability
status changes, and a clear non-goal list.

## Campaign 3: Evidence Quality

Campaign ID: `evidence-quality`

Status: complete

Objective:

```text
Make findings explain changed behavior, oracle strength, propagation, activation,
and unknown stop reasons with enough precision to guide test work.
```

End state:

- oracle kind and strength are probe-relative
- local delta flow can name visible sinks
- activation modeling can name observed and missing discriminator values
- output is evidence-first
- unknown findings include stop reasons across surfaces
- negative and metamorphic fixtures protect evidence-first output

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `output/unknown-stop-reason-invariant` | done | Unknown classifications carry stop reasons across domain, JSON, context, GitHub annotations, and human output. |
| `analysis/oracle-strength-v2` | done | Oracle kind and strength distinguish exact error variants, exact values, broad errors, smoke-only checks, snapshots, relational checks, and mock expectations. |
| `analysis/local-delta-flow-v1` | done | Findings carry typed local flow sinks for visible return, error, field, match-arm, and effect boundaries. |
| `analysis/activation-value-modeling-v1` | done | Findings carry observed value facts and missing discriminator facts tied to local flow evidence. |
| `output/evidence-first-output` | done | Human and JSON output render changed behavior, evidence path, weakness, stop reasons, and next action as first-class finding evidence. |
| `fixtures/negative-metamorphic-baseline` | done | Negative and metamorphic fixtures cover whitespace/comment/import noise, unrelated token mentions, strong boundary/error oracles, and syntax variants. |
| `campaign/evidence-quality-closeout` | done | Campaign 3 closed with evidence-first output and negative/metamorphic fixture guardrails. |

Dependencies:

- `output/unknown-stop-reason-invariant` should land before deeper unknown
  evidence grows so silent unknowns do not become accepted output.
- `analysis/local-delta-flow-v1` landed before activation/value modeling.
- `analysis/activation-value-modeling-v1` landed before evidence-first output.
- `output/evidence-first-output` landed before negative/metamorphic fixture
  expansion.
- `fixtures/negative-metamorphic-baseline` should land before Campaign 3
  closeout so the evidence-first output has negative and metamorphic guardrails.

Commands:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-output-contracts
cargo xtask pr-summary
```

Blocking conditions:

- unknown classification without a stop reason
- output drift without golden evidence
- schema change required outside the scoped PR
- fixture expansion before evidence fields are stable

Review policy:

Campaign 3 work should improve evidence precision without claiming real mutation
outcomes. Unknown is acceptable, but it must be explicit and actionable.

## Campaign 4A: Test Efficiency and Vacuity Signals

Status: complete

Objective:

```text
Make low-discriminator tests visible from the same evidence facts used for
static exposure findings.
```

End state:

- per-test ledgers name reachable owners, oracle kind and strength, observed
  values, and static limitations
- likely-vacuous, smoke-only, broad-oracle, opaque, circular, and `duplicative`
  signals are advisory
- reports explain evidence and suggested next steps without calling tests bad
- test-efficiency metrics are available for trend tracking
- agent and editor surfaces can avoid imitating low-discriminator tests
- `ripr` and `ripr+` badge artifacts publish unresolved-finding counts as
  inbox-zero signals, with intent and suppressions as durable exception files

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `test-efficiency/test-fact-ledger` | done | `cargo xtask test-efficiency-report` writes advisory per-test ledgers with reached owners, oracle kind/strength, observed values, and static limitations. |
| `test-efficiency/vacuous-signal-v1` | done | The advisory report now records smoke-only, broad-oracle, disconnected, opaque, circular, and likely-vacuous reasons. |
| `test-efficiency/duplicate-discriminator-v1` | done | Advisory groups expose tests sharing an owner set, role-aware activation signature, and oracle shape; members are reclassified `duplicative` with reason `duplicate_activation_and_oracle_shape` and a per-test `duplicate_group_id` linked to the top-level `duplicate_groups` array. Already-flagged classes (`opaque`, `likely_vacuous`, `possibly_circular`) are preserved. |
| `test-efficiency/report-and-metrics` | done | Top-level `metrics` object in `target/ripr/reports/test-efficiency.json` exposes `tests_scanned`, `class_counts` (all seven classes), `reason_counts` (all emitted reasons), and `duplicate_discriminator_group_count = duplicate_groups.length`. The `duplicative` test count and the group count are intentionally distinct fields. Capability metadata in `metrics/capabilities.toml` references the new metrics surface. |
| `docs/badge-policy` | done | [Badge policy](BADGE_POLICY.md) locks the badge counting rule, native JSON shape, Shields projection, and exact emitted vocabulary. |
| `badge/summary-renderer-v1` | done | Private `BadgeSummary`, `BadgeCounts`, `BadgePolicy`, `BadgeKind`, `BadgeStatus` live in `pub(crate) mod output::badge`. `ripr_badge_summary` derives counts from `CheckOutput`; `render_native_json` and `render_shields_json` produce the wire shapes. 14 unit tests. Public API and `policy/public_api.txt` unchanged. |
| `badge/ripr-count-v1` | done | `ripr check --format badge-json` and `--format badge-shields` dispatch through `output::badge::ripr_badge_summary` plus the native and Shields renderers from #189. The temporary `#![allow(dead_code)]` in `output/badge.rs` and its `.ripr/allow-attributes.txt` entry are removed. CLI smoke tests cover both formats and confirm `badge-plus-*` formats remain rejected until `badge/ripr-plus-count-v1`. |
| `test-intent/v1` | done | `.ripr/test_intent.toml` loader attaches `declared_intent` metadata (intent, owner, reason, source) to matching test-efficiency entries. The original `class` is preserved — intent is additive metadata, never a replacement. Unmatched and ambiguous (name-only) selectors fail the report; declared tests remain visible in both the JSON ledger and the Markdown `## Declared Test Intent` section. |
| `badge/ripr-plus-count-v1` | done | `ripr check --format badge-plus-json` and `--format badge-plus-shields` read `target/ripr/reports/test-efficiency.json` (relative to `--root`), sum unsuppressed exposure gaps and unsuppressed actionable test-efficiency findings, exclude entries with `declared_intent` metadata, and report `opaque` entries as `unknowns_test_efficiency`. Missing report fails clearly with a regenerator hint. |
| `suppressions/v1` | done | `.ripr/suppressions.toml` loader with closed-set kinds (`exposure_gap`, `test_efficiency`); `owner` + `reason` required, `expires` optional in `YYYY-MM-DD`. Expired entries do **not** apply and surface as warnings — silent green-forever debt is impossible. Suppressed findings stay visible in detailed reports; the badge counts move them from `unsuppressed_*` to `suppressed_*`. Native badge JSON gains a `warnings` array; Shields stays exactly four fields. |
| `ci/badge-artifacts` | done | `cargo xtask badge-artifacts` writes `ripr-badge.json`, `ripr-badge-shields.json`, `ripr-plus-badge.json`, `ripr-plus-badge-shields.json`, and `ripr-badges.md` to `target/ripr/reports/`. The CI workflow runs `cargo xtask test-efficiency-report` then `cargo xtask badge-artifacts` (both advisory, both `\|\| true`); the existing `Upload ripr reports` step picks up the new files; the badges Markdown is appended to `$GITHUB_STEP_SUMMARY`. The `badge-artifacts` task captures `git diff origin/main...HEAD` to `target/ripr/badge-input.diff` and runs each format against `--root .` so exposure and test-efficiency analyze the same codebase. New `ReceiptSpec` covers all five files. Advisory by default — no `--fail-on-nonzero`. |
| `badge/repo-scope-artifacts` | done | `cargo xtask repo-badge-artifacts` analyzes the full repo baseline through `run_repo_analysis` (every currently-probeable production syntax shape, not a diff) and writes `repo-ripr-badge.json`, `repo-ripr-badge-shields.json`, `repo-ripr-plus-badge.json`, `repo-ripr-plus-badge-shields.json`, and `repo-ripr-badges.md`. Native badge JSON now carries a `scope` field (`"diff"` or `"repo"`) on schema `0.2`; Shields projection stays exactly four fields. New `OutputFormat::RepoBadge*` variants route through `app::check_workspace_repo`; existing diff-scoped `cargo xtask badge-artifacts` and the `BadgeJson`/`BadgeShields`/`BadgePlus*` formats are unchanged. The v1 baseline is the *currently-probeable* repo surface — not full seam inventory, not mutation adequacy proof; the deeper seam / test-grip model is tracked as later work. |
| `badge/publish-main-endpoint` | done | The two repo-scoped Shields JSON files (`badges/ripr.json`, `badges/ripr-plus.json`) are committed to `main` and served via `raw.githubusercontent.com/EffortlessMetrics/ripr/main/badges/...`. Root `README.md` renders them via `img.shields.io/endpoint`. Refresh: `cargo xtask update-badge-endpoints` (regenerates from `repo-badge-artifacts` and copies into `badges/`). Verify (advisory, not yet a hard CI gate): `cargo xtask check-badge-endpoints`. Pages deployment was prototyped and rejected as over-engineered for v1 dogfood — it would have required Pages enablement, a deploy workflow, and would have implied downstream users must also enable Pages. The `ripr` product contract is "ripr emits Shields-compatible JSON"; hosting is replaceable. See `deferred/hosted-badge-service` in `docs/DEFERRED.md`. |
| `campaign/test-efficiency-closeout` | done | Campaign 4A marked complete here and in `.ripr/goals/active.toml`. Final architecture: per-test ledger + class/reason metrics from `cargo xtask test-efficiency-report`; `.ripr/test_intent.toml` declarations and `.ripr/suppressions.toml` exceptions wired into the `ripr+` count; diff-scoped PR badge artifacts via `cargo xtask badge-artifacts` (#195); repo-scoped baseline via `cargo xtask repo-badge-artifacts` (#204) on schema 0.2 with `scope: "repo"`; checked-in `badges/ripr.json` and `badges/ripr-plus.json` rendered through `img.shields.io/endpoint?url=https://raw.githubusercontent.com/EffortlessMetrics/ripr/main/badges/...` (#209). Final dogfood snapshot at this campaign close: `ripr 163`, `ripr+ 163` (`main` = `6b4b2b0`); snapshot, **not** a fixture expectation. PR chain: #195, #198, #199, #200, #204, #205 (DEFERRED.md), #206 (friction-log graduation), #208 (stale-`317`-headline correction), #209. Issue #207 was the endpoint design-plan. Pages was rejected for v1 dogfood; hosted badge service is `deferred/hosted-badge-service`. The seam-inventory + test-grip product reframe is **next-campaign work** (`deferred/seam-inventory-test-grip`), not unfinished 4A work. |

Dependencies:

- Campaign 3 evidence fields should remain the source of truth; test-efficiency
  work should not invent a separate classifier for changed behavior.
- The first report should be advisory and should not fail CI.
- Badge counting must use the exact emitted strings audited in
  [Badge policy](BADGE_POLICY.md); aspirational class names that the reporter
  does not produce must not appear in the badge schema.
- `test-intent/v1` ships before `suppressions/v1` so intentional smoke and
  duplicate tests are positive declarations, not exception entries.

Commands:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask pr-summary
cargo xtask reports index
cargo xtask receipts check
cargo xtask test-oracle-report
```

Blocking conditions:

- output says a test is bad instead of reporting evidence and risk shape
- static analysis suggests deleting tests
- report language becomes blocking policy before calibration/configuration
- new automation bypasses Rust-first `xtask` policy

## Campaign 4: Editor and Agent Loop

Objective:

```text
Turn findings into editor and agent actions that help produce targeted tests.
```

End state:

- LSP diagnostics carry finding and probe IDs
- hovers show evidence for the selected finding
- code actions can copy context packets or open related tests
- context packets include missing values and assertion shapes

The original Campaign 4 plan was a direct extension of Campaign 3's
`Finding`/`StageEvidence` model. Campaign 4A (Test Efficiency) made
clear that the editor/agent surface needs a richer substrate —
behavior seams classified by test-grip evidence rather than ad-hoc
finding metadata. The continuation lives under Campaign 4B; the work
items below are subsumed there with seam-aware shapes:

| Work item | Status | Notes |
| --- | --- | --- |
| `lsp/evidence-hover-actions` | superseded | Folded into Campaign 4B as `lsp/seam-evidence-hover-v1` (preceded by `lsp/repo-seam-diagnostics-v1`). |
| `context/agent-context-v2` | superseded | Folded into Campaign 4B as `context/agent-seam-packets-v1`, scoped around `RepoSeam` and `SeamGripClass`. |
| `docs/how-to-use-agent-context` | superseded | Folded into Campaign 4B as `docs/agent-dispatch-workflow-v1`. |

## Campaign 4B: Repo Seam Inventory and Test Grip

Campaign ID: `repo-seam-inventory-test-grip`

Status: complete

Objective:

```text
Inventory behavior seams across the repo, classify how strongly current tests
grip each seam through RIPR evidence, and turn actionable gaps into editor
diagnostics and agent-ready test packets.
```

The Voice A baseline shipped in Campaign 4A
(`badge/repo-scope-artifacts`, #204) becomes a special case of seam
classification rather than the analyzer's only repo mode. The seam
evidence loop is the editor/agent loop with the right substrate:
first-class `RepoSeam` and `SeamGripClass` underneath, evidence-first
hover and agent packets on top.

End state:

- `RepoSeam`, `SeamKind`, `RequiredDiscriminator`, and `SeamGripClass`
  exist as a first-class data model
- seam IDs are stable across runs and across input file walk reorderings
- test-grip evidence per seam covers reach, activate/infect,
  propagate, observe, discriminate
- a separate `SeamGripClass` / `TestGripClass` is used for grip
  classification; mapping to existing `ExposureClass` and to badge
  counts is explicit, not implicit through type extension
- a repo exposure report enumerates seams with their grip class and
  missing-discriminator hypothesis
- LSP diagnostics surface ungripped or under-gripped seams
- hover renders the RIPR evidence path for the classification with
  cited related tests
- agent context packets carry the load-bearing fields a coding agent
  needs to write the missing test
- public repo badge counts can be derived from seam classification
  without breaking the existing schema
- static-language constraints hold: no `killed`/`survived`/`proven`/
  `adequate` in static output
- static seam evidence does not pretend to prove mutation adequacy

**Pre-4B LSP groundwork.** Before the seam model was ready, three PRs
built editor/agent surfaces on the current `Finding` / `AnalysisSnapshot`
model. They protect the LSP loop and provide fallback behavior while
Campaign 4B types are being designed:

- **PR #211** — evidence-rich hover over current `Finding` /
  `AnalysisSnapshot`, replacing generic "evidence found" text with real
  `StageEvidence.summary`, related-test oracle text, and weakness rendering.
- **PR #218** — LSP `executeCommand` `ripr.collectContext` with
  server-side context packet lookup and VS Code LSP-first / CLI-fallback
  `copyContext` path.
- **PR #219** — VS Code extension e2e smoke tests for activation,
  command registration, `copyContext`, and `restartServer`; wired CI
  `xvfb-run` step.

Campaign 4B LSP work (`lsp/repo-seam-diagnostics-v1`,
`lsp/seam-evidence-hover-v1`, `context/agent-seam-packets-v1`) will
extend or revise these surfaces for `RepoSeam` / `SeamGripClass`.

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/repo-seam-inventory` | done | Landed in #223 as `docs/specs/RIPR-SPEC-0005-repo-seam-inventory.md`; defines `RepoSeam`, `SeamKind`, `RequiredDiscriminator`, `TestGripEvidence`, `SeamGripClass`, stable seam ID rules, the relationship to `ProbeShapeFact`, headline-vs-visible mapping, static-language boundaries, and the Voice A vs Voice B contract. |
| `analysis/repo-seam-model-v1` | done | Landed in #229 as `crates/ripr/src/analysis/seams.rs`; introduces `RepoSeam`, `SeamId`, `SeamKind`, `ExpectedSink`, `RequiredDiscriminator`, `SeamGripClass` as crate-private types per RIPR-SPEC-0005. Deterministic 16-char `SeamId` via FNV-1a 64-bit; no public Rust API change; no LSP; no badge change. |
| `analysis/repo-seam-inventory-v1` | done | Walks production Rust files and emits `Vec<RepoSeam>`; writes `target/ripr/reports/repo-seams.{json,md}` via `cargo xtask repo-seam-inventory`. Initial seam kinds: predicate_boundary, error_variant, return_value, field_construction, side_effect, match_arm, call_presence (`validation_branch` deferred to a follow-up detection PR). |
| `analysis/test-grip-evidence-v1` | done | Crate-private `TestGripEvidence` + `RelatedTestGrip` attaching reach/activate/propagate/observe/discriminate evidence per inventoried seam. No classification, no public report. Built from existing `RustIndex` / `OracleFact` / `ValueFact` facts. |
| `analysis/repo-ripr-classification-v1` | done | Crate-private `SeamGripClass` (re-introduced) + `classify_seam(seam, evidence)` mapping `TestGripEvidence` to one of 11 spec classes. Headline-vs-visible table on `is_headline_eligible`. Replaces the stage-zero discard hook from #236 with a real classifier consumer. |
| `output/repo-exposure-report-v1` | done | `cargo xtask repo-exposure-report` writes `target/ripr/reports/repo-exposure.{json,md}` from the classified seam inventory; `repo-exposure-json` / `repo-exposure-md` formats live in `crates/ripr/src/output/repo_exposure.rs`. Schema 0.1 documented in `docs/OUTPUT_SCHEMA.md` § "Repo Exposure Report". Replaces the stage-zero classification discard from #237 with the real renderer consumer. |
| `lsp/repo-seam-diagnostics-v1` | done | LSP publishes seam diagnostics with stable `ripr-seam-{class}` codes when `seamDiagnostics: true` is set in initialization options. WARNING for `weakly_gripped`/`ungripped`/`reachable_unrevealed`; INFORMATION for the four `*_unknown` classes and `opaque`. `strongly_gripped`/`intentional`/`suppressed` produce no diagnostic. Off by default until `cache/repo-seam-facts-v1` lands. Diagnostic data carries `seam_id` for hover lookup. |
| `lsp/seam-evidence-hover-v1` | done | LSP hover for seam diagnostics: looks up `ClassifiedSeam` via `data.seam_id` and renders the seam evidence path (grip class, all five RIPR stages with summary, observed values, missing discriminator, related tests with oracle kind/strength, per-kind next step). Pre-4B Finding hover still works for diff-scoped diagnostics — backend prefers seam hover when `seam_id` is present, otherwise falls through to Finding hover. Code-action work deferred. |
| `context/agent-seam-packets-v1` | done | `cargo xtask agent-seam-packets` writes `target/ripr/reports/agent-seam-packets.json`. Schema 0.2 in `crates/ripr/src/output/agent_seam_packets.rs`. Each headline-eligible classified seam emits one `write_targeted_test` packet with seam_id, owner, kind, expression, current_grip, RIPR evidence, observed values, missing input values, missing oracle shape, related tests, and assertion templates. Strongly-gripped/opaque/intentional/suppressed seams emit no packet. |
| `docs/agent-dispatch-workflow-v1` | done | `docs/AGENT_DISPATCH_WORKFLOW.md` documents the practical loop: run ripr → inspect report/diagnostic → read seam evidence hover → copy seam packet → hand to agent → agent writes targeted test → rerun ripr → optional cargo-mutants confirmation. Includes per-kind examples (predicate boundary, error variant, return value, field construction, side effect, opaque, intentional, suppressed) and explicit pushback against "add more tests" / "coverage is fine" / "this is proven". Linked from `docs/DOCUMENTATION.md`. |
| `cache/repo-seam-facts-v1` | rolled-forward | Carried forward into Campaign 5 (Adoption and Calibration). Optional fact-layer cache (file-facts, owner-index, seam-facts; never final outputs). Gated on real performance signal. Landed in Campaign 5A as #255. |
| `calibration/cargo-mutants-v1` | rolled-forward | Carried forward into Campaign 5. Optional scaffold for comparing static `SeamGripClass` against cargo-mutants outcomes. Advisory only; static output adopts no mutation-runtime language. |
| `campaign/seam-inventory-test-grip-closeout` | done | Campaign 4B marked complete here and in `.ripr/goals/active.toml`. Repo seam evidence is now first-class: `RepoSeam` model, repo seam inventory, `TestGripEvidence`, `SeamGripClass` classification, repo exposure report, agent seam packets, LSP seam diagnostics, seam evidence hover, and agent dispatch workflow docs. Static output remains evidence-first; runtime mutation testing remains a separate confirmation step (`calibration/cargo-mutants-v1` in Campaign 5). PR chain: #229, #235, #236, #237, #239, #240, #241, #242, #248. The active manifest now points at Campaign 5; `cache/repo-seam-facts-v1` and `calibration/cargo-mutants-v1` carry forward as ready items there. |

Dependencies:

- `spec/repo-seam-inventory` landed in #223, `analysis/repo-seam-model-v1`
  in #229, `analysis/repo-seam-inventory-v1` in #235,
  `analysis/test-grip-evidence-v1` in #236,
  `analysis/repo-ripr-classification-v1` in #237, and
  `output/repo-exposure-report-v1` follows. Recommended next core
  steps: `context/agent-seam-packets-v1` (agent work-order packets) or
  `lsp/repo-seam-diagnostics-v1` (editor surface). `cache/repo-seam-facts-v1`
  and `calibration/cargo-mutants-v1` remain unblocked but optional.
- `lsp/seam-evidence-hover-v1` extends or revises PR #211, which is
  already merged as pre-4B evidence-rich hover over the current
  Finding / AnalysisSnapshot model. The seam-native hover will
  supersede the Finding-backed hover once RepoSeam and SeamGripClass
  are stable.
- PR #218 (LSP executeCommand `ripr.collectContext`) and PR #219
  (VS Code extension smoke tests) are also pre-4B groundwork merged
  before Campaign 4B seam work began. Campaign 4B agent and editor
  surfaces will build on or replace these current-model implementations.
- `cache/repo-seam-facts-v1` and `calibration/cargo-mutants-v1`
  subsume their broader analogs from Campaign 5; Campaign 5 retains
  its config and CI policy work.

Commands:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask check-spec-format
cargo xtask check-spec-ids
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask markdown-links
cargo xtask check-doc-index
```

Blocking conditions:

- analyzer code committed before the spec lands
- `SeamGripClass` extended without explicit mapping to badge counts
- runtime-mutation language (`killed`, `survived`, etc.) leaking into
  static seam reports
- public Rust API surface change without a `policy/public_api.txt`
  update
- LSP / agent surfaces shipped before the seam model and report are
  settled

Review policy:

This campaign sits inside the operating contract codified in
[`docs/reference/AGENT_HANDOFF_PROTOCOL.md`](reference/AGENT_HANDOFF_PROTOCOL.md).
Spec/model work pings the owner; mechanical sub-step work proceeds
inline once authorized.

## Campaign 5A: Seam Evidence Usability and Precision

Campaign ID: `seam-evidence-usability-and-precision`

Status: done

Objective:

```text
Make repo seam evidence fast, precise, and directly actionable for
developers and coding agents, without adopting mutation-runtime
language in static output.
```

Why it matters:

Campaign 4B made repo seam evidence first-class (RepoSeam,
TestGripEvidence, SeamGripClass, repo exposure report, agent seam
packets, LSP diagnostics, hover, agent dispatch docs). The signal is
visible but not yet useful every day: full-repo seam classification
adds multi-second editor latency (so `seamDiagnostics` ships off by
default), related-test fanout is broad, many seams classify as
`activation_unknown` because value extraction does not yet cover
common Rust test data patterns, oracle-shape detection misses
real-world assertion shapes (field assertions, whole-object equality,
mock expectations), and packets explain the gap without telling an
agent where and how to close it. This campaign closes that gap along
four product axes: fast (cache), precise (related-test, value,
oracle-shape), actionable (agent packets v2, LSP code actions), and
calibrated (cargo-mutants).

Operationalization items (`config/ripr-config-v1`,
`ci/sarif-ci-policy`) move to Campaign 5B because their defaults and
severity model depend on cache performance and oracle-shape
stability.

End state:

- seam fact layers cache cleanly so the cold path still works and the
  warm path avoids full repo seam walk when inputs are unchanged
- cache invalidates on source/config/intent/suppression changes; repo
  exposure report and LSP diagnostics consume the same cached fact
  source
- no rendered outputs are cached; cache serialization stays behind a
  codec boundary; binary serialization, when introduced, uses
  `postcard` (never `bincode`)
- related-test fanout is reduced and ranked; related tests carry
  `relation_reason` and `relation_confidence`; high-fanout files show
  fewer irrelevant top related tests
- activation/value evidence detects common Rust test data patterns
  (let bindings, constants, builder methods, table-driven cases,
  rstest cases, enum variants, `Option`/`Result` constructors,
  fixture factories); `activation_unknown` count falls without new
  false positives
- oracle-shape evidence recognizes `assert_matches` exact variants,
  field assertions, whole-object equality, snapshot calls with
  visible field names, mock expectations, and event/state/persistence
  assertions
- agent seam packet v2 carries recommended test name, recommended
  test file, nearest strong test to imitate, candidate input values,
  assertion shape with example, patterns to imitate, patterns to
  avoid, and confidence — enough to write the targeted test directly
- LSP code actions surface "Copy seam packet", "Copy suggested
  assertion", "Open related test", and "Refresh ripr analysis" for
  diagnostics that carry `seam_id`; no automatic edits
- calibration scaffold compares static `SeamGripClass` against
  cargo-mutants outcomes; runtime mutation vocabulary stays inside
  calibration/runtime reports; static reports keep the audit
  vocabulary

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `cache/repo-seam-facts-v1` | done | Landed in #255. Workspace-level `Vec<ClassifiedSeam>` fact cache at `target/ripr/cache/repo-seam-facts/{schema_version}/{key_hash}.json`. `serde_json` behind a codec module boundary; never bincode. Cache key hashes the same Rust file set fed to `build_index` (production seam sources + test evidence sources), workspace root, cfg/features, config, test intent, suppressions, analyzer version, and schema version — so test-only edits invalidate. Cold path on miss / corrupt; store failures never fail analysis. Renders (JSON, Markdown, diagnostics, hover, packets) stay outside the cache. |
| `analysis/related-test-precision-v1` | done | Landed in #310. Adds `relation_reason` and `relation_confidence` to related tests; ranks related tests in repo exposure report, agent packets, and LSP hover. Reduces noisy fanout without removing `related_tests_total`. Schema bumps: cache `0.1→0.2`, agent_seam_packets `0.2→0.3`, repo_exposure `0.1→0.2`. Comment/string-stripping defense added for `import_path_affinity`. |
| `analysis/value-extraction-v2` | done | Adds syntactic value resolution for let bindings, same-file constants/statics, builder and fixture-override methods, table-driven loops, rstest cases, enum variants, and one-level `Option`/`Result` constructors. Keeps string/comment shadows, cross-file constants, and unrelated builder tokens from inflating observed values. |
| `analysis/oracle-shape-v2` | done | Expands oracle-shape detection for field assertions, whole-object equality over visible struct literals, event/state/persistence observers, mock expectations, and simple custom assertion helpers. Keeps `is_err` broad and exact `assert_matches!(..., Err(...))` strong without learned priors or helper-body analysis. |
| `context/agent-seam-packets-v2` | done | Schema 0.3 packets now carry `recommended_test`, `nearest_strong_test_to_imitate`, `candidate_values`, `assertion_shape` (kind + example), `patterns_to_imitate`, `patterns_to_avoid`, and recommendation `confidence`. Uses ranked related tests from `analysis/related-test-precision-v1` when available; no automatic edits or generated test skeletons. |
| `lsp/seam-code-actions-v1` | done | Seam diagnostics now surface code actions for copying the selected seam packet, copying a concrete suggested assertion when the agent packet assertion shape is available, opening the nearest related test when a related-test location is present, and refreshing ripr analysis. Finding diagnostic context-copy actions still work. No automatic edits, generated tests, CodeLens, or in-memory overlays. |
| `calibration/cargo-mutants-v1` | done | Adds advisory `cargo xtask mutation-calibration` report generation. Imported cargo-mutants JSON/output is joined to static `SeamGripClass` evidence by `seam_id` first and unambiguous normalized file/line second; span-based locations are imported, ambiguous file/line candidates stay unassigned, and unmatched runtime mutants remain visible; runtime mutation vocabulary stays inside `target/ripr/reports/mutation-calibration.{json,md}`. |
| `campaign/seam-evidence-usability-closeout` | done | Final Campaign 5A state transition. Closed the campaign after #255, #310, #313, #314, #315, #316, and #327 landed; operationalization items moved to Campaign 5B. |

Dependencies:

- `cache/repo-seam-facts-v1` does not block the precision items
  technically, but landing it first lets the precision PRs benchmark
  warm/cold paths without rerunning full inventory.
- `analysis/related-test-precision-v1` should land before
  `context/agent-seam-packets-v2` so v2 packets can use ranked
  related tests as `patterns_to_imitate` / `patterns_to_avoid`.
- `analysis/oracle-shape-v2` can land independently now that
  `analysis/value-extraction-v2` has stabilized the value evidence
  floor.
- `lsp/seam-code-actions-v1` should land after
  `context/agent-seam-packets-v2` so the "Copy suggested assertion"
  action can use the v2 `assertion_shape` field.
- `calibration/cargo-mutants-v1` is independent and can land any time.

Commands:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask goldens check
cargo xtask check-output-contracts
cargo xtask check-static-language
```

Blocking conditions:

- bincode introduced as a serialization dependency (use postcard)
- rendered outputs cached (only fact layers may be cached)
- mutation-runtime language (`killed`, `survived`, `proven`,
  `adequate`) leaking from calibration into static reports
- output drift without golden evidence
- `seamDiagnostics` flipped on by default before cache lands

Review policy:

This campaign is product work, not refactor work. Each work item
should preserve the spec/test/code/output trail. PRs that mix
implementation with refactoring should be split.

Closeout:

Campaign 5A is complete. Landed PR chain:

- #255 `cache/repo-seam-facts-v1`
- #310 `analysis/related-test-precision-v1`
- #313 `analysis/value-extraction-v2`
- #314 `analysis/oracle-shape-v2`
- #315 `context/agent-seam-packets-v2`
- #316 `lsp/seam-code-actions-v1`
- #327 `calibration/cargo-mutants-v1`

The active campaign now moves to Campaign 5B. Config, SARIF, and
badge count remapping are operationalization work, not unfinished
5A precision work.

## Campaign 5B: Operationalization

Campaign ID: `operationalization`

Status: complete

Objective:

```text
Make ripr deployable: repository config governs analyzer behavior,
SARIF and CI policy modes integrate with PR workflows, and the badge
schema can be remapped onto seam-native counts.
```

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `config/ripr-config-v1` | done | Repo-root `ripr.toml` governs analysis mode, oracle policy, severity mapping, suppressions path, report caps, and LSP seam-diagnostic defaults while explicit CLI/LSP options still win. |
| `ci/sarif-ci-policy` | done | SARIF and policy modes consume configured severity and suppression policy; RIPR-SPEC-0008 pins the rule IDs, severity mapping, suppression visibility, advisory default, renderer, and opt-in baseline policy. |
| `badge/seam-native-count-mapping` | done | Repo-scoped `ripr` and `ripr+` badges now count configured-visible seam-native unresolved gaps, while diff-scoped badge artifacts remain versioned as legacy finding-exposure counts. Native badge JSON is schema `0.3` with `basis` and `counts.analyzed_seams`; Shields endpoint artifacts were refreshed together. |
| `campaign/operationalization-closeout` | done | Closed Campaign 5B after config, SARIF/CI policy, and seam-native badge count mapping landed. The next active campaign is Campaign 6, starting with a draft-stack audit before structural refactors. |

Review policy:

5B started with `config/ripr-config-v1`, then landed SARIF rendering and the
opt-in baseline policy, then remapped public repo badges onto seam-native counts.
The closeout is docs/manifest only: no analyzer behavior, output schema, SARIF
policy, or badge mapping changes.

## Campaign 6: Module SRP Refactoring

Campaign ID: `modularize-ripr-submodules`

Status: active

Objective:

```text
Refactor internal modules under crates/ripr/src/ so each module has one
product responsibility, improving maintainability, testability, and reasoning
without splitting the package.
```

Why it matters:

Current modules mix responsibilities (e.g., `analysis/mod.rs` orchestrates pipeline
and counts summaries; `analysis/rust_index.rs` parses, indexes, and extracts facts).
This makes behavior changes ripple across boundaries, testing harder, and future
modularization (async, parallelism, caching) more complex. Module boundaries should
align with RIPR stages and clear responsibilities.

End state:

```text
crates/ripr/src/
  domain/           — stable data model
  app/              — use-case orchestration
  analysis/
    diff/           — diff parsing
    workspace/      — file discovery and scope
    facts/          — fact model and index
    syntax/         — syntax adapter
    extract/        — fact extraction
    probes/         — probe generation
    classify/       — classification pipeline
  output/           — rendering
  cli/              — argv parsing and execution
  lsp/              — LSP server
  xtask/            — repo automation
```

The ripr package **stays one crate** with one published library and binary. Do not
split into `ripr-core`, `ripr-cli`, `ripr-lsp`, or schema crates.

Hard constraints:

```text
- Do not split the crate
- No JSON schema changes
- No static output language changes
- No new probe families or classification behavior changes
- Preserve all public behavior and CLI surface
- Re-bless goldens only if the PR intentionally changes output
```

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `campaign/modularization-stack-audit` | done | Audited the old Campaign 6 draft stack against current `main` after Campaign 5B closeout. Canonical order is #244 -> #245 -> #246 -> #247 -> #249 -> #251 -> new PR 6 syntax-adapter extraction -> #253; #250 stays parked and should close or rewrite after the facts/syntax/build-index path stabilizes. |
| `modularization/infrastructure-and-planning` | done | Documentation, campaign outline, first-PR pattern, and the post-5B stack audit exist; implementation resumes with the canonical stack order below. |
| `analysis/summary-extraction` | done | PR 1 (#244): Extracted duplicated summary and sort logic from `analysis/mod.rs` into focused helper modules with no output/API/schema drift. |
| `analysis/pipeline-extraction` | done | PR 2 (#245): Extracted diff and repo pipeline orchestration into `analysis/pipeline.rs` while preserving `run_analysis` and `run_repo_analysis` as stable facades. |
| `diff/module-split` | done | PR 3 (#246): Split `analysis/diff.rs` into `diff/{mod,model,load,parse}.rs` with the parser and git-diff adapter behavior preserved. |
| `workspace/module-split` | done | PR 4 (#247): Split workspace concerns into focused modules without changing workspace selection behavior. |
| `probes/module-split` | done | PR 5 (#249): Split probe concerns into focused modules and preserved `sanitize_path` behavior for Unix paths, Windows-style paths, colons, and trimming. |
| `facts/model-extraction` | done | PR 6 (#251): Moved neutral fact DTOs into `analysis/facts/model.rs` while leaving syntax adapters, builders, extraction, and query logic in place. |
| `syntax/adapter-extraction` | done | PR 7: Moved syntax adapter traits and shared syntax facts into `analysis/syntax/adapter.rs` without moving builders or extraction logic yet. |
| `facts/builder-extraction` | done | PR 8 (#253): Moved index construction into `analysis/facts/build.rs` after syntax adapter type extraction. |
| `syntax/ra-extraction` | done | PR 9: Parser-backed RA syntax adapter implementation moved into `analysis/syntax/ra.rs` after build-index extraction. |
| `syntax/lexical-extraction` | done | PR 10: Lexical syntax fallback implementation moved into `analysis/syntax/lexical.rs` after RA extraction. |
| `extract/fact-extraction` | done | PR 11: Moved call, return, literal, oracle, and text extraction helpers plus probe-shape constants into `analysis/extract/*` while keeping `rust_index` as the compatibility facade. |
| `probes/family-extraction` | done | PR 12: Moved probe-family mapping, changed-line family heuristics, and delta metadata into `analysis/probes/family.rs`. |
| `probes/expectations-extraction` | done | PR 13: Moved expected sink and required oracle helpers into `analysis/probes/expectations.rs`. |
| `probes/id-extraction` | done | PR 14: Moved probe ID construction and path sanitization helpers into `analysis/probes/ids.rs`. |
| `probes/lexical-extraction` | done | PR 15: Moved lexical changed-line probe fallback helpers into `analysis/probes/lexical.rs`. |
| `probes/diff-repo-split` | done | PR 16: Confirmed diff and repo probe seeding already live in `analysis/probes/diff.rs` and `analysis/probes/repo.rs` after the probe module split and helper extractions. |
| `classify/context-extraction` | done | PR 17: Created `analysis/classify/context.rs` with `ProbeContext` as the shared classifier input for later stage extraction. |
| `classify/related-tests` | done | PR 18: Moved related-test discovery into `analysis/classify/related_tests.rs` after `ProbeContext` extraction. |
| `classify/reach-stage` | ready | PR 19: Move reach evidence into stage module |
| `classify/flow-propagation` | pending | PR 20: Move flow and propagation stages |
| `classify/activation-stage` | pending | PR 21: Move activation stage |
| `classify/remaining-stages` | pending | PR 22: Move infection, reveal, decision, confidence, missing, stop reasons |
| `app/usecase-split` | pending | PR 23: Split `app.rs` into use-case modules (check, explain, context) |
| `output/format-extraction` | pending | PR 24: Move `OutputFormat` to `output/format.rs` |
| `output/render-dispatch` | pending | PR 25: Move rendering logic to `output/render.rs` |
| `cli/command-model` | pending | PR 26: Create `cli/command.rs` with `CliCommand` enum |
| `cli/parse-command` | pending | PR 27: Update `cli/parse.rs` to return `CliCommand` |
| `cli/execute-command` | pending | PR 28: Create `cli/execute.rs` for command execution |
| `domain/context-packet-dto` | pending | PR 28: Create `domain/context_packet.rs` with `ContextPacket` struct |
| `output/json-context-dto` | pending | PR 29: Update JSON context renderer to use `ContextPacket` |
| `lsp/context-packet-usage` | pending | PR 30: Update LSP to use `ContextPacket` |
| `api/doc-hidden-internals` | pending | PR 31: Mark internal modules `#[doc(hidden)]` |
| `api/private-internals` | pending | PR 32: Make internal modules private (breaking, optional) |
| `xtask/command-dispatch` | pending | PR 33: Split xtask into command and run modules |
| `xtask/policy-modules` | pending | PR 34: Organize policy checks into `xtask/src/policy/` |
| `xtask/report-modules` | pending | PR 35: Organize reports into `xtask/src/reports/` |
| `campaign/modularization-closeout` | pending | Final review and closure of Campaign 6 |

Stack audit:

The Campaign 6 draft PRs were opened before Campaign 5B config, SARIF, badge,
and saved-workspace LSP cockpit work landed. Audit snapshot: 2026-05-06 against
`main` at `e2648b6`.

| PR | Branch | Current base | GitHub state | Disposition |
| --- | --- | --- | --- | --- |
| #244 | `claude/c6-01-analysis-summary-extraction` | `main` | draft, conflicting | Keep as the canonical first refactor, but rebase onto current `main`; preserve the summary/sort extraction only and remove `.ripr/no-panic-allowlist.toml` churn unless focused tests still need it. |
| #245 | `claude/c6-02-analysis-pipeline-extraction` | `main` | draft, conflicting | Keep after #244; rebase on the merged summary extraction so `analysis/mod.rs` becomes a thin facade without changing analyzer behavior. |
| #246 | `claude/c6-03-diff-module-split` | `main` | draft, conflicting | Keep after #245; rebase and restrict the diff split to `diff/{mod,model,load,parse}.rs`. Any `#[allow(unused_imports)]` re-export must stay narrow and documented, and policy allowlist changes need explicit justification. |
| #247 | `claude/c6-04-workspace-module-split` | `main` | draft, conflicting | Keep after #246; rebase and preserve current analysis-mode scope semantics for `instant`, `draft` / `fast`, `deep` / `ready`, and `--no-unchanged-tests`. |
| #249 | `claude/c6-05-probes-module-split` | `main` | draft, mergeable but unstable | Keep after #247 and before #251. Rebase onto the workspace split, confirm `sanitize_path` still replaces `/`, `\`, and `:` with `_`, trims leading/trailing underscores, keeps the Unix, Windows-style, and trimming tests, and resolve the stale review thread before validation. |
| #251 | `claude/c6-05-facts-model-extraction` | `claude/c6-04-workspace-module-split` | draft, stacked | Keep as the canonical facts model extraction after #249 lands or is deliberately skipped; rebase through the stack and keep syntax adapters, builders, extractors, and query logic out of the facts model PR. |
| new PR 6 | `claude/c6-06-syntax-adapter-type-extraction` exists without an open PR | #251 successor | branch-only | Open or recreate this as the missing syntax-adapter extraction after #251; it must establish the `analysis/syntax` seam before #253 moves index building. |
| #253 | `claude/c6-07-index-builder-extraction` | `claude/c6-06-syntax-adapter-type-extraction` | draft, stacked | Hold until the missing PR 6 base exists and merges; then rebase and keep the PR scoped to `build_index` movement into `analysis/facts/build.rs`. |
| #250 | `claude/c6-06-rust-index-module-split` | `main` | draft, conflicting | Do not repair as-is if #251 remains canonical. It overlaps facts-model extraction; close or rewrite later, salvaging only useful tests or notes. |

Canonical merge path:

```text
#244 -> #245 -> #246 -> #247 -> #249 -> #251 -> new PR 6 syntax-adapter extraction -> #253
```

Hold or rewrite path:

```text
#250: close or rewrite if #251 remains the facts-model path
```

Per-refactor acceptance bar:

```text
- move code only
- preserve behavior
- add focused seam tests for the moved boundary
- no output drift
- no public API drift
- no schema drift
- no analyzer semantic changes
```

Required gates for each refactor PR:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask check-public-api
cargo xtask check-architecture
cargo xtask check-output-contracts
cargo test --workspace
git diff --check
```

Dependencies:

- Phase 1 (summary, pipeline) establishes the extraction pattern and should merge before Phase 2
- Phases 2–5 (analysis breakdown) should follow the audited stack order until
  the draft stack is retired
- Phase 6–7 (app/CLI split) should follow analysis stabilization
- Phase 8–9 (API tightening) should follow all internal movement
- Phase 10 (xtask) is lowest-priority and can happen any time after Phase 1
- LSP, SARIF, and badge surfaces are frozen except defect fixes while Campaign 6
  structural refactors are in flight

Commands:

```bash
cargo fmt --check
cargo test --workspace
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-architecture
cargo xtask check-public-api
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask dogfood
```

Blocking conditions:

- Output or golden drift without intentional spec/test evidence
- Architecture guard or public API guard fails
- PR mixes multiple phases or responsibilities
- JSON schema change without new version docs
- Static language constraints violated

Review policy:

Each modularization PR should be a pure movement with zero behavior change. Include
a production-delta summary noting which responsibilities moved to which modules. No
refactoring or cleanup in the same PR. Include the standard acceptance checklist in
the PR template.
