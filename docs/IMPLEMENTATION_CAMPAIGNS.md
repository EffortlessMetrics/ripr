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
| `lsp/repo-seam-diagnostics-v1` | done | LSP publishes seam diagnostics with stable `ripr-seam-{class}` codes under the bounded saved-workspace default, with `seamDiagnostics: false` available as an explicit initialization option override. WARNING for `weakly_gripped`/`ungripped`/`reachable_unrevealed`; INFORMATION for the four `*_unknown` classes and `opaque`. `strongly_gripped`/`intentional`/`suppressed` produce no diagnostic. Diagnostic data carries `seam_id` for hover lookup. |
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
adds multi-second editor latency before the cache/defaults-first work,
related-test fanout is broad, many seams classify as
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
- LSP code actions surface inspect-seam, write-targeted-test,
  open-related-test, and refresh-analysis actions for diagnostics that carry
  `seam_id`; no automatic edits
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
| `calibration/cargo-mutants-v1` | done | Adds advisory `cargo xtask mutation-calibration` report generation and public `ripr calibrate cargo-mutants` import. Imported cargo-mutants JSON/output is joined to static `SeamGripClass` evidence by `seam_id` first and unambiguous normalized file/line second; span-based locations are imported, ambiguous file/line candidates stay unassigned, and unmatched runtime mutants remain visible; runtime mutation vocabulary stays inside calibration reports. |
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
- default-on seam diagnostics without the repo-seam cache and bounded
  saved-workspace defaults

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

Status: complete

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
| `campaign/modularization-stack-audit` | done | Audited the old Campaign 6 draft stack against current `main` after Campaign 5B closeout. That audit was the starting snapshot; the final landed chain replaced the stale #251/#253 path with current-base PRs and closed the parked forks. |
| `modularization/infrastructure-and-planning` | done | Documentation, campaign outline, first-PR pattern, and the post-5B stack audit exist; implementation resumes with the canonical stack order below. |
| `analysis/summary-extraction` | done | PR 1 (#244): Extracted duplicated summary and sort logic from `analysis/mod.rs` into focused helper modules with no output/API/schema drift. |
| `analysis/pipeline-extraction` | done | PR 2 (#245): Extracted diff and repo pipeline orchestration into `analysis/pipeline.rs` while preserving `run_analysis` and `run_repo_analysis` as stable facades. |
| `diff/module-split` | done | PR 3 (#246): Split `analysis/diff.rs` into `diff/{mod,model,load,parse}.rs` with the parser and git-diff adapter behavior preserved. |
| `workspace/module-split` | done | PR 4 (#247): Split workspace concerns into focused modules without changing workspace selection behavior. |
| `probes/module-split` | done | PR 5 (#249): Split probe concerns into focused modules and preserved `sanitize_path` behavior for Unix paths, Windows-style paths, colons, and trimming. |
| `facts/model-extraction` | done | PR 6 (#354): Moved neutral fact DTOs into `analysis/facts/model.rs` while leaving syntax adapters, builders, extraction, and query logic in place. |
| `syntax/adapter-extraction` | done | PR 7 (#357): Moved syntax adapter traits and shared syntax facts into `analysis/syntax/adapter.rs` without moving builders or extraction logic yet. |
| `facts/builder-extraction` | done | PR 8 (#359): Moved index construction into `analysis/facts/build.rs` after syntax adapter type extraction. |
| `syntax/ra-extraction` | done | PR 9 (#361): Parser-backed RA syntax adapter implementation moved into `analysis/syntax/ra.rs` after build-index extraction. |
| `syntax/lexical-extraction` | done | PR 10 (#367): Lexical syntax fallback implementation moved into `analysis/syntax/lexical.rs` after RA extraction. |
| `extract/fact-extraction` | done | PR 11 (#369): Moved call, return, literal, oracle, and text extraction helpers plus probe-shape constants into `analysis/extract/*` while keeping `rust_index` as the compatibility facade. |
| `probes/family-extraction` | done | PR 12 (#370): Moved probe-family mapping, changed-line family heuristics, and delta metadata into `analysis/probes/family.rs`. |
| `probes/expectations-extraction` | done | PR 13 (#371): Moved expected sink and required oracle helpers into `analysis/probes/expectations.rs`. |
| `probes/id-extraction` | done | PR 14 (#372): Moved probe ID construction and path sanitization helpers into `analysis/probes/ids.rs`. |
| `probes/lexical-extraction` | done | PR 15 (#373): Moved lexical changed-line probe fallback helpers into `analysis/probes/lexical.rs`. |
| `probes/diff-repo-split` | done | PR 16 (#376): Confirmed diff and repo probe seeding already live in `analysis/probes/diff.rs` and `analysis/probes/repo.rs` after the probe module split and helper extractions. |
| `classify/context-extraction` | done | PR 17 (#377): Created `analysis/classify/context.rs` with `ProbeContext` as the shared classifier input for later stage extraction. |
| `classify/related-tests` | done | PR 18 (#379): Moved related-test discovery into `analysis/classify/related_tests.rs` while preserving classification behavior. |
| `classify/reach-stage` | done | PR 19 (#380): Moved reach evidence into `analysis/classify/reach.rs` while preserving classification behavior. |
| `classify/flow-propagation` | done | PR 20 (#381): Moved local flow and propagation evidence into `analysis/classify/flow.rs` while preserving classification behavior. |
| `classify/activation-stage` | done | PR 21 (#383): Moved activation evidence into `analysis/classify/activation.rs` while preserving classification behavior. |
| `classify/remaining-stages` | done | PR 22 (#385): Moved infection, reveal, decision, confidence, missing, stop reasons, and next-step helpers into focused `analysis/classify` modules while preserving classification behavior. |
| `app/usecase-split` | done | PR 23 (#387): Split check, explain, and context use-case orchestration into focused `app` modules while preserving public API, CLI, LSP, output, and schema behavior. |
| `output/format-extraction` | done | PR 24 (#388): Moved `OutputFormat` to `output/format.rs` while preserving the `app::OutputFormat` public path. |
| `output/render-dispatch` | done | PR 25 (#390): Moved `render_check` dispatch into `output/render.rs` while preserving the `app::render_check` public facade. |
| `cli/command-model` | done | PR 26 (#391): Created `cli/command.rs` with a focused `CliCommand` enum while preserving top-level CLI dispatch behavior. |
| `cli/parse-command` | done | PR 27 (#392): Updated `cli/parse.rs` to return the parsed command shape while preserving argument behavior. |
| `cli/execute-command` | done | PR 28 (#394): Created `cli/execute.rs` for command execution while preserving argument and handler behavior. |
| `domain/context-packet-dto` | done | PR 29 (#397): Created `domain/context_packet.rs` with the context packet DTO shape. |
| `output/json-context-dto` | done | PR 30 (#398): Updated JSON context renderer to use `ContextPacket` without changing packet output. |
| `lsp/context-packet-usage` | done | PR 31 (#399): Updated LSP context packet lookup to use `ContextPacket` while preserving packet output. |
| `api/doc-hidden-internals` | done | PR 32 (#400): Marked compatibility module exports `#[doc(hidden)]` while preserving public API paths. |
| `api/private-internals` | blocked | PR 33: Make internal modules private (breaking, optional) |
| `xtask/command-dispatch` | done | PR 34 (#401): Split xtask into command and run modules. |
| `xtask/policy-modules` | done | PR 35 (#403): Organize policy checks into `xtask/src/policy/`. |
| `xtask/report-modules` | done | PR 36 (#405): Organize reports into `xtask/src/reports/`. |
| `campaign/modularization-closeout` | done | Final review closed Campaign 6, confirmed stale forks #250, #253, and #352 are closed unmerged, and moved the active manifest to Campaign 7 defaults-first operator adoption. |

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

Closeout:

Campaign 6 is complete. Landed PR chain:

- #347 `campaign/modularization-stack-audit`
- #244 `analysis/summary-extraction`
- #245 `analysis/pipeline-extraction`
- #246 `diff/module-split`
- #247 `workspace/module-split`
- #249 `probes/module-split`
- #354 `facts/model-extraction`
- #357 `syntax/adapter-extraction`
- #359 `facts/builder-extraction`
- #361 `syntax/ra-extraction`
- #367 `syntax/lexical-extraction`
- #369 `extract/fact-extraction`
- #370 `probes/family-extraction`
- #371 `probes/expectations-extraction`
- #372 `probes/id-extraction`
- #373 `probes/lexical-extraction`
- #376 `probes/diff-repo-split`
- #377 `classify/context-extraction`
- #379 `classify/related-tests`
- #380 `classify/reach-stage`
- #381 `classify/flow-propagation`
- #383 `classify/activation-stage`
- #385 `classify/remaining-stages`
- #387 `app/usecase-split`
- #388 `output/format-extraction`
- #390 `output/render-dispatch`
- #391 `cli/command-model`
- #392 `cli/parse-command`
- #394 `cli/execute-command`
- #397 `domain/context-packet-dto`
- #398 `output/json-context-dto`
- #399 `lsp/context-packet-usage`
- #400 `api/doc-hidden-internals`
- #401 `xtask/command-dispatch`
- #403 `xtask/policy-modules`
- #405 `xtask/report-modules`

Stale fork disposition at closeout:

- #250 closed unmerged as the old `rust_index.rs` module-directory fork.
- #253 closed unmerged as the old stacked build-index PR; #359 is the landed current-base replacement.
- #352 closed unmerged as the old draft PR #10 extractor modularization branch.
- #351 remains a separate policy lane, not Campaign 6 closeout work.

`api/private-internals` remains explicitly blocked because making compatibility
module exports private is a breaking public API decision, not required for the
Campaign 6 internal SRP boundary. The saved-workspace LSP cockpit contract stayed
green through every analyzer-affecting refactor; post-merge proof for the final
xtask report seam passed on `main` at `72ee398`.

The active campaign now moves to Campaign 7. Operator adoption work should build
on the modularized internals without adding speculative LSP features.

## Campaign 7: Defaults-First Operator Adoption

Campaign ID: `defaults-first-operator-adoption`

Status: done

Objective:

```text
Make ripr useful from a clean install by giving CLI, editor, and CI users one
defaults-first path from static seam evidence to a targeted-test action and a
receipt that shows the seam improved.
```

Why it matters:

The core product surfaces now exist: repo exposure reports, seam-native badges,
SARIF, LSP diagnostics/hovers/actions, targeted-test briefs, targeted-test
outcome receipts, and mutation calibration import. Adoption now depends on a
clear operator loop more than additional analyzer structure.

End state:

- built-in defaults and generated `ripr.toml` are documented and conservative
- fast, normal, and deep mode behavior is clear without hand tuning
- one operator cockpit joins the existing report surfaces into next action
- GitHub Actions use a copyable workflow with artifacts and optional SARIF rendering/upload
- editor install and command docs cover the existing saved-workspace loop only
- example corpus demonstrates the targeted-test loop and optional calibration
- install/release paths are verified enough for a new user to run the loop

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `defaults/config-init` | done | Built-in defaults, generated `ripr.toml`, repo-mode exclusions, seam-diagnostic policy, badge/report defaults, and fast/normal/deep mode behavior are documented and test-pinned without output schema or LSP drift. |
| `reports/operator-cockpit` | done | `cargo xtask operator-cockpit` writes `target/ripr/reports/operator-cockpit.{json,md}` by joining existing repo exposure, LSP cockpit, SARIF policy, badge status, targeted-test outcome, and optional mutation calibration artifacts into one next-action surface. `operator-cockpit-report` remains an alias for existing automation. Missing inputs stay visible with generator commands; top weak seams carry why-it-matters text, a suggested targeted test, and best related-test context when available. The command does not rerun analysis or change static classifications. |
| `ci/github-action-entrypoint` | done | `ripr init --ci github` generates the copyable defaults-first GitHub Action entrypoint. It runs `ripr pilot`, renders diff/repo SARIF only when `RIPR_UPLOAD_SARIF` is true, writes repo badge JSON and Shields artifacts, uploads the pilot/report directories, and keeps the job plus upload steps advisory. |
| `editor/install-polish` | done | Documented the normal VS Code/Open VSX install path, server-resolution fallback, local VSIX smoke path, saved-workspace default, and existing command coverage. The docs now reflect the current e2e coverage for command registration, draft-mode defaults, LSP-first seam context, targeted-test brief copying, suggested assertions, related-test opening, malformed argument handling, and restart behavior without adding editor features. |
| `fixtures/example-corpus` | done | Added `fixtures/EXAMPLE_CORPUS.md`, the `opaque_fixture_builder` executable fixture, checked boundary-gap before/after repo-exposure snapshots, targeted-test outcome receipts, and optional mutation-calibration reports. The corpus maps boundary gap, missing equality boundary, weak oracle, exact error variant, opaque fixture/builder, LSP actions, CLI goldens, receipts, and calibration artifacts. |
| `release/install-polish` | done | Verified crate package listing, publish dry-run, local `cargo install` smoke, VSIX packaging, public `v0.3.0` GitHub Release server manifest/assets, Windows server archive checksum, and extracted server CLI/LSP smoke; `0.3.1` is prepared as the first public install line that includes `ripr pilot` and `ripr outcome`. |
| `campaign/defaults-first-closeout` | done | Closed Campaign 7 after #409 through #417 landed. The closeout audit is recorded in `docs/handoffs/2026-05-07-campaign-7-closeout.md`; the installed binary ran the boundary-gap seam packet, outcome receipt, and optional calibration loop, and the active manifest now points to Campaign 8 runtime calibration fixtures. |

Dependencies:

- `defaults/config-init` landed first so every later surface can use the same
  default profile and mode vocabulary.
- `reports/operator-cockpit` landed before GitHub Action and example-corpus
  work so the CI and demo paths have one canonical next-action artifact.
- `ci/github-action-entrypoint` landed before editor install polish so the
  public CI path already uploads the same pilot/report artifacts the editor
  docs can point reviewers toward.
- `editor/install-polish` should remain documentation/verification unless a
  regression appears in the existing saved-workspace contract.
- `fixtures/example-corpus` follows editor install polish so the public examples
  can point to the documented editor and CI adoption paths.
- `release/install-polish` follows the example corpus so install and release
  proof can exercise the same public operator loop.
- `campaign/defaults-first-closeout` follows release/install proof so the final
  review can validate a complete install-to-targeted-test loop instead of
  approving individual surfaces in isolation.

Commands:

```bash
cargo package -p ripr --list
cargo publish -p ripr --dry-run
npm --prefix editors/vscode run package
cargo xtask check-pr
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask check-traceability
cargo xtask check-capabilities
cargo test --workspace
```

Blocking conditions:

- new LSP feature work instead of preserving the existing saved-workspace loop
- output schema drift without a versioned spec update
- default policy that makes CI blocking by surprise
- broad examples that do not prove the targeted-test loop
- install instructions that require `cargo install ripr` for the normal editor path

Landed PR chain:

- #409 `vscode: default editor analysis to draft`
- #410 `campaign: pin defaults config baseline`
- #411 `test: pin defaults mode and repo filters`
- #412 `campaign: add operator cockpit report`
- #413 `ci: add defaults-first GitHub Action entrypoint`
- #414 `ci: gate generated SARIF rendering`
- #415 `vscode: document and verify install polish`
- #416 `fixtures: add defaults-first example corpus`
- #417 `docs: verify release install paths`

The active campaign now moves to Campaign 8. Calibration fixture work should
keep runtime mutation data as explicit supplied input and must not make RIPR run
mutation tests.

## Campaign 8: Runtime Calibration Fixture Expansion

Campaign ID: `runtime-calibration-fixtures`

Status: done

Objective:

```text
Expand the calibration fixture lane so RIPR can compare static test-grip
evidence with supplied cargo-mutants results across representative agreement
buckets without turning RIPR into a mutation runner.
```

Why it matters:

Campaign 7 made the operator loop usable from install to targeted-test receipt.
The next credibility gap is calibration breadth: one boundary-gap sample proves
the path, but not the range of static/runtime agreement buckets users will see
when importing cargo-mutants data from real repositories.

End state:

- calibration fixtures cover static gaps with runtime signals and static gaps
  without runtime signals
- calibration fixtures cover runtime signals without static gaps, ambiguous
  file/line joins, and unmatched runtime data
- every runtime artifact is supplied input or generated calibration output
- operator cockpit and docs show calibration as optional advisory context
- static output vocabulary remains unchanged outside explicit calibration reports

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `calibration/runtime-fixtures-v1` | done | Added `fixtures/boundary_gap/calibration/runtime-fixtures-v1/` with supplied repo-exposure and cargo-mutants JSON inputs plus checked Markdown/JSON reports. `crates/ripr/tests/cli_smoke.rs::calibration_runtime_fixture_matches_checked_reports` verifies the public command output against those reports and pins the main static/runtime buckets, ambiguous file/line joins, unmatched runtime data, static seams without runtime data, and `seam_id`/`file_line` joins. |
| `campaign/runtime-calibration-closeout` | done | Closed Campaign 8 after the fixture-backed calibration lane was reviewed, post-merge proof passed on `main`, and manifests moved to Campaign 9 hot-sidecar latency proof. Runtime calibration remains optional supplied-data context; RIPR still does not run mutation tests. |

Commands:

```bash
cargo test -p ripr calibration
cargo xtask mutation-calibration fixtures/boundary_gap/input --mutants-json fixtures/boundary_gap/calibration/runtime-fixtures-v1/runtime-mutants.json --repo-exposure-json fixtures/boundary_gap/calibration/runtime-fixtures-v1/repo-exposure.json
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-fixture-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo test --workspace
```

Blocking conditions:

- adding runtime mutation execution to RIPR
- changing static classifications to match a runtime sample
- using runtime outcome vocabulary outside explicit calibration reports
- making calibration required for the default pilot, LSP, SARIF, or badge paths

Landed PR chain:

- #420 `fixtures: add runtime calibration agreement sample`
- `campaign/runtime-calibration-closeout`

The active campaign now moves to Campaign 9. Hot-sidecar work should start with
measurement of current cache and editor refresh behavior before changing cache
semantics.

## Campaign 9: Hot Sidecar Latency Proof

Campaign ID: `hot-sidecar-latency`

Status: done

Objective:

```text
Make the editor and operator paths faster without broadening the analyzer or LSP
surface by measuring current cache and refresh behavior first, then tightening
warm-path reuse only where there is evidence.
```

Why it matters:

Campaign 5A shipped the first repo seam fact cache, and Campaign 7 made the
saved-workspace editor/operator loop usable. The next product risk is latency:
large workspaces and repeated editor refreshes need proof that warm paths stay
fast without serving stale seam evidence.

End state:

- current repo seam cache behavior and saved-workspace LSP refresh latency are
  measured from existing commands
- any hot-path cache change preserves output schemas, static vocabulary, public
  API, SARIF, badges, and saved-workspace LSP cockpit behavior
- rendered outputs remain uncached; only fact layers or in-memory indexes may be
  reused
- large-repo and editor latency decisions are backed by reports, not speculative
  storage

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `cache/current-latency-audit` | done | Measured the current proof surfaces without behavior changes. Unit-level seam cache and seam inventory tests, LSP tests, `lsp-cockpit-report`, and `operator-cockpit` were cheap on a warm local build. LSP cockpit stayed green with the boundary-gap fixture and all contributed VS Code commands covered. Operator cockpit generated quickly and correctly surfaced missing required report inputs when only LSP and optional calibration reports were present. A direct `cargo xtask repo-exposure-report` audit did not finish within a 20-minute local timeout, so the next work should add bounded latency visibility before any cache rewrite. |
| `cache/repo-exposure-latency-report` | done | Added `cargo xtask repo-exposure-latency-report`, which builds the local debug `ripr` binary, runs `repo-exposure-json` under a bounded timeout, captures opt-in analyzer trace lines from stderr, skips Markdown after a JSON timeout, and writes `target/ripr/reports/repo-exposure-latency.{json,md}`. The report observes cache collection, cache load hit/miss/corrupt state, cold compute, cache store, and total phase timing without changing repo-exposure JSON/Markdown, LSP, SARIF, badge, or public API behavior. |
| `cache/repo-exposure-warm-path-reuse` | done | Added a repo file-fact cache under `target/ripr/cache/repo-file-facts/0.1`, changed repo-exposure cold compute to build its index from already-collected workspace bytes, and reused precomputed related-test context plus seam-independent value-resolution facts during full repo evidence construction. The latency report now exposes `file_fact_cache` counters and cold sub-phases through classification. Local evidence showed `file_fact_cache` moving from `hits_0_misses_134` at about 3065 ms to `hits_134_misses_0` at about 328 ms, and after a long bounded run populated the classified-seam cache, the default 30-second latency report passed on cache hits. |
| `pilot/budget-aware` | done | Added a default 30 second `ripr pilot` analysis budget plus `--timeout-ms`. Complete runs keep writing repo exposure, agent seam packets, and summary artifacts with `pilot-summary.json` schema `0.2`; timeout runs write `pilot-summary.{json,md}` with `status: partial`, `reason: timeout`, `outputs_written`, and a retry command instead of waiting silently. |
| `pilot/first-screen-clarity` | done | Improved `pilot-summary.md` and terminal copy so the top recommendation answers what was inspected, why the seam matters, what focused test to write, and what command to run after without opening JSON. The complete-run JSON schema remains `0.2`; only human-facing Markdown/terminal copy changed. |
| `cache/evidence-latency-progress` | done | Closeout proof found that the bounded repo-exposure latency report can still time out after `inventory_seams`, even with file-fact cache hits, without identifying how far evidence construction progressed. Added trace-only progress lines inside `evidence_for_seams`; this changes only opt-in latency stderr/report diagnostics and does not change analyzer outputs, schemas, LSP, SARIF, badges, or public API. |
| `cache/evidence-hot-path-indexes` | done | Replaced the per-seam full test scan with indexed related-test candidate lookup, built value-resolution facts lazily per related test, and used an owned classification path in repo inventory to avoid cloning full evidence records. A long bounded cold run completed and stored the classified-seam cache; the following default 30-second latency report passed on JSON and Markdown cache hits. No analyzer output, schema, LSP, SARIF, badge, or public API changes are intended. |
| `campaign/hot-sidecar-latency-closeout` | done | Closed Campaign 9 after latency measurement, file-fact warm reuse, evidence hot-path indexing, bounded pilot behavior, first-screen pilot clarity, and post-merge saved-workspace LSP proof landed. Current-main proof showed the first cold default repo-exposure latency run can still exceed 30 seconds until the classified-seam cache is filled; a 120-second bounded cold run completed, stored the cache, and the following default 30-second JSON/Markdown latency report passed on cache hits. |

Commands:

```bash
cargo test -p ripr analysis::seam_cache --lib
cargo test -p ripr analysis::seam_inventory --lib
cargo test -p ripr lsp
cargo test -p ripr lsp::tests
cargo xtask lsp-cockpit-report
cargo xtask repo-exposure-latency-report
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-pr
cargo test --workspace
```

Blocking conditions:

- new LSP features instead of preserving the saved-workspace contract
- caching rendered JSON, Markdown, diagnostics, hover text, or agent packets
- stale test, config, intent, or suppression data surviving a warm path
- output/schema/public API/SARIF/badge drift without an explicit spec update

Audit notes:

- Warm local command timings for `analysis::seam_cache`, `analysis::seam_inventory`,
  `lsp`, `lsp::tests`, `lsp-cockpit-report`, and `operator-cockpit` were all
  sub-second on Windows after the build was already warm. These are smoke
  measurements, not benchmark claims.
- `target/ripr/reports/lsp-cockpit.{json,md}` reported `pass`, one boundary-gap
  seam diagnostic, all existing seam actions, and no uncovered contributed VS
  Code commands.
- `target/ripr/reports/operator-cockpit.{json,md}` generated quickly but warned
  because repo exposure, SARIF policy, badge status, and targeted-test outcome
  reports had not been generated in that target directory. This is current
  expected behavior: the cockpit joins existing reports and does not rerun
  analysis.
- A direct `cargo xtask repo-exposure-report` audit did not complete within a
  20-minute local timeout on this workspace. The spawned `ripr.exe` process was
  stopped after the timeout. Treat this as the first Campaign 9 finding: before
  optimizing cache internals, add bounded repo-exposure latency visibility that
  reports phase timing and cache hit/miss state.
- `cargo xtask repo-exposure-latency-report` now provides that bounded surface.
  A local 2-second smoke run and the default 30-second run both timed out in
  `repo-exposure-json`; the trace reported `collect_workspace_state` as fast
  and observed a repo seam fact cache miss before entering cold compute. That
  makes the next optimization target concrete without changing analyzer results
  or output schemas.
- `cache/repo-exposure-warm-path-reuse` added file-fact cache reuse below the
  workspace classified-seam cache. The first local latency run populated 134
  file-fact entries and reported `file_fact_cache` at about 3065 ms; the next
  run reported 134 hits and about 328 ms for that phase. Full repo evidence
  also now reuses per-test related and value facts. After one long bounded run
  populated the classified-seam cache, the default 30-second latency report
  passed on both JSON and Markdown cache-hit runs. `pilot/first-screen-clarity`
  then made the pilot Markdown and terminal first screen spell out the inspected
  seam, why it matters, the focused test to write, and the before/after command
  pair.
- The first `campaign/hot-sidecar-latency-closeout` proof attempt found
  bounded repo-exposure latency still timing out after `inventory_seams` on the
  current repo. `cache/evidence-latency-progress` added trace-only progress
  markers inside evidence construction so the latency report can show whether
  future timeouts are stuck before context build, during per-seam evidence, or
  after evidence classification.
- `cache/evidence-hot-path-indexes` followed that trace. It moved evidence
  candidate discovery from per-seam full test scans to precomputed candidate
  indexes, made value-resolution facts lazy, and classified owned seam/evidence
  vectors on the repo inventory path. Local proof: a 120-second cold latency
  run passed and stored the classified-seam cache; the next default 30-second
  latency report passed with `repo-exposure-json` and `repo-exposure-md` cache
  hits at about 12 seconds each.
- `campaign/hot-sidecar-latency-closeout` reran proof on current `main` after
  the concurrent agent-brief and clippy-policy PRs had merged below the final
  cache PR. `cargo test -p ripr lsp`, `cargo test -p ripr lsp::tests`, and
  `cargo xtask lsp-cockpit-report` passed. The first default 30-second
  `repo-exposure-latency-report` run was a cache miss and timed out during
  evidence construction; a 120-second bounded run completed cold compute in
  about 33 seconds, stored the classified-seam cache, and the following default
  30-second report passed on cache hits (`repo-exposure-json` about 14.6
  seconds, `repo-exposure-md` about 13.4 seconds). Campaign 9 is closed and the
  active manifest now moves to Campaign 10 editor-agent integration.

Landed PR chain:

- #422 `campaign: record hot sidecar latency audit`
- #423 `cache: add repo exposure latency report`
- #431 `cache: reuse warm repo exposure facts`
- #436 `cli: make pilot budget-aware`
- #437 `cache: reuse repo exposure warm path facts`
- #448 `pilot: clarify first-screen recommendation`
- #450 `cache: trace repo exposure evidence progress`
- #451 `cache: index repo evidence hot path`
- #454 `campaign: close hot sidecar latency proof`

## Campaign 10: Editor Agent Integration

Campaign ID: `editor-agent-integration`

Status: done

Objective:

```text
Make the saved-workspace editor loop and the agent CLI loop line up:
diagnostic -> evidence -> packet or brief -> targeted test -> verify -> receipt
-> cockpit and CI artifacts.
```

Why it matters:

Campaigns 4B, 7, 8, and 9 made the major product pieces real:
saved-workspace seam diagnostics, hovers, copyable packets and briefs, repo
exposure, operator cockpit, advisory CI, badge artifacts, calibration imports,
and a bounded pilot path. #457 and #458 added `ripr agent verify` and
`ripr agent receipt`. The next product risk is not another analyzer capability;
it is that users and agents still have to stitch the editor, CLI, receipt,
cockpit, and CI surfaces together by hand.

#463 briefly changed the active lane to `release-surface-0-4`. This campaign
keeps the active product lane as editor-agent integration and carries the useful
release-readiness requirements as `release/editor-agent-readiness-proof` before
closeout.

End state:

- a saved-workspace seam diagnostic exposes the same evidence and next commands
  as the agent CLI path
- users can copy the agent packet or brief, after-snapshot command, verify
  command, and receipt command without automatic edits
- `operator-cockpit` joins existing before/after, verify, receipt, SARIF, badge,
  LSP, and optional calibration reports without rerunning analysis
- one fixture pins the full editor-agent loop from LSP expectations through
  agent packet, verify, receipt, and cockpit output
- generated CI uploads the editor-agent artifacts as visible non-blocking
  evidence first
- installed CLI, packaged VSIX, package dry-run, and known-limits proof cover
  the loop before closeout

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `editor-agent/integration-contract-audit` | done | Define the editor-agent integration contract and inventory current CLI, LSP, VS Code, agent, receipt, cockpit, CI, fixture, install, and release-readiness surfaces. Docs/manifest only. |
| `lsp/agent-loop-copy-commands` | done | Seam diagnostics expose command-oriented copy actions for agent packet, brief, after-snapshot, verify, and receipt commands. The actions are pinned in the boundary-gap LSP fixture and VS Code command registration coverage without automatic edits, CodeLens, inlay hints, semantic tokens, or unsaved-buffer overlays. |
| `operator/verify-receipt-status` | done | `operator-cockpit` now reports the editor-agent before snapshot, after snapshot, agent verify JSON, and agent receipt JSON as required inputs. Missing inputs include next commands that match the saved-workspace editor command chain, and present agent verify artifacts summarize improved, changed, regressed, and unchanged counts without rerunning analysis. |
| `fixtures/editor-agent-loop` | done | Boundary-gap now has a checked `expected/editor-agent-loop/` packet that pins LSP diagnostics/actions through agent packet, agent brief, agent verify, agent receipt, and operator cockpit output. The fixture also pins host-independent agent packet paths. |
| `ci/editor-agent-artifacts` | done | The generated GitHub workflow now uploads the non-blocking editor-agent loop artifacts: pilot summary, repo exposure, agent packet, agent brief, agent verify, agent receipt, targeted-test outcome, optional operator cockpit when the repo-local xtask exists, SARIF when enabled, and badge JSON. |
| `docs/full-evidence-loop` | done | Quickstart and installed-user docs now lead with the real diagnostic-to-receipt loop: `ripr pilot`, targeted brief, focused test, after snapshot, `ripr outcome`, `ripr agent verify`, `ripr agent receipt`, editor actions, generated CI artifacts, and known limits. They state that `ripr init` materializes optional repo policy rather than activating the useful default path. |
| `release/editor-agent-readiness-proof` | done | `release-readiness --version 0.4.0` now proves the installed CLI command surface, boundary-gap `pilot`, `outcome`, `agent verify`, focused `agent receipt`, repo-exposure latency, LSP cockpit, advisory workflow defaults, VSIX packaging path, and known-limit docs. Package and publish gates remain explicit release-prep checks until the version bump. |
| `campaign/editor-agent-integration-closeout` | done | Closed Campaign 10 after editor, agent, cockpit, CI, fixture, docs, and release-readiness proof aligned with no new public crates, runtime execution, automatic edits, or speculative editor features. |

Closeout:

- The editor and agent paths now share one evidence chain:
  saved-workspace diagnostic -> evidence -> packet or brief -> focused test ->
  after snapshot -> `ripr outcome` -> `ripr agent verify` ->
  `ripr agent receipt` -> cockpit and CI artifacts.
- The generated GitHub workflow uploads the non-blocking editor-agent artifact
  set without running mutation testing or enabling CI blocking by default.
- `cargo xtask release-readiness --version 0.4.0` proves the installed command
  surface, boundary-gap pilot/outcome/verify/receipt fixtures, repo-exposure
  latency, LSP cockpit, advisory workflow defaults, VSIX path, and known-limit
  docs. Package and publish gates remain explicit release-prep checks until the
  version bump.
- No new analyzer family, LSP feature expansion, unsaved-buffer overlay,
  automatic edit, runtime mutation execution, CI blocking policy, public crate
  split, or SARIF/badge schema change shipped in this campaign.

Commands:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
```

Blocking conditions:

- new analyzer families
- LSP feature expansion
- unsaved-buffer overlays
- runtime execution
- CI blocking by default
- SARIF or badge schema churn unless explicitly versioned
- broad refactors mixed into release-readiness proof
- replacing the editor-agent integration lane without an explicit product pivot

## Campaign 11: LLM Work Loop

Campaign ID: `llm-work-loop`

Status: done

Objective:

```text
Make the completed editor-agent loop stateful, deterministic, and useful to LLM
agents under review pressure: status -> task packet -> edit target -> verify ->
receipt -> reviewer summary.
```

Why it matters:

Campaign 10 made the editor-agent loop functionally complete. The next risk is
operator drift: agents can see the commands and artifacts, but still have to
infer which step is missing, which seam links the artifacts, and what evidence
reviewers should inspect. Campaign 11 adds a read-only, artifact-oriented
control plane around the existing loop.

End state:

- agents can inspect loop state without rerunning analysis or relying on chat
  history
- loop commands and artifact paths are centralized across CLI, LSP, cockpit,
  CI, docs, fixtures, and release proof
- receipts carry provenance and bounded static next-action guidance
- a reviewer summary joins status, receipt, cockpit, repo exposure, LSP cockpit
  when present, and CI artifact state
- fixtures pin happy, unchanged, regressed, missing-artifact, stale-artifact,
  configured-off, path-with-spaces, and Windows-separator cases
- generated CI uploads LLM work-loop packets as advisory evidence

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `agent/loop-status-report` | done | Added `ripr agent status --root . --json` as a read-only artifact status report for before snapshot, after snapshot, agent brief, agent packet, agent verify, and agent receipt, with recoverable seam_id, missing-input commands, and stale-looking warnings. |
| `agent/centralize-loop-command-templates` | done | Added `crates/ripr/src/agent/loop_commands.rs` as the shared internal source for workflow, pilot, and editor-agent artifact paths plus packet, brief, snapshot, verify, receipt, status, review-summary, and outcome command templates; agent status, agent brief, pilot, LSP copy actions, generated CI paths, and operator cockpit missing-input commands now reuse it without changing emitted command text. |
| `agent/workflow-manifest` | done | Added `ripr agent start --root . --seam-id <id> --out target/ripr/workflow` to write `workflow.json`, `commands.md`, and `agent-brief.json` as a source-edit-free workflow packet with selected seam details, artifact paths, shared commands, missing inputs, and explicit no-edit/no-LLM/no-runtime-execution boundaries. |
| `agent/receipt-provenance` | done | Added agent receipt schema `0.2` provenance with ripr version, repo root, optional config fingerprint, command template version, render timestamp, before/after/verify artifact SHA-256 hashes, selected seam ID, before/after classes, movement, and explicit static-boundary flags. |
| `agent/next-action-guidance` | done | Added structured `summary.next_action` guidance to agent receipt schema `0.3` for improved, changed, regressed, unchanged, new-gap, and resolved states while preserving existing summary fields. |
| `agent/reviewer-summary` | done | Added `ripr agent review-summary --root .` Markdown plus `--json` schema `0.1` output that joins status, workflow, receipt, cockpit, repo exposure, LSP cockpit when present, and local CI artifact state into a compact review packet. |
| `fixtures/llm-work-loop` | done | Added a boundary-gap `expected/llm-work-loop/` fixture matrix for happy, unchanged, regressed, missing-artifact, stale-artifact, configured-off, path-with-spaces, and Windows-separator loop cases. |
| `ci/llm-work-packets` | done | Generated CI now writes and uploads `target/ripr/workflow` with workflow manifest, commands Markdown, agent status JSON/Markdown, agent review summary JSON/Markdown, agent packet, brief, and verify JSON, plus `target/ripr/reports/agent-receipt.json` and repo-local operator cockpit artifacts when available. Existing `target/ripr/agent` compatibility copies remain uploaded. |
| `docs/llm-operator-guide` | done | Added `docs/LLM_OPERATOR_GUIDE.md` as the source-edit-free operator guide for humans and external LLM tools, covering agent status, workflow packet, packet or brief, focused test target, after snapshot, verify, receipt, reviewer summary, CI/editor artifact paths, and explicit anti-goals. |
| `campaign/llm-work-loop-closeout` | done | Closed Campaign 11 after status, command templates, workflow manifests, receipt provenance, next-action guidance, reviewer summary, fixtures, generated CI artifacts, and the operator guide aligned around a source-edit-free static work loop. |

Closeout:

- Campaign 11 now has a deterministic, source-edit-free work loop:
  `ripr agent status` -> `ripr agent start` workflow packet -> packet or
  brief -> focused external test edit -> after snapshot -> `ripr agent verify`
  -> provenance-backed `ripr agent receipt` -> `ripr agent review-summary`.
- Command templates and artifact paths are centralized for CLI, LSP copy
  actions, operator cockpit missing-input commands, generated CI, docs, and
  fixtures.
- Receipts carry static provenance, artifact hashes, command-template version,
  static boundary flags, and bounded next-action guidance without claiming
  runtime confirmation.
- Generated CI uploads workflow status, manifests, packet/brief/verify,
  receipt, review summary, and operator cockpit artifacts as advisory evidence.
- The LLM operator guide documents how humans and external LLM tools consume the
  packet without RIPR calling models, generating tests, editing source, running
  mutation testing, or blocking CI by default.
- Campaign 12 is now the active First-Hour UX lane for making the VS Code and
  GitHub Action first screens useful without report archaeology.

Commands:

```bash
cargo test -p ripr agent_status
cargo test -p ripr agent_review_summary
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
```

Blocking conditions:

- automatic source edits
- generated tests committed by RIPR
- runtime mutation execution
- speculative LSP features
- new public crates
- command strings duplicated into new surfaces after the template
  centralization work item

## Campaign 12: First-Hour UX

Campaign ID: `first-hour-ux`

Status: complete

This campaign is intentionally separate from Campaign 11. Campaign 11 keeps the
LLM work loop stateful and deterministic through status, command templates,
workflow manifests, receipts, and reviewer summaries. Campaign 12 starts after
that control plane is stable and keeps the CLI as the shared engine while making
the first editor and CI screens useful.

Objective:

```text
Make a new user successful in the first hour through either the VS Code
extension or generated GitHub workflow, without requiring them to understand
RIPR's internal report topology.
```

Why it matters:

RIPR 0.4.0 aligned the editor, CLI, agent, cockpit, and CI evidence loop. The
next user-facing risk is translation cost: editor users still need to know why
diagnostics did not appear, which code action maps to the next focused test, and
how to verify the result; CI users still need a useful GitHub-facing summary
before downloading artifacts. Campaign 12 keeps the CLI as the shared engine and
receipt surface while making the LSP-first and CI-first paths obvious from the
surfaces users already open.

End state:

- VS Code users can see server, workspace, analysis, staleness, and diagnostic
  state without reading logs first
- editor code actions are titled around user intent: write the targeted test,
  open the best related test, copy an agent handoff, verify after the test, and
  refresh analysis
- generated GitHub workflows put the top advisory recommendation in the PR or
  step summary before artifact download is necessary
- PR test guidance annotations have a spec, JSON contract, placement rules,
  caps, and opt-in review-comment boundary before generated workflows post line
  guidance
- generated CI workflow behavior is pinned by a fixture that covers artifact
  paths, non-blocking posture, optional SARIF, badge output, and agent artifacts
- agent command templates and workflow manifests from Campaign 11 feed these UX
  surfaces instead of creating another command-string source of truth
- README and installed-user docs are organized by user type: VS Code, CI, CLI,
  agent, troubleshooting, and known limits

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/pr-test-guidance-annotations` | done | RIPR-SPEC-0012 pins the advisory PR annotation/comment contract before implementing `ripr review-comments`, including changed-line placement, anti-spam caps, bounded LLM guidance, check annotations by default, optional inline review comments, JSON shape, and non-blocking CI posture. |
| `vscode/first-run-status` | done | VS Code now has a status bar and `ripr: Show Status` path for server resolution, workspace detection, analysis running/complete/stale/failed, and no-actionable-seam states without adding unsaved-buffer overlays. |
| `vscode/action-discoverability` | done | Seam diagnostics now group code-action titles around inspect, write targeted test, agent handoff, verify after test, review result, and refresh intent while keeping command IDs and payloads stable. |
| `ci/pr-summary-surface` | done | The generated workflow now writes a reviewer-oriented `RIPR advisory summary` with pilot and agent review content, artifact paths, SARIF and badge status, known limits, and PR guidance annotation counts when `target/ripr/review/comments.json` exists; it also emits non-blocking changed-line check annotations from that report. |
| `ci/generated-workflow-smoke-fixture` | done | The generated workflow smoke fixture now pins artifact paths, top-seam extraction, agent artifact generation, non-blocking posture, optional SARIF gates, badge output, advisory summary sections, and PR guidance annotation hooks. |
| `docs/ux-by-user-type` | done | `docs/QUICKSTART.md` now routes the first hour by VS Code, CI, CLI, and agent/reviewer user type, with troubleshooting and known limits; README keeps the short front-door summary and links to the deeper path. |
| `campaign/first-hour-ux-closeout` | done | Campaign 12 closed after the editor status path, intent-titled actions, generated CI advisory summary, generated workflow smoke fixture, and user-type quickstart made the first hour understandable from VS Code, CI, CLI, and agent/reviewer surfaces. |

Dependencies:

- Campaign 11 should centralize command templates before Campaign 12 adds or
  rewrites command-copy surfaces.
- Campaign 11 workflow manifests should become the source for any guided
  agent work packet shown through editor or CI UX.

Commands:

```bash
cargo test -p ripr lsp
cd editors/vscode
npm ci
npm run compile
npm run package
npm run test:e2e
cd ../..
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-doc-index
cargo xtask check-pr
```

Blocking conditions:

- new analyzer families
- automatic source edits or generated tests
- runtime mutation execution
- default CI blocking
- unsaved-buffer overlays
- new public crates
- duplicated command templates after Campaign 11 centralization
- more report formats that do not improve the VS Code or GitHub first screen

## Campaign 13: PR Review Guidance

Campaign ID: `pr-review-guidance`

Status: complete

Campaigns 10 through 12 made the editor, CLI, agent loop, cockpit, generated
CI artifacts, and first-hour docs converge on the same static evidence loop.
The remaining visible gap is pull-request review projection. RIPR-SPEC-0012
defines `ripr review-comments`, the read-only report producer now exists, and
generated CI now runs that producer before the existing summary and annotation
consumer steps. The exact placement and suppression cases are now
fixture-pinned, and the dedicated PR guidance docs now explain the command, CI
behavior, summary-only fallback, and inline-comment opt-in boundary. The next
step is choosing the next product campaign explicitly.

Objective:

```text
Project the existing static evidence loop into bounded pull-request review
guidance: changed seam -> focused test intent -> verification command -> review
artifact, without turning RIPR into a free-form reviewer or CI blocker.
```

Why it matters:

Humans and LLM agents now have a deterministic workflow once they inspect RIPR
artifacts, but CI-first reviewers still need to download or open reports before
they see the changed seam and focused test intent. Campaign 13 should produce
the smallest PR-facing projection of existing evidence: changed line placement
when safe, summary-only fallback when not safe, bounded test intent, and the
verification command. It must not post comments by default, generate tests, make
CI blocking, or let an LLM decide what matters.

End state:

- `ripr review-comments` writes `target/ripr/review/comments.json` and
  `comments.md` as read-only advisory reports
- review guidance only places line annotations on changed lines and falls back
  to summary-only recommendations otherwise
- guidance is capped, deterministic, deduplicated, and rooted in existing repo
  exposure, agent packet, agent brief, related-test, severity, and suppression
  evidence
- generated GitHub workflows run the report producer before consuming
  `target/ripr/review/comments.json` for summaries and check annotations
- inline PR comments remain opt-in and are not posted by default
- fixtures pin exact-line, owner-function-line, same-file-line, summary-only,
  capped, suppressed, and changed-test skip cases
- docs explain PR guidance as advisory static evidence, not LLM review,
  runtime mutation proof, automatic edits, generated tests, or default CI
  blocking

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `campaign/pr-review-guidance-audit` | done | Audited the long-term static-evidence control-plane objective against current artifacts. The editor/CLI/CI artifact loop is real, but PR review convergence was incomplete because `ripr review-comments` was specified and consumed only as a future report. |
| `review/pr-guidance-renderer` | done | Added read-only `ripr review-comments --root . --base <sha> --head <sha> --out target/ripr/review/comments.json` plus Markdown output, joining existing static evidence to produce bounded PR guidance without posting to GitHub or changing analyzer behavior. |
| `ci/run-pr-guidance-report` | done | Updated generated GitHub workflows to run `ripr review-comments` before the existing advisory summary and check-annotation consumer steps, preserving non-blocking defaults. |
| `fixtures/pr-guidance-cases` | done | Pinned PR guidance fixtures for exact changed seam line, owner-function changed line, same-file changed line, summary-only fallback, cap suppression, configured suppression, and nearby changed-test skip. |
| `docs/pr-review-guidance` | done | Added [PR review guidance](PR_REVIEW_GUIDANCE.md) documenting `ripr review-comments`, generated CI check annotations, summary-only fallback, the inline-comment opt-in boundary, pinned fixture cases, and static-evidence limits. |
| `campaign/pr-review-guidance-closeout` | done | Closed Campaign 13 after PR guidance was produced, consumed by generated CI, fixture-pinned, documented, and kept advisory/non-blocking by default. |

Closeout:

- PR guidance now projects existing RIPR evidence into bounded PR surfaces:
  `ripr review-comments` -> generated CI summary/check annotations -> fixture
  matrix -> dedicated user docs.
- The default workflow remains advisory and non-blocking. Inline PR review
  comments are not posted by generated workflows and remain a custom explicit
  opt-in boundary.
- The guidance path keeps the normal evidence loop intact: changed seam ->
  focused test intent -> agent brief command -> after snapshot -> agent verify
  -> receipt or review summary.
- No analyzer behavior, LSP feature expansion, source edits, generated tests,
  runtime mutation execution, default CI blocking, public crate split, SARIF
  schema change, or badge schema change shipped in this campaign.

Next:

- Campaign 14 is complete. It measured recommendation quality before any
  ranking or policy work so future optional gates can consume calibrated
  evidence rather than unmeasured signal.

Dependencies:

- RIPR-SPEC-0012 remains the product contract for review guidance.
- Campaign 11 shared command templates remain the source for agent brief and
  verify command strings used by review guidance.
- Campaign 12 generated workflow annotation steps remain the non-blocking
  consumers of the producer output.

Commands:

```bash
cargo test -p ripr review_comments
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
```

Blocking conditions:

- free-form LLM review comments
- automatic source edits or generated tests
- runtime mutation execution or runtime adequacy claims
- default CI blocking
- inline PR review comments without explicit opt-in
- comments placed on unrelated unchanged lines
- new public crates

## Campaign 14: Recommendation Calibration

Campaign ID: `recommendation-calibration`

Status: complete

Campaigns 11 through 13 built the deterministic human, CI, editor, and external
agent control plane: selected seam -> brief/packet -> focused test -> after
snapshot -> verify/receipt -> PR guidance. The next trust layer is not a gate.
It is measuring whether the recommendation was worth the reviewer or agent's
attention in the first place.

Objective:

```text
Move RIPR from a complete static evidence loop to measured recommendation
quality: determine whether PR-time guidance is useful, correctly placed,
properly suppressed or capped, and correlated with improved static evidence
after one focused test.
```

Why it matters:

RIPR now emits bounded PR guidance, but that still does not answer the adoption
question: was the top recommendation worth showing to a reviewer? Campaign 14
turns that into repo-local calibration evidence before any future ranking or
policy work. Calibration stays advisory, deterministic, and static; it does not
add telemetry, external services, generated tests, runtime mutation execution,
or default CI blocking.

End state:

- a PR-shaped calibration corpus records useful, noisy, wrong-line,
  already-covered, summary-only, suppression, generated/migration,
  macro-heavy, trait/generic, and async/error-boundary cases
- review guidance outcome receipts can record recommendation outcomes without
  telemetry, external services, source edits, generated tests, or runtime
  mutation execution
- recommendation calibration reports measure top recommendation usefulness,
  false annotations, summary-only fallback correctness, suppression
  correctness, recommended test target correctness, and review-comment latency
- generated CI remains advisory and non-blocking while surfacing calibration
  artifacts when available
- calibration results feed future ranking and policy decisions without opaque
  scores or runtime adequacy claims

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `campaign/recommendation-calibration-audit` | done | Audited the post-Campaign-13 product objective and made recommendation quality the next trust layer before optional policy gates. |
| `spec/recommendation-calibration-report` | done | Defined RIPR-SPEC-0013 for recommendation calibration reports, including inputs, JSON/Markdown shape, usefulness metrics, false annotation tracking, summary-only correctness, suppression correctness, target-file correctness, latency fields, advisory posture, and non-goals. |
| `fixtures/pr-guidance-calibration-corpus` | done | Added PR-shaped calibration expectation metadata for useful recommendation, noisy recommendation, wrong-line placement, already-covered seam, correct summary-only fallback, suppression correctness, generated/migration exclusion, macro-heavy code, trait/generic boundary, and async/error boundary. |
| `review-feedback/outcome-receipts` | done | Added a lightweight review guidance outcome receipt schema and pinned useful, noisy, wrong-line, already-covered, wrong-target, summary-only-correct, and suppressed-correctly receipt fixtures without telemetry or external services. |
| `report/recommendation-precision` | done | Added `cargo xtask recommendation-calibration`, an advisory JSON/Markdown report that joins PR guidance, calibration corpus expectations, optional outcome receipts, suppression state, target placement, latency, and static movement without changing CI blocking defaults. Checked outputs live under `fixtures/boundary_gap/expected/recommendation-calibration/recommendation-calibration.{json,md}`. |
| `docs/calibration-workflow` | done | Added [Recommendation calibration](RECOMMENDATION_CALIBRATION.md), documenting how to run and read the report, outcome receipts, placement quality, suppression correctness, static movement buckets, reviewer use, fixture artifacts, and advisory limits. |
| `campaign/recommendation-calibration-closeout` | done | Closed after recommendation quality was specified, fixture-pinned, receipt-backed, reported, documented, surfaced advisory-first, and ready to inform later ranking or policy work. |

Dependencies:

- Campaign 13 PR guidance remains the placement and recommendation source.
- Campaign 11 receipts and review summaries remain the source for before/after
  static movement and reviewer context.
- Campaign 5A/8 mutation calibration remains supplied-data calibration.
  Recommendation calibration may compare against imported runtime artifacts, but
  RIPR must not run mutation testing.
- Future calibrated gates are policy over measured evidence. They should not
  become active until recommendation quality has a calibration baseline.

Commands:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
```

Blocking conditions:

- LSP feature work
- LLM provider integration
- automatic source edits or generated tests
- runtime mutation execution
- runtime adequacy claims
- default CI blocking
- opaque scores
- telemetry or external service dependencies
- policy gates or acknowledgement labels as part of the calibration report
- new public crates

Closeout:

- Recommendation quality is now specified by RIPR-SPEC-0013 and the output
  schema.
- The PR-shaped calibration corpus pins useful, noisy, wrong-line,
  already-covered, summary-only, suppression, generated/migration,
  macro-heavy, trait/generic, and async/error-boundary cases.
- Outcome receipts provide repo-local review feedback labels without telemetry
  or external services.
- `cargo xtask recommendation-calibration` emits advisory JSON and Markdown
  precision reports from existing artifacts.
- [Recommendation calibration](RECOMMENDATION_CALIBRATION.md) documents how to
  run and read reports, receipts, placement quality, suppression correctness,
  static movement buckets, and limits.
- [Campaign 14 closeout](handoffs/2026-05-08-campaign-14-closeout.md) records
  the PR chain, proof commands, and deferred policy boundary.

Next:

- Campaign 15 is complete. [Campaign 15
  closeout](handoffs/2026-05-08-campaign-15-closeout.md) records the PR chain,
  proof commands, and explicit boundary: optional calibrated gates are available
  only when configured, while generated workflows remain advisory by default.

## Campaign 15: Calibrated Gate Policy

Campaign ID: `calibrated-gate-policy`

Status: complete

Recommendation calibration comes first. Once RIPR has measured whether its
top recommendations are useful, correctly placed, and low-noise, a later policy
lane can define optional gates over that evidence without turning advisory
visibility into blocking behavior by accident.

Objective:

```text
Define the optional calibrated gate layer for PR-time test-oracle evidence:
separate visibility from blocking, fail only under explicit policy, preserve
waiver/acknowledgement paths, and use runtime mutation calibration only as
imported confidence evidence.
```

Why it matters:

RIPR gives reviewers a compact PR-facing test-oracle gap packet, but policy
should earn the right to block. Some teams may eventually want narrow,
high-confidence checks, acknowledgeable warnings, or baseline comparisons.
Campaign 15 should define that policy layer only after Campaign 14 supplies a
recommendation-quality baseline.

End state:

- a gate policy spec defines inputs, outputs, modes, acknowledgement labels,
  calibration evidence, and non-goals before implementation
- a read-only gate evaluator consumes existing PR guidance, repo exposure,
  SARIF policy, suppressions, labels, recommendation calibration, and optional
  mutation calibration reports
- default generated workflows remain advisory and non-blocking unless an
  explicit gate mode is configured
- blocking decisions are deterministic, narrow, auditable, and limited to
  calibrated high-confidence new gaps
- waiver labels such as `ripr-waive` produce visible acknowledged outcomes,
  not silent success
- fixtures pin advisory, acknowledged, fail-on-new-high-confidence-gap,
  baseline-check, suppression, and calibration agreement/disagreement cases
- docs explain visibility versus gating and keep static evidence vocabulary
  separate from runtime mutation outcomes

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/calibrated-gate-policy` | done | Pin the optional calibrated gate policy after recommendation calibration, including modes, inputs, outputs, acknowledgement labels, runtime calibration boundaries, default advisory posture, and non-goals. |
| `gate/policy-evaluator` | done | Add a read-only gate evaluator that writes gate-decision JSON/Markdown from existing evidence and explicit policy without posting comments, editing source, running mutation tests, or changing generated workflow defaults. |
| `fixtures/calibrated-gate-cases` | done | Pin gate fixtures for advisory, acknowledged, baseline-check, fail-on-new-high-confidence-gap, suppression, missing-input, and calibration agreement/disagreement cases. |
| `ci/generated-gate-wiring` | done | Wire generated GitHub workflows to optionally run the gate evaluator only when explicitly configured, preserving advisory defaults and surfacing acknowledged or blocking decisions in summaries. |
| `docs/calibrated-gate-policy` | done | Document calibrated gates as optional policy over existing static evidence, including modes, waiver labels, CI behavior, calibration evidence, and static/runtime vocabulary boundaries. |
| `campaign/calibrated-gate-closeout` | done | Closed Campaign 15 after optional calibrated gates were specified, evaluated, fixture-pinned, optionally wired into generated CI, documented, and kept advisory by default. The closeout audit is recorded in `docs/handoffs/2026-05-08-campaign-15-closeout.md`. |

Campaign 15 is complete. Landed PR chain:

- #554 opened the current calibrated gate policy lane after Campaign 14 supplied
  recommendation calibration.
- #559 pinned RIPR-SPEC-0014 and the gate decision schema contract.
- #560 added the read-only `ripr gate evaluate` producer.
- #561 pinned the calibrated-gate fixture matrix.
- Direct commit `dceb291` wired generated GitHub workflows to run the gate only
  when explicitly configured.
- #564 preserved evidence uploads when explicit gate modes fail.
- #566 added the calibrated gate policy guide and aligned docs with SARIF
  policy inputs.
- `campaign/calibrated-gate-closeout` recorded the final audit and closed the
  campaign.

Dependencies:

- Campaign 14 Recommendation Calibration supplies the signal-quality baseline.
- Campaign 13 PR guidance remains the visibility surface. Gates consume it;
  they do not replace it.
- Campaign 5A/8 mutation calibration remains supplied-data calibration. Gates
  may import calibration artifacts, but RIPR must not run mutation testing.
- Campaign 5B SARIF policy remains a related advisory policy surface; gate
  decisions need their own explicit output contract.
- The `ripr-waive` label remains an acknowledgement path, not a hidden
  suppression.

Closeout:

- [Campaign 15 closeout](handoffs/2026-05-08-campaign-15-closeout.md)
  records the final Campaign 15 PR chain, validation commands, and deferred
  adoption boundary.

Commands:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-pr
```

Blocking conditions:

- default CI blocking
- broad "fail on any RIPR finding" policy
- runtime mutation vocabulary in static gate decisions
- running cargo-mutants or any mutation engine from the gate
- hiding acknowledged or waived gaps from summaries
- treating PR-body validation claims as observed evidence
- posting inline comments as part of the gate evaluator
- automatic source edits or generated tests
- new public crates

Next:

- Campaign 16 is complete. [Campaign 16
  closeout](handoffs/2026-05-08-campaign-16-closeout.md) records the gate
  adoption PR chain, proof commands, and the boundary that Editor Evidence UX
  remains queued until an explicit activation PR or parallel-lane decision.

## Campaign 16: Gate Adoption UX

Campaign ID: `gate-adoption-ux`

Status: complete

Campaign 15 built the optional gate. The next product risk is adoption:
teams need copyable setup examples, visible waiver workflows, baseline guidance,
and first-screen summaries before calibrated policy can be used without
surprise.

Objective:

```text
Make optional calibrated gate adoption safe and obvious for real teams:
provide copyable generated-CI examples, visible waiver and baseline workflows,
first-screen gate summaries, dogfood receipts, and guidance for when blocking is
appropriate without changing advisory defaults.
```

Why it matters:

Calibrated gates are now policy over existing evidence, but the default path
must stay low-risk. Campaign 16 should make the adoption path clear enough that
a team can start with `visible-only`, move to acknowledgement labels, add a
baseline, and only later enable calibrated blocking when local evidence supports
it.

End state:

- docs provide copyable generated-CI examples for `visible-only`,
  `acknowledgeable`, `baseline-check`, and `calibrated-gate`
- waiver and label workflows show how `ripr-waive` produces visible
  acknowledged decisions rather than hidden success
- baseline creation and refresh guidance lets teams avoid punishing historical
  debt while identifying new policy-eligible gaps
- generated CI summaries make gate decisions understandable without opening
  JSON artifacts
- `ripr` dogfood receipts demonstrate visible-only and stricter opt-in gate
  behavior from repo-local evidence
- blocking-readiness guidance explains when teams should leave gates advisory,
  use acknowledgement, or enable calibrated blocking

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `docs/gate-adoption-examples` | done | Added copyable generated-CI repository-variable examples for default advisory posture, `visible-only`, `acknowledgeable`, `baseline-check`, and `calibrated-gate` while preserving generated workflow defaults. |
| `docs/gate-waiver-workflows` | done | Added sample `ripr-waive` label and reviewer workflows that keep acknowledged findings visible in gate decisions, auditable through `target/ci/labels.json`, and separate from durable suppressions. |
| `docs/gate-baseline-workflow` | done | Added baseline creation, review, and refresh guidance that treats baselines as visible historical-debt ledgers, not suppressions, and ties shrink refreshes to focused-test evidence movement. |
| `ci/gate-decision-summary-polish` | done | Added a generated-CI gate decision at-a-glance summary with mode, status, counts, PR labels, acknowledgement labels, applied waiver, baseline, calibration inputs/effects, blocking reason, and artifact paths before the full Markdown report. |
| `dogfood/gate-adoption-receipts` | done | Extended `cargo xtask dogfood` with checked repo-local gate adoption receipts for `visible-only`, acknowledged waiver, baseline-existing, baseline-new, missing-baseline, and explicit calibrated-gate decisions while recording that generated CI remains non-blocking by default. |
| `docs/blocking-readiness-guide` | done | Added RIPR blocking-readiness guidance for staying advisory, requiring acknowledgement, using baseline-check, and enabling calibrated blocking only when local evidence is mature. |
| `campaign/gate-adoption-ux-closeout` | done | Closed Campaign 16 after gate adoption docs, waiver workflows, baseline guidance, CI summary polish, dogfood receipts, and blocking-readiness guidance were complete while defaults stayed advisory. The closeout audit is recorded in `docs/handoffs/2026-05-08-campaign-16-closeout.md`. |

Campaign 16 is complete. Landed PR chain:

- #571 opened Gate Adoption UX after Campaign 15 supplied explicit optional
  gates.
- #573 added copyable generated-CI adoption examples for default advisory,
  `visible-only`, `acknowledgeable`, `baseline-check`, and `calibrated-gate`
  modes.
- #575 documented visible `ripr-waive` acknowledgement workflows.
- #576 documented baseline creation, review, and shrink refresh workflows for
  historical debt.
- #578 polished generated-CI gate summaries, and #581 hardened their Markdown
  escaping.
- #580 added checked repo-local gate adoption dogfood receipts through
  `cargo xtask dogfood`.
- #582 added blocking-readiness guidance for moving from advisory visibility to
  acknowledgement, baseline-check, and calibrated blocking.
- `campaign/gate-adoption-ux-closeout` recorded the final audit and closed the
  campaign.

Dependencies:

- Campaign 15 Calibrated Gate Policy supplies the evaluator, decision schema,
  generated CI opt-in wiring, fixture matrix, and operating model.
- Campaign 14 Recommendation Calibration remains the local signal-quality
  source. Gate adoption docs must not imply calibration exists when an input is
  missing.
- Campaign 13 PR guidance remains the reviewer visibility source. Gate adoption
  should summarize policy decisions over PR guidance, not replace the
  recommendation packet.
- The `ripr-waive` label remains acknowledgement, not suppression.
- Baselines are adoption tools for historical debt, not a reason to hide new
  policy-eligible gaps.

Commands:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-pr
```

Blocking conditions:

- new gate semantics before adoption examples
- default CI blocking
- broader "fail on any RIPR finding" policy
- hiding acknowledged or waived gaps from summaries
- treating missing calibration as high confidence
- runtime mutation vocabulary in static gate adoption docs
- running cargo-mutants or any mutation engine from adoption workflows
- automatic source edits or generated tests
- LSP or analyzer behavior changes in this campaign lane
- new public crates

Closeout:

- [Campaign 16 closeout](handoffs/2026-05-08-campaign-16-closeout.md)
  records the final Campaign 16 PR chain, validation commands, and adoption
  boundary.

Next:

- Campaign 17 closed RIPR Zero Adoption. It turned baselines into burn-down
  ledgers with create, diff, and shrink-only refresh commands while keeping
  generated CI advisory by default.

## Campaign 17: RIPR Zero Adoption

Campaign ID: `ripr-zero-adoption`

Status: complete

Campaign 16 made optional gates adoptable. The next PR/CI product risk is
movement: teams need to see whether a PR introduces new policy-eligible debt,
resolves baseline debt, acknowledges an exception, or moves the repo toward
RIPR 0.

Objective:

```text
Make RIPR 0 adoption concrete for PR/CI users: turn baselines into visible
burn-down ledgers, create reviewed baseline checkpoints from gate decisions,
diff current evidence against checked-in debt, support shrink-only refreshes,
and keep every new policy mode explicit and advisory by default.
```

Why it matters:

Most repositories will not start at RIPR 0. Adoption has to show the whole
truth without punishing the first run: visible baseline debt, resolved debt,
new policy-eligible gaps, acknowledged exceptions, suppressions, stale baseline
entries, and safe commands for shrinking reviewed debt.

End state:

- a baseline debt delta report compares current evidence against reviewed
  baseline debt without auto-adopting new findings
- `ripr baseline create` writes reviewed baseline ledgers from existing
  gate-decision evidence without implying accepted-forever debt
- `ripr baseline diff` reports still-present, resolved, new policy-eligible,
  acknowledged, suppressed, stale, invalid, and missing-input identities
- `ripr baseline update --remove-resolved` supports shrink-only baseline
  refreshes and never auto-adopts new debt in CI
- generated CI uploads baseline debt delta artifacts and summarizes debt
  movement while the gate evaluator remains responsible for pass or fail
- RIPR Zero adoption docs explain initial baseline creation, baseline-check
  rollout, shrink-only refresh, new debt review, and waiver versus baseline
  versus suppression boundaries

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/baseline-debt-delta-report` | done | Defined RIPR-SPEC-0016 and the output-schema contract for comparing current PR/CI evidence to reviewed baseline debt without changing analyzer identity, auto-adopting new debt, or making CI blocking by default. |
| `baseline/create` | done | Added `ripr baseline create` so users can produce stable reviewed `.ripr/gate-baseline.json` ledgers from existing gate-decision evidence without overwriting by default. |
| `baseline/diff` | done | Added `ripr baseline diff` to write baseline-debt-delta JSON/Markdown with still-present, resolved, new, acknowledged, suppressed, stale, invalid, and missing-input buckets. |
| `baseline/update-remove-resolved` | done | Added `ripr baseline update --remove-resolved` as a shrink-only refresh path that removes resolved baseline entries, preserves malformed or ambiguous entries for review, and refuses to auto-adopt new current debt. |
| `ci/baseline-debt-delta-artifacts` | done | Generated CI now runs `ripr baseline diff` when `RIPR_GATE_BASELINE` and `gate-decision.json` are present, uploads the JSON/Markdown through `ripr-reports`, and summarizes baseline debt movement without making the delta report the pass/fail authority. |
| `docs/baseline-ledger-workflow` | done | Added `docs/BASELINE_LEDGER_WORKFLOW.md` to document initial adoption, reviewed baseline creation, baseline-check rollout, shrink-only refresh, new debt review, waiver versus baseline versus suppression, and the path toward RIPR 0. |
| `campaign/ripr-zero-adoption-closeout` | done | Closed Campaign 17 after baseline delta, baseline create/diff/shrink-only update, CI artifacts, and baseline ledger docs were in place while defaults stayed advisory. The closeout audit is recorded in `docs/handoffs/2026-05-09-campaign-17-closeout.md`. |

Dependencies:

- Campaign 16 supplies the visible gate adoption workflow, waiver docs,
  baseline docs, first-screen summaries, dogfood receipts, and blocking
  readiness guide.
- Campaign 15 supplies the explicit gate evaluator and decision schema. RIPR
  Zero adoption consumes gate decisions; it does not redefine gate policy.
- Campaign 14 supplies recommendation calibration. Missing or unknown
  calibration must stay visible rather than becoming confidence.
- Campaign 13 supplies PR guidance. Debt-delta summaries may reference that
  evidence, but the baseline ledger is driven by gate decision identities.
- Existing baselines are adoption checkpoints for historical debt, not
  suppressions and not permission to adopt new debt silently.

Commands:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-pr
```

Blocking conditions:

- analyzer identity rewrites inside Lane 4
- default CI blocking
- baseline auto-adoption of new PR findings
- treating baselines as suppressions or accepted-forever debt
- hiding acknowledged, suppressed, stale, invalid, or missing-input entries
  from summaries
- auto-refreshing or rewriting baselines in generated CI
- runtime mutation vocabulary in static debt-delta summaries
- running cargo-mutants or any mutation engine from adoption workflows
- automatic source edits or generated tests
- LSP/editor behavior changes in this campaign lane
- new public crates

Next:

- Campaign 18 opened RIPR Zero Reporting. It turns the Campaign 17 baseline
  ledger mechanics into repo-level progress, stale-baseline, trend, and repair
  reporting while preserving advisory defaults.

## Campaign 18: RIPR Zero Reporting

Campaign ID: `ripr-zero-reporting`

Status: complete

Campaign 17 made reviewed baselines executable. The next adoption risk is that
teams can create a baseline and see one PR's movement, but they still lack a
repo-level status surface that explains age, ownership, stale entries, debt
trends, top repair areas, and progress toward RIPR 0.

Objective:

```text
Make RIPR Zero progress visible as a reporting layer over reviewed baselines,
baseline debt deltas, gate decisions, and recommendation evidence: show repo
RIPR 0 status, baseline age and ownership, stale-debt warnings, trends, and top
repair areas without changing analyzer identity, gate policy, or advisory
defaults.
```

Why it matters:

RIPR 0 should be an attainable operational target, not a one-time baseline
file. Maintainers need to know whether known debt is aging, whether baseline
entries have owners and reasons, whether debt is shrinking, which repair areas
matter most, and whether CI is routing focused test work without turning RIPR
into default-blocking gateware.

End state:

- a RIPR Zero reporting spec defines repo-level status, debt trends, baseline
  metadata, stale warnings, top debt areas, and repair routing without claiming
  perfect tests or coverage adequacy
- baseline ledgers can carry reviewed owner/reason/created/review metadata
  while preserving compatibility with existing Campaign 17 baseline files
- a read-only RIPR Zero status report joins baseline ledgers, baseline debt
  deltas, gate decisions, and recommendation evidence into JSON/Markdown
  progress summaries
- generated CI can surface RIPR Zero status and top repair areas as advisory
  evidence without making the report the pass/fail authority
- user docs explain how teams read RIPR Zero status, age and refresh
  baselines, route repair packets, and interpret progress toward RIPR 0

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/ripr-zero-reporting-surface` | done | Added RIPR-SPEC-0017 for repo-level RIPR Zero status, baseline metadata, stale warnings, trends, top debt areas, and advisory repair routing without analyzer identity rewrites or default CI blocking. |
| `baseline/metadata-v2` | done | Baseline create now writes additive owner/reason/created/review-after/source metadata, baseline diff reports preserved metadata on baseline-derived items, and shrink-only update preserves existing entry metadata without breaking Campaign 17 ledgers. |
| `report/ripr-zero-status` | done | Added `ripr zero status`, a read-only JSON/Markdown status report that joins reviewed baseline, baseline debt delta, optional gate decision, PR guidance, and recommendation calibration evidence. |
| `ci/ripr-zero-summary` | done | Generated CI now runs `ripr zero status` when `baseline-debt-delta.json` exists, uploads `ripr-zero-status.{json,md}`, and appends a RIPR Zero summary with visible unresolved debt, metadata health, top debt area, and repair route while leaving gate decisions as pass/fail authority. |
| `docs/ripr-zero-reporting-workflow` | done | Added `docs/RIPR_ZERO_REPORTING_WORKFLOW.md` so teams can read RIPR Zero status, age and refresh baselines, route repair packets, and interpret progress without treating RIPR 0 as perfect tests or 100 percent coverage. |
| `campaign/ripr-zero-reporting-closeout` | done | Closed Campaign 18 after RIPR Zero status, baseline metadata, generated-CI reporting, and user workflow docs were in place while defaults stayed advisory. The closeout audit is recorded in `docs/handoffs/2026-05-09-campaign-18-closeout.md`. |

Dependencies:

- Campaign 17 supplies reviewed baseline ledgers, debt delta reports,
  shrink-only refreshes, generated-CI artifacts, and the baseline ledger
  workflow.
- Campaign 16 supplies gate adoption modes and visible acknowledgement
  workflows. RIPR Zero reporting may summarize them; it must not redefine gate
  policy.
- Campaign 14 supplies recommendation calibration. Missing calibration remains
  an explicit unknown, not confidence.
- Campaign 13 supplies PR guidance and repair-oriented recommendations. RIPR
  Zero reporting may route to those packets; it must not generate tests or
  make LLM calls.

Commands:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-pr
```

Blocking conditions:

- analyzer identity rewrites inside Lane 4
- recommendation ranking changes
- gate policy semantic changes
- default CI blocking
- baseline auto-adoption of new current debt
- treating baselines as suppressions or accepted-forever debt
- hiding acknowledged, suppressed, stale, invalid, or missing-input entries
  from summaries
- runtime mutation vocabulary in static RIPR Zero summaries
- running cargo-mutants or any mutation engine from adoption workflows
- automatic source edits or generated tests
- LSP/editor behavior changes in this campaign lane
- new public crates

Next:

- No ready work item remains in Campaign 18. Open the next product lane as a
  new explicit campaign rather than editing RIPR Zero Reporting in place.

## Campaign 19: PR Evidence Ledger

Campaign ID: `pr-evidence-ledger`

Status: complete

Campaign 18 made RIPR Zero status visible for the current PR and current
baseline state. The next adoption risk is history: teams need to know whether
PRs are shrinking baseline debt, adding new policy-eligible gaps, accumulating
waivers, preserving repair receipts, and improving behavioral grip even when
line coverage does not move.

Objective:

```text
Turn PR-time RIPR evidence into a durable adoption ledger: record per-PR
behavioral grip movement, waiver and suppression visibility, baseline burn-down,
repair receipts, and coverage/grip frontier signals without changing analyzer
identity, gate policy, recommendation ranking, or advisory defaults.
```

Why it matters:

RIPR Zero is not just a status page. Maintainers need an audit trail that
explains whether each PR improved or worsened behavioral grip, which waivers
are aging, which suppressions remain durable policy exceptions, whether
baseline debt is shrinking, and whether focused tests improved static evidence
without implying that coverage is adequacy.

End state:

- a PR evidence ledger spec defines append-only PR movement records for new
  policy-eligible gaps, resolved baseline debt, acknowledgements, suppressions,
  gate mode, repair receipts, and optional coverage/grip signals
- a read-only PR evidence ledger report writes JSON/Markdown from existing gate
  decisions, baseline debt deltas, RIPR Zero status, recommendation
  calibration, outcome receipts, and optional coverage data
- generated CI can upload and summarize the PR evidence ledger as advisory
  history while leaving gate decisions as the pass/fail authority
- coverage/grip frontier reporting makes execution coverage and behavioral grip
  movement visible as separate axes without treating coverage as adequacy
- user docs explain how teams use PR evidence ledgers for waiver aging,
  baseline burn-down, repair routing, and movement toward RIPR 0

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/pr-evidence-ledger-surface` | done | Added RIPR-SPEC-0018 as the PR evidence ledger contract for append-only per-PR movement, waiver aging, baseline burn-down, repair receipts, optional coverage/grip frontier signals, and advisory-only CI projection without analyzer identity rewrites or default blocking. |
| `report/pr-evidence-ledger` | done | Added `ripr pr-ledger record`, a read-only JSON/Markdown report over existing PR guidance, gate decision, baseline debt delta, RIPR Zero status, recommendation calibration, agent receipt, optional coverage, and optional history inputs. |
| `ci/pr-evidence-ledger-summary` | done | Generated GitHub CI now runs `ripr pr-ledger record` on pull requests after PR guidance, optional gate, baseline delta, and RIPR Zero reports exist; uploads `pr-evidence-ledger.{json,md}` with the normal artifact packet; and appends a PR movement card while leaving gate decisions as the pass/fail authority. |
| `report/coverage-grip-frontier` | done | Added `ripr coverage-grip frontier`, a read-only JSON/Markdown report that keeps coverage delta, RIPR movement, quadrants, interpretation, warnings, and advisory limits as separate axes without treating coverage as adequacy. |
| `docs/pr-evidence-ledger-workflow` | done | Added `docs/PR_EVIDENCE_LEDGER_WORKFLOW.md`, explaining how teams read PR evidence ledgers, use waiver aging and baseline burn-down, route repair receipts, interpret coverage/grip frontier signals, and track movement toward RIPR 0 without learning internal report topology. |
| `campaign/pr-evidence-ledger-closeout` | done | Closed Campaign 19 after PR evidence ledgers, generated-CI projection, coverage/grip frontier summaries, and user workflow docs landed while defaults stayed advisory and gate decisions remained the pass/fail authority. |

Dependencies:

- Campaign 18 supplies RIPR Zero status, baseline metadata health, trend
  availability, top debt areas, repair routes, and generated-CI projection.
- Campaign 17 supplies reviewed baselines and baseline debt delta reports.
- Campaign 16 supplies visible waiver and baseline adoption workflows.
- Campaign 15 supplies gate decisions. The ledger may record gate output; it
  must not redefine gate policy or pass/fail authority.
- Campaign 14 supplies recommendation calibration and outcome receipts. Missing
  calibration remains explicit unknown evidence.
- Coverage data is optional execution evidence. The campaign must not turn RIPR
  into a coverage dashboard or treat coverage movement as adequacy.

Commands:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-pr
```

Blocking conditions:

- analyzer identity rewrites inside Lane 4
- recommendation ranking changes
- gate policy semantic changes
- default CI blocking
- making the PR evidence ledger the pass/fail authority
- baseline auto-adoption of new current debt
- treating baselines, waivers, or suppressions as interchangeable
- hiding acknowledged, suppressed, stale, invalid, or missing-input entries
  from summaries
- runtime mutation vocabulary in static ledger summaries unless imported
  runtime calibration is explicitly cited
- treating coverage movement as test adequacy
- running cargo-mutants or any mutation engine from ledger workflows
- automatic source edits or generated tests
- LSP/editor behavior changes in this campaign lane
- new public crates

Closeout:

Campaign 19 closed PR Evidence Ledger. It turned per-PR RIPR evidence into an
adoption ledger for new policy-eligible gaps, resolved baseline debt,
acknowledgements, suppressions, gate mode, repair receipts, waiver aging,
optional history, and optional coverage/grip frontier signals. The campaign
kept Lane 4's boundary: it consumed existing analyzer, gate, calibration,
baseline, and receipt artifacts; it did not change analyzer identity,
recommendation ranking, gate policy semantics, LSP/editor behavior, mutation
execution, source editing, generated tests, public crate shape, or generated-CI
advisory defaults.

No ready work item remains in `.ripr/goals/active.toml` after this closeout.
Open the next product campaign explicitly rather than extending PR Evidence
Ledger by inertia.

## Campaign 20: Test-Oracle Assistant Proof

Campaign ID: `test-oracle-assistant-proof`

Status: complete

Campaign 19 made per-PR adoption history visible. The next product risk is
whether the already-built surfaces form one review loop instead of a collection
of reports: changed Rust behavior should lead to one visible recommendation,
one bounded handoff packet, one focused test, one after-evidence check, one
receipt, and one advisory PR/CI projection.

Objective:

```text
Prove the full PR-time test-oracle assistant loop: changed Rust behavior flows
through static evidence, PR/editor guidance, a bounded focused-test handoff,
before/after verification, receipt, and advisory CI/ledger projection without
changing analyzer semantics, recommendation ranking, gate policy, LSP behavior,
or default CI blocking.
```

Why it matters:

RIPR has the individual pieces needed for the product promise: PR guidance,
editor evidence, agent packets, receipts, calibrated gates, baselines, RIPR
Zero reports, ledgers, and coverage/grip frontier reports. Teams still need a
checked first path that shows how those pieces fit together for one changed
behavior without artifact archaeology or internal vocabulary.

End state:

- a spec defines the end-to-end proof contract from diff evidence through
  recommendation, handoff packet, focused-test repair, after-evidence, receipt,
  and advisory PR/CI projection
- a canonical review-loop fixture pins one changed-behavior case across before
  evidence, top recommendation, related test, focused test shape, after
  evidence, receipt, and ledger projection expectations
- a dogfood receipt proves the current repo can trace one seam through PR
  guidance, editor/agent packet surfaces, verification commands, receipts, and
  advisory CI artifacts without artifact archaeology
- user docs explain the PR-time assistant workflow without requiring users to
  learn cockpit or internal report topology first
- closeout records which parts of the loop are verified, which remain advisory,
  and which future work must not blur static evidence with runtime mutation
  confirmation

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/test-oracle-assistant-loop` | done | Added RIPR-SPEC-0019 as the end-to-end test-oracle assistant proof contract from changed Rust behavior through static evidence, PR/editor guidance, focused-test handoff, after-evidence verification, receipt, and advisory PR/CI projection without changing analyzer, policy, editor, or CI defaults. |
| `fixtures/canonical-review-loop` | done | Added the canonical boundary-gap replay corpus under `fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/` and a regression test that pins one seam across recommendation, related-test context, suggested focused test, before/after static movement, receipt, and PR ledger projection expectations. |
| `dogfood/test-oracle-assistant-receipt` | done | Recorded a repo-local proof receipt that traces seam `67fc764ba37d77bd` through PR guidance, editor/agent packet surfaces, verification commands, after-evidence, receipt, PR evidence ledger, and coverage/grip frontier availability without changing source automatically. |
| `docs/test-oracle-assistant-workflow` | done | Added `docs/TEST_ORACLE_ASSISTANT_WORKFLOW.md`, documenting the user workflow from PR recommendation or editor diagnostic to bounded handoff, one focused test, after evidence, receipt, and advisory CI/ledger projection while preserving static-evidence limits. |
| `campaign/test-oracle-assistant-proof-closeout` | done | Closed Campaign 20 after the end-to-end assistant proof contract, canonical fixture, dogfood receipt, and user workflow docs demonstrated the full loop while defaults stay advisory. |

Dependencies:

- Campaign 13 supplies bounded PR guidance and changed-line-safe annotation
  placement.
- Campaign 14 supplies recommendation calibration and outcome receipts.
  Missing calibration remains explicit unknown evidence.
- Campaign 15 supplies optional gate decisions. This campaign may display gate
  output; it must not redefine gate policy or pass/fail authority.
- Campaigns 17 and 18 supply baseline debt deltas and RIPR Zero status.
- Campaign 19 supplies the PR evidence ledger and coverage/grip frontier.
- Editor Evidence UX supplies the saved-workspace editor handoff surface.

Commands:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-pr
```

Blocking conditions:

- analyzer identity rewrites inside the proof campaign
- recommendation ranking changes
- gate policy semantic changes
- default CI blocking
- making any ledger or proof receipt the pass/fail authority
- hiding acknowledged, suppressed, stale, invalid, or missing-input entries
- runtime mutation vocabulary in static proof surfaces unless imported runtime
  calibration is explicitly cited
- treating coverage movement as test adequacy
- running cargo-mutants or any mutation engine from proof workflows
- automatic source edits or generated tests
- LSP/editor behavior changes in this campaign lane
- new public crates

Next:

- No ready work item remains in Campaign 20. Choose the next campaign
  explicitly before opening another product lane.

## Campaign 21: Test-Oracle Assistant Report Producer

Campaign ID: `test-oracle-assistant-report-producer`

Status: complete

Campaign 20 proved the assistant loop as a contract, fixture, dogfood receipt,
and user workflow. The next product gap is that users still need a first-class
read-only report producer instead of reading the artifact chain by hand.

Objective:

```text
Make the test-oracle assistant loop a concrete report surface: join existing
PR guidance, editor/agent handoff, before/after static evidence, receipts, PR
ledger, optional gate decisions, and optional coverage/grip frontier inputs
into advisory `test-oracle-assistant-proof.{json,md}` artifacts without
rerunning hidden analysis, editing source, generating tests, calling providers,
running mutation testing, or changing default CI blocking.
```

End state:

- `ripr assistant-loop proof` writes JSON and Markdown from explicit existing
  artifact paths
- the report preserves selected seam identity, missing discriminator, placement
  state, handoff command, static movement, receipt path, PR ledger path,
  optional gate path, optional coverage/grip frontier path, warnings, and
  static limits
- fixtures and tests cover complete proof, summary-only guidance, missing
  optional inputs, missing required inputs, unchanged movement, and advisory
  limits
- generated CI may surface the proof report only as advisory evidence when
  inputs exist
- user docs explain how to read the proof report without artifact archaeology
- closeout records remaining advisory boundaries and future work exclusions

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `report/test-oracle-assistant-proof` | done | Implemented `ripr assistant-loop proof`, a read-only report producer that writes advisory JSON and Markdown from explicit Campaign 20 artifact inputs while preserving static vocabulary and advisory limits. |
| `ci/test-oracle-assistant-proof-artifacts` | done | Generated GitHub CI now runs `ripr assistant-loop proof` only when PR guidance, editor/agent brief, before/after evidence, agent receipt, and PR evidence ledger inputs exist, uploads `test-oracle-assistant-proof.{json,md}`, and appends proof summary content without changing default blocking. |
| `docs/test-oracle-assistant-proof-report` | done | Added `docs/TEST_ORACLE_ASSISTANT_PROOF_REPORT.md`, explaining how reviewers, maintainers, and coding agents read proof report status, warnings, static movement, optional CI projection, handoff fields, and advisory limits without artifact archaeology. |
| `campaign/test-oracle-assistant-report-closeout` | done | Closed Campaign 21 after the proof report producer, generated-CI advisory projection, docs, and validation demonstrated a first-class report surface without changing analyzer, gate, editor, or mutation behavior. |

Dependencies:

- Campaign 20 supplies RIPR-SPEC-0019, the canonical replay corpus, dogfood
  receipt, and user workflow docs.
- Campaign 13 supplies bounded PR guidance and changed-line-safe annotation
  placement.
- Campaign 19 supplies PR evidence ledger and coverage/grip frontier inputs.
- Agent loop artifacts supply existing handoff, verify, and receipt paths.
- Optional gate decisions stay separate from proof report pass/fail authority.

Commands:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-pr
```

Blocking conditions:

- analyzer identity rewrites
- recommendation ranking changes
- gate policy semantic changes
- default CI blocking
- hidden reruns or implicit artifact discovery in the proof producer
- making proof reports the pass/fail authority
- runtime mutation vocabulary without imported runtime calibration
- treating coverage movement as test adequacy
- running cargo-mutants or any mutation engine from proof workflows
- automatic source edits or generated tests
- LSP/editor behavior changes in this campaign lane
- new public crates

Next:

- Campaign 22, First Useful Action, is now closed as the current completed
  campaign in `.ripr/goals/active.toml`. Its report contract, routing corpus,
  read-only producer, generated CI projection, editor projection, workflow
  guide, dogfood receipts, and closeout audit are pinned. No ready Campaign 22
  work item remains; choose the next campaign explicitly before opening another
  product lane.

## Campaign 22: First Useful Action

Campaign ID: `first-useful-action`

Status: closed in the work-item ledger. `.ripr/goals/active.toml` remains the
current completed campaign manifest until the next campaign is explicitly
opened.

Campaign 21 made the test-oracle assistant proof loop a first-class advisory
report. The next product risk is report sprawl: users should not need to know
which artifact is authoritative before they know the next useful test action.
This campaign compresses existing evidence into one advisory action for
developers, reviewers, and coding agents.

Objective:

```text
Given existing RIPR artifacts, produce one advisory first-useful-action report
that tells a developer, reviewer, or coding agent what to do next, why, where,
how to verify, what receipt proves the result, and what limits remain without
rerunning hidden analysis, editing source, generating tests, calling providers,
running mutation testing, changing default CI blocking, or inventing policy.
```

End state:

- `target/ripr/reports/first-useful-action.json` and `.md` summarize the
  highest-value next test action or the reason no action should be taken
- routing is deterministic over explicit existing inputs such as PR guidance,
  PR evidence ledger, baseline debt delta, assistant proof, receipts, optional
  gate decisions, optional coverage/grip frontier, editor context, and
  status/staleness
- fixtures pin actionable, stale, missing-artifact, baseline-only,
  acknowledged, waived, suppressed, already-improved, unchanged-after-attempt,
  and no-actionable-seam cases
- generated CI surfaces the first useful action as advisory summary/artifact
  content without changing pass/fail authority
- the editor may project the report in status or Show Status without new
  diagnostics, CodeLens, inlay hints, unsaved-buffer overlays, source edits, or
  generated tests
- docs and dogfood receipts show how developers, reviewers, and agents use the
  action without treating static evidence as runtime proof

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/first-useful-action-report` | done | Added RIPR-SPEC-0020 plus OUTPUT_SCHEMA, traceability, capability, campaign, plan, roadmap, and changelog updates for the first-useful-action report contract before implementation. |
| `fixtures/first-useful-action-corpus` | done | Added `fixtures/boundary_gap/expected/first-useful-action/` with a routing index plus expected JSON/Markdown report outputs for actionable, stale, missing-required-artifact, baseline-only, acknowledged, waived, suppressed, no-actionable-seam, already-improved, and unchanged-after-attempt cases. |
| `report/first-useful-action` | done | Added the read-only `ripr first-action` producer, JSON/Markdown renderers, explicit artifact input parsing, fixture-pinned routing tests, and CLI smoke coverage without rerunning hidden analysis. |
| `ci/first-useful-action-summary` | done | Generated GitHub CI now runs `ripr first-action` when explicit first-action inputs are already present, uploads `first-useful-action.{json,md}` with the normal report packet, and appends a First Useful Action summary without changing default blocking. |
| `lsp/first-useful-action-status` | done | VS Code status and `ripr: Show Status` now project an existing `target/ripr/reports/first-useful-action.json` report without invoking `ripr first-action`, adding diagnostics, CodeLens, inlay hints, unsaved-buffer analysis, source edits, or generated tests. |
| `docs/first-useful-action-workflow` | done | Added `docs/FIRST_USEFUL_ACTION_WORKFLOW.md`, documenting GitHub and editor entry points, status meanings, developer/reviewer/agent actions, verification, receipts, fallback interpretation, and the advisory gate boundary. |
| `dogfood/first-useful-action-receipts` | done | Extended `cargo xtask dogfood` with checked repo-local first-action receipts for actionable, baseline-only, stale, missing-required-artifact, unchanged-after-attempt, and no-actionable-seam cases while recording advisory limits. |
| `campaign/first-useful-action-closeout` | done | Closed Campaign 22 with a prompt-to-artifact audit, PR chain, validation plan, explicit future-lane boundary, and handoff at `docs/handoffs/2026-05-09-campaign-22-closeout.md`. |

Dependencies:

- Campaign 13 supplies PR guidance and bounded changed-line recommendation
  placement.
- Campaign 14 supplies calibration metrics and outcome receipt vocabulary.
- Campaign 15 supplies optional gate decision artifacts without making gates
  the first-action authority.
- Campaigns 17 and 18 supply baseline debt delta and RIPR Zero status inputs.
- Campaign 19 supplies PR evidence ledger and coverage/grip frontier inputs.
- Campaigns 20 and 21 supply the assistant proof loop and proof report.
- Editor Evidence UX supplies the saved-workspace editor context and projection
  surface.

Commands:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-pr
```

Blocking conditions:

- analyzer behavior changes
- recommendation ranking model or provider calls
- source edits or generated tests
- runtime mutation execution
- default CI blocking
- policy or gate semantic changes
- hidden analysis reruns or implicit artifact discovery in the report producer
- new diagnostics, CodeLens, inlay hints, unsaved-buffer overlays, or other
  speculative editor surfaces
- treating coverage, static movement, or first-action routing as runtime
  adequacy
- new public crates

Follow-up:

- Campaign 23, Assistant Loop Health, is now closed. It made
  assistant-directed test work measurable across existing proof artifacts.

## Campaign 23: Assistant Loop Health

Campaign ID: `assistant-loop-health`

Status: complete

Campaign 21 made one assistant-directed test loop reviewable as
`test-oracle-assistant-proof.{json,md}`. Campaign 22 settled the first-screen
routing contract so users get one next action instead of another raw report.
Assistant Loop Health now measures whether proof packets are complete, stuck,
missing receipts, or actually improving static evidence over time.

Objective:

```text
Summarize proof completeness, missing inputs, static evidence movement,
recurring warnings, and next repair queues across one or more assistant proof
reports without changing analyzer behavior, recommendation ranking, gate
semantics, LSP/editor behavior, mutation execution, provider calls, source
files, generated tests, or default CI blocking.
```

End state:

- `target/ripr/reports/assistant-loop-health.json` and `.md` summarize
  assistant proof packet health from explicit existing proof inputs
- the report counts complete, partial, missing-required, and missing-optional
  proof packets
- the report summarizes static movement as improved, unchanged, regressed, or
  unknown using proof data only
- recurring warnings and missing inputs are grouped without turning into an
  opaque score
- a bounded repair queue routes maintainers or coding agents to rerun missing
  commands, regenerate stale inputs, inspect unchanged evidence, or attach
  receipts
- generated CI uploads and summarizes the report as advisory evidence only

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/assistant-loop-health-report` | done | Added RIPR-SPEC-0022 plus OUTPUT_SCHEMA, traceability, capability, campaign, plan, roadmap, and changelog updates for the assistant-loop-health report contract before implementation. |
| `fixtures/assistant-loop-health-corpus` | done | Added `fixtures/boundary_gap/expected/assistant-loop-health/` with complete-improved, partial-missing-optional, missing-required-input, unchanged, regressed, warning-heavy, and multi-proof report fixtures plus representative proof inputs. |
| `report/assistant-loop-health` | done | Added the read-only `ripr assistant-loop health` producer over explicit proof inputs, with JSON/Markdown rendering and fixture-backed CLI coverage. |
| `ci/assistant-loop-health-artifacts` | done | Generated GitHub CI runs `ripr assistant-loop health` when `test-oracle-assistant-proof.json` exists, uploads `assistant-loop-health.{json,md}` with the normal report packet, and appends an advisory health summary. |
| `docs/assistant-loop-health-workflow` | done | Added `docs/ASSISTANT_LOOP_HEALTH_WORKFLOW.md`, explaining proof report versus health report, generated-CI summary use, complete/partial/missing states, static movement interpretation, missing-input repair, agent handoff, and advisory limits. |
| `campaign/assistant-loop-health-closeout` | done | Closed Campaign 23 with a prompt-to-artifact audit, PR chain, validation plan, advisory boundary, and future-lane boundary at `docs/handoffs/2026-05-09-campaign-23-closeout.md`. |

References:

- [Assistant Loop Health proposal](ASSISTANT_LOOP_HEALTH_PROPOSAL.md)
- [RIPR-SPEC-0022](specs/RIPR-SPEC-0022-assistant-loop-health-report.md)
- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)

Blocking conditions:

- analyzer behavior or recommendation ranking changes
- gate policy, LSP/editor, provider, mutation, source-edit, generated-test, or
  default-blocking changes
- adequacy, correctness, or runtime mutation claims

Closeout:

- Campaign 23 is closed. The report contract, fixture corpus, read-only
  producer, generated-CI advisory projection, workflow docs, and closeout audit
  are in place. No ready Campaign 23 work item remains in
  `.ripr/goals/active.toml`.
- Campaign 24, PR Review Front Panel, is now closed. It composes existing
  artifacts into one advisory generated-CI first screen without changing
  analyzer, ranking, gate, editor, provider, mutation, source-edit,
  generated-test, or default-blocking behavior.

## Campaign 24: PR Review Front Panel

Campaign ID: `pr-review-front-panel`

Status: complete

Campaigns 13 through 23 built the PR guidance, calibration, optional gate,
baseline, ledger, assistant proof, first useful action, and assistant-loop
health surfaces. The next reviewer risk is report sprawl: the evidence exists,
but the GitHub first screen still should compose it into one test-oracle review
story.

Objective:

```text
Compose existing PR guidance, first useful action, assistant proof,
assistant-loop health, PR evidence ledger, baseline delta, gate decision,
receipts, calibration, and optional coverage/grip frontier artifacts into one
advisory PR review front panel that answers what matters, why, what to do next,
how to verify it, what receipt exists, and what policy state applies without
changing analyzer behavior, recommendation ranking, gate semantics, editor
behavior, mutation execution, provider calls, source files, generated tests, or
default CI blocking.
```

End state:

- `target/ripr/reports/pr-review-front-panel.json` and `.md` summarize the PR's
  test-oracle story from explicit existing inputs
- the panel shows the selected top issue or explains why no safe action is
  available
- the panel carries missing discriminator, related test, repair or agent
  handoff command, verify command, receipt path, and static movement when
  present
- baseline, new policy-eligible, acknowledged, waived, suppressed, and gated
  states remain visible without becoming hidden success
- optional coverage/grip and calibration inputs are surfaced as advisory
  context without adequacy claims
- generated CI uploads and summarizes the panel as advisory evidence only when
  source artifacts exist

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/pr-review-front-panel-report` | done | Added RIPR-SPEC-0023 plus OUTPUT_SCHEMA, traceability, capability, campaign, plan, roadmap, and changelog updates for the PR review front-panel JSON/Markdown contract, explicit inputs, first-screen fields, artifact grouping, advisory limits, and generated-CI projection boundaries. |
| `fixtures/pr-review-front-panel-corpus` | done | Pinned advisory-only, actionable, summary-only, acknowledged, suppressed, baseline-resolved, blocked, missing-proof, and coverage-flat-grip-improved cases plus an xtask guard before adding the producer. |
| `report/pr-review-front-panel` | done | Added `ripr pr-review front-panel`, a read-only producer that emits advisory JSON/Markdown from explicit existing artifact paths without rerunning analysis or changing gate authority. |
| `ci/pr-review-front-panel-summary` | done | Generated GitHub CI now runs `ripr pr-review front-panel` only when explicit input artifacts exist, uploads `pr-review-front-panel.{json,md}` with the report packet, and appends an advisory first-screen summary while preserving gate-decision pass/fail authority. |
| `docs/pr-review-front-panel-workflow` | done | Added the workflow guide for reviewers, maintainers, developers, and coding agents, including first-screen reading, repair routes, receipt inspection, and advisory gate limits. |
| `dogfood/pr-review-front-panel-receipts` | done | Added dogfood validation and output-schema documentation for checked front-panel receipts covering actionable, acknowledged, suppressed, baseline-resolved, blocked, missing-proof, no-actionable, and coverage-flat-grip-improved cases while preserving advisory defaults. |
| `campaign/pr-review-front-panel-closeout` | done | Closed Campaign 24 with a prompt-to-artifact audit, PR chain, validation plan, advisory boundary, and future-lane boundary at `docs/handoffs/2026-05-10-campaign-24-closeout.md`. |

References:

- [PR Review Front Panel proposal](PR_REVIEW_FRONT_PANEL_PROPOSAL.md)
- [RIPR-SPEC-0023: PR Review Front Panel Report](specs/RIPR-SPEC-0023-pr-review-front-panel-report.md)
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md)
- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
- [Assistant Loop Health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md)
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md)

Blocking conditions:

- analyzer behavior changes or recommendation ranking changes
- gate policy, waiver, suppression, or baseline semantic changes
- LSP/editor, provider, mutation, source-edit, generated-test, inline-comment,
  or default-blocking changes
- adequacy, correctness, or runtime mutation claims
- hidden analysis reruns or source artifact discovery that changes semantics

Closeout:

- Campaign 24 is closed. The report contract, fixture corpus, read-only
  producer, generated-CI advisory projection, workflow docs, dogfood receipts,
  and closeout audit are in place. No ready Campaign 24 work item remains in
  `.ripr/goals/active.toml`.
- The next Lane 4 adoption surface should be opened explicitly rather than
  folded into PR Review Front Panel closeout work.

## Campaign 25: Report Packet Index

Campaign ID: `report-packet-index`

Status: complete

Campaigns 13 through 24 made PR guidance, gates, baselines, ledgers,
assistant proof, first useful action, assistant-loop health, and the PR review
front panel visible in generated CI. The next reviewer risk is artifact packet
navigation: the uploaded `ripr-reports` artifact should have one index that
shows where to start, what is missing, and which artifact answers each review
question.

Objective:

```text
Make the uploaded RIPR report packet navigable as a reviewer-first index over
explicit existing artifacts: the front panel, PR guidance, first useful action,
assistant proof and health, PR evidence ledger, baseline delta, RIPR Zero, gate
decision, receipts, calibration, coverage/grip frontier, SARIF/badge outputs,
and local validation reports. The index must group artifacts by reviewer use,
show missing or stale expected surfaces, name the next command to regenerate a
missing packet, and remain advisory without changing analyzer behavior,
recommendation ranking, gate semantics, editor behavior, mutation execution,
provider calls, source files, generated tests, inline-comment defaults, or
default CI blocking.
```

End state:

- `target/ripr/reports/index.json` and `.md` are the report-packet front door
  for PR reviewers
- the index groups artifacts by start-here, PR review story, repair or agent
  handoff, evidence, policy or gates, calibration, validation receipts, and
  SARIF or badge outputs
- the index identifies missing expected report surfaces and suggests precise
  commands to regenerate them
- generated CI uploads and summarizes the index as advisory evidence only when
  source artifacts exist
- the index never becomes pass/fail authority and never hides gate, waiver,
  suppression, baseline, missing-input, or warning states
- fixtures and dogfood receipts cover complete packet, sparse packet, missing
  front-panel, blocked gate, missing proof, and coverage/grip-present cases

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/report-packet-index-contract` | done | Defined the report-packet index contract, explicit inputs, grouping model, missing-surface warnings, generated-CI projection boundary, advisory limits, and fixture-first implementation plan before changing the producer. |
| `fixtures/report-packet-index-corpus` | done | Pinned report-packet index cases for complete packet, sparse advisory packet, missing front panel, blocked gate, missing assistant proof, missing receipts, and coverage/grip-present packet before changing the producer. |
| `report/report-packet-index` | done | Added the public read-only `ripr reports index` producer, JSON/Markdown renderer, CLI/help wiring, and CLI smoke coverage for grouped reviewer-first packet indexes over explicit artifact paths without rerunning analysis or changing gate authority. |
| `ci/report-packet-index-summary` | done | Generated GitHub CI runs `ripr reports index` only when indexed artifacts exist, uploads `index.{json,md}` with the report packet, and appends a compact advisory packet-index summary while preserving gate-decision pass/fail authority. |
| `docs/report-packet-index-workflow` | done | Added `docs/REPORT_PACKET_INDEX_WORKFLOW.md`, explaining reviewer, maintainer, developer, and coding-agent use of the grouped packet map, missing-surface regeneration, and advisory gate boundary. |
| `dogfood/report-packet-index-receipts` | done | Extended `cargo xtask dogfood` with checked report-packet index receipts for complete, sparse, missing-front-panel, blocked-gate, missing-proof, missing-receipts, and coverage/grip-present cases. |
| `campaign/report-packet-index-closeout` | done | Closed Campaign 25 with a prompt-to-artifact audit, PR chain, validation plan, advisory boundary, and future-lane boundary at `docs/handoffs/2026-05-10-campaign-25-closeout.md`. |

References:

- [Report Packet Index proposal](REPORT_PACKET_INDEX_PROPOSAL.md)
- [RIPR-SPEC-0024: Report Packet Index](specs/RIPR-SPEC-0024-report-packet-index.md)
- [Report packet index workflow](REPORT_PACKET_INDEX_WORKFLOW.md)
- [PR Review Front Panel workflow](PR_REVIEW_FRONT_PANEL_WORKFLOW.md)
- [PR automation](PR_AUTOMATION.md)
- [CI](CI.md)

Blocking conditions:

- analyzer behavior changes or recommendation ranking changes
- gate policy, waiver, suppression, or baseline semantic changes
- LSP/editor, provider, mutation, source-edit, generated-test, inline-comment,
  or default-blocking changes
- adequacy, correctness, or runtime mutation claims
- hidden analysis reruns or artifact discovery that changes upstream report
  semantics

Closeout:

- Campaign 25 is closed. The report contract, fixture corpus, read-only
  producer, generated-CI advisory projection, workflow docs, dogfood receipts,
  and closeout audit are in place. No ready Campaign 25 work item remains in
  `.ripr/goals/active.toml`.
- The next Lane 4 adoption surface should be opened explicitly rather than
  folded into Report Packet Index closeout work.

## Campaign 26: PR Inline Comment Publisher

Campaign ID: `pr-inline-comment-publisher`

Status: closed

Campaigns 13 through 25 made PR guidance, generated-CI summaries, changed-line
check annotations, optional gates, baselines, ledgers, assistant proof, first
useful action, assistant-loop health, PR review front panel, and report-packet
index artifacts visible without posting durable PR comments. The next adoption
risk is explicit inline comment publishing for teams that choose review-thread
visibility after the summary and annotation surfaces are trusted.

Objective:

```text
Define and implement an explicit opt-in inline PR comment publisher over the
existing `ripr review-comments` artifact without changing default generated CI.
The lane must first produce a read-only publish plan from explicit
`target/ripr/review/comments.json` input, then optionally publish only safe
changed-line comments when a workflow explicitly enables it. The publisher must
never post `summary_only` guidance, must cap comments, deduplicate by
`dedupe_key`, upsert or replace prior RIPR comments, preserve advisory
language, avoid hidden analysis reruns, and avoid analyzer, ranking, gate,
editor, provider, mutation, source-edit, generated-test, branch-protection, or
default-blocking changes.
```

End state:

- `target/ripr/review/comment-publish-plan.json` and `.md` describe intended
  inline comment operations before anything posts
- the plan consumes only explicit `review-comments` artifacts and optional
  existing-comment metadata
- summary-only guidance is never publishable as an inline comment
- publishable comments target changed lines only and are capped to three by
  default
- dedupe keys drive upsert or replace behavior so RIPR comments do not
  duplicate across reruns
- generated CI keeps inline comments disabled by default and only posts when
  explicit configuration and safe permissions exist
- fixtures and dogfood receipts cover publishable, summary-only, capped,
  duplicate, stale-existing, fork or no-token, and missing-input cases

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/pr-inline-comment-publisher-contract` | done | Defined the optional inline comment publisher contract, read-only publish-plan schema, explicit inputs, permission boundary, dedupe/upsert semantics, cap rules, summary-only exclusion, generated-CI default-off posture, and fixture-first implementation plan before changing producer or workflow behavior. |
| `fixtures/pr-inline-comment-publisher-corpus` | done | Pinned inline comment publish-plan cases for publishable changed-line comments, summary-only exclusion, cap overflow, dedupe/upsert, stale existing RIPR comments, fork or no-token no-op, and missing review-comments input before adding publisher behavior. |
| `report/pr-inline-comment-publish-plan` | done | Added read-only `ripr pr-comments plan` JSON/Markdown producer over explicit review-comments and optional existing-comment metadata without posting to GitHub or changing gate authority. |
| `ci/pr-inline-comment-publisher` | done | Added generated GitHub CI wiring with `RIPR_COMMENT_MODE=off` by default, opt-in existing-comment metadata capture, advisory publish-plan artifacts and summaries, and inline GitHub comment create/update calls only when `RIPR_COMMENT_MODE=inline` and the safe plan permits publishing. |
| `docs/pr-inline-comment-publisher-workflow` | done | Documented `off`, `plan`, and `inline` rollout modes, publish-plan review, review-thread noise controls, forks, missing permissions, dedupe/upsert behavior, rollback, and the advisory gate boundary. |
| `dogfood/pr-inline-comment-publisher-receipts` | done | Extended `cargo xtask dogfood` with checked repo-local receipts for publishable, summary-only, capped, dedupe/upsert, stale-existing, fork or no-token, and missing-input publish plans without posting real PR comments. |
| `campaign/pr-inline-comment-publisher-closeout` | done | Closed Campaign 26 after the spec, fixtures, read-only publish plan, explicit generated-CI opt-in wiring, workflow docs, dogfood receipts, and validation showed inline comments are safe, capped, deduped, advisory, and disabled by default. |

References:

- [PR inline comment publisher proposal](PR_INLINE_COMMENT_PUBLISHER_PROPOSAL.md)
- [RIPR-SPEC-0025: PR inline comment publisher](specs/RIPR-SPEC-0025-pr-inline-comment-publisher.md)
- [RIPR-SPEC-0012: PR test guidance annotations](specs/RIPR-SPEC-0012-pr-test-guidance.md)
- [PR review guidance](PR_REVIEW_GUIDANCE.md)
- [PR review front panel workflow](PR_REVIEW_FRONT_PANEL_WORKFLOW.md)
- [Report packet index workflow](REPORT_PACKET_INDEX_WORKFLOW.md)
- [CI](CI.md)

Blocking conditions:

- analyzer behavior changes or recommendation ranking changes
- gate policy, waiver, suppression, or baseline semantic changes
- LSP/editor, provider, mutation, source-edit, generated-test, branch
  protection, or default-blocking changes
- inline comments posted by default
- `summary_only` guidance posted as inline comments
- comments placed on unchanged or unsafe lines
- duplicate durable comments across reruns
- free-form LLM review comments
- `pull_request_target` or unproven fork-permission behavior

Next:

- Campaign 26 is closed. Campaign 27 (Language Adapter Preview) is the
  explicit next product lane. Do not fold PR summary polish, comment-policy
  extensions, analyzer, ranking, gate policy, editor, platform, release,
  dependency, or MSRV work into this closeout.

## Campaign 27: Language Adapter Preview

Campaign ID: `language-adapter-preview`

Status: closed

Campaigns 1 through 26 built a credible Rust static oracle-gap analyzer with
an editor evidence loop, an advisory PR review front panel, baselines, RIPR
Zero status, an assistant proof loop, first useful action, a report packet
index, and an opt-in inline comment publisher. Every surface is
language-neutral by intent but single-language by accident. The next
adoption gap is mixed-language repositories that want the same evidence
surface across Rust, TypeScript/JavaScript, and Python without forking RIPR
into separate tools, separate schemas, or separate UX.

Objective:

```text
Introduce a language-neutral analysis adapter boundary inside the existing
`crates/ripr` package. Keep Rust as the reference adapter. Add syntax-first
TypeScript and Python preview adapters that feed the same RIPR domain,
output, LSP, agent, and Lane 4 review surfaces. The adapter boundary must
preserve current Rust behavior, fixtures, and goldens; the output schema
must gain only additive optional `language` and `language_status` fields;
preview adapters must be opt-in via `[languages]` repo configuration and
labeled `preview` in every public surface; preview adapters must report
explicit static limits instead of guessing; generated CI stays Rust-default
and advisory; Rust analyzer behavior, recommendation ranking, gate
semantics, LSP/editor behavior for Rust seams, mutation execution, provider
behavior, source files, generated tests, branch protection,
`pull_request_target` defaults, and default CI blocking do not change.
```

End state:

- `LanguageId`, `LanguageAdapter`, `LanguageFacts`, `OwnerFact`, `TestFact`,
  `AssertionFact`, `RelatedTest`, `FlowSink`, `Probe`, and `StaticLimit` are
  language-neutral domain or analysis types
- Rust fact extraction sits behind `RustAdapter` with no observable
  fixture, golden, or output schema change
- existing reports carry additive optional `language` and `language_status`
  fields without forking schemas
- TypeScript/JavaScript preview adapter emits syntax-first owners, tests,
  assertions, related tests, probes, and explicit static limits
- Python preview adapter emits syntax-first owners, tests, assertions,
  related tests, probes, and explicit static limits
- repo configuration adds `[languages] enabled = ["rust"]` as the default,
  with explicit opt-in to enable preview adapters
- VS Code extension language selectors cover Rust plus TypeScript,
  TypeScript React, JavaScript, JavaScript React, and Python once preview
  adapters are enabled, without changing saved-workspace defaults
- generated GitHub CI groups advisory summaries by language only when
  `[languages]` declares more than Rust, and Rust-default behavior is
  unchanged
- fixtures and `cargo xtask dogfood` receipts cover at least one TypeScript
  and one Python preview scenario plus the language router and static-limit
  cases
- the workspace remains one published package, one binary, one library, one
  LSP server, and one VS Code extension

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/language-adapter-preview-contract` | done | Landed the proposal/spec set: RIPR-PROP-0001 records design intent and alternatives; RIPR-SPEC-0026 pins the language-adapter boundary, additive optional output metadata, `[languages]` opt-in, preview labeling, and static-limit vocabulary; RIPR-SPEC-0027 and RIPR-SPEC-0028 pin the TypeScript and Python per-language static-fact contracts. |
| `analysis/language-adapter-boundary` | done | Introduced `LanguageId`, `LanguageAdapter`, and the language router inside `crates/ripr/src/analysis/` without changing Rust fixture, golden, or output schema behavior. The `RustAdapter` reference type is wired into workspace discovery as a functional no-op so the seam is alive. |
| `analysis/rust-adapter-behind-boundary` | done | Moved Rust fact extraction behind the `LanguageAdapter` trait as the reference adapter while preserving every existing Rust fixture, golden, capability, and output contract. Pipeline orchestration loads diffs, dispatches `analyze_diff`/`analyze_repo` to the adapter, and applies the language-neutral sort + summary on returned `Finding`s. |
| `output/language-metadata` | done | Added additive optional `language` field on each `Finding` (Rust adapter sets `"rust"`, omitted otherwise) and `language_status` (preview adapters set `"preview"`; omitted for Rust per spec). `LanguageId`/`LanguageStatus` moved to `domain::language` as pure-data enums so output renderers can serialize without depending on the analysis layer. JSON renderer emits the fields when populated; goldens refreshed accordingly. `owner_kind` and `static_limit_kind` remain deferred until preview adapters populate them. |
| `config/languages-opt-in` | done | Added `[languages] enabled` to `ripr.toml` with default `["rust"]`, vocabulary validation (rust/typescript/python), duplicate rejection, `deny_unknown_fields`, and a doctor-command surface. Generated `ripr init` config, `ripr.toml.example`, and `docs/CONFIGURATION.md` updated. |
| `analysis/typescript-preview-adapter` | done | Add the TypeScript/JavaScript syntax-first adapter, the TypeScript fixture corpus, and the preview labeling without changing Rust behavior, default CI, or inline-comment defaults. Scaffold sub-slice landed `oxc_parser`, the `TypeScriptAdapter` struct, and language-aware dispatch through `[languages] enabled`. Owner+test sub-slice (#777) added top-level function-declaration + `test()`/`it()` extraction, related-test matching by name reference, a minimal `WeaklyExposed`/`NoStaticPath` classifier, and `fixtures/typescript_boundary_gap/`. Assertion-shape extraction (#781 closes #767) refined the gradient to `Exposed`/`WeaklyExposed`/`NoStaticPath` via `toBe`/`toEqual`/`toThrow` plus async `.resolves`/`.rejects`. Probe-shape classification (#784 closes #768) replaced the placeholder `Predicate`/`Control` default with syntax-first `ProbeFamily`/`DeltaKind` per changed line, with `fixtures/typescript_return_value_shape/`. Mocked-module static-limit reporting (#791 closes #769) surfaces `vi.mock(...)`/`jest.mock(...)` via `evidence`/`missing` text without downgrading classification, with `fixtures/typescript_mocked_module_limit/`. Together these slices establish the first useful TypeScript preview loop end-to-end. Remaining preview polish — structured `static_limit_kind` field, additional limit kinds, arrow-function const owners, class methods — is intentionally deferred to follow-up issues and is not on the Campaign 27 critical path. Targets 0.6.0. |
| `analysis/typescript-editor-readiness` | done | Resolved the TypeScript preview gaps that made editor projection unsafe before `lsp/editor-language-routing` can consume TypeScript evidence: #779 human output visibly labels preview TypeScript evidence, #780 owner matching is file-scoped before line-range matching, #782 broad `toThrow()` remains weak broad-error evidence, #785 awaited `Promise.reject(...)` classifies as error-path preview evidence, and #786 fixture/golden evidence covers every TypeScript probe family currently emitted by the preview adapter. Landed without VS Code selector, LSP routing, source edit, generated test, provider, mutation execution, gate, or default-blocking behavior changes. |
| `adr/python-parser-substrate` | done | Pin the Python parser-substrate decision before any Cargo dependency or adapter code lands, mirroring how ADR 0008 (`oxc_parser`) was sequenced ahead of the TypeScript scaffold. Landed via #794 (closes #770) — adds `docs/adr/0009-python-parser-substrate.md`, registers it in `docs/adr/README.md`, and adds the approved-decision comment to `policy/dependency_allowlist.txt`. ADR 0009 was superseded in-place by a follow-up correction PR after discovering the originally-picked `ruff_python_parser` is `publish = false` in the astral-sh/ruff workspace and unavailable on crates.io; the corrected pick is `rustpython-parser`, the documented natural fallback already named under the ADR's Revisit Criteria. No Cargo dependency, no adapter code, no behavior change. |
| `analysis/python-preview-adapter` | done | Add the Python syntax-first adapter, the Python fixture corpus, and the preview labeling without changing Rust behavior, default CI, or inline-comment defaults. Scaffold sub-slice mirrors the TypeScript scaffold (PR #759): adds the `rustpython-parser` Cargo dependency approved by the corrected ADR 0009, the `PythonAdapter` type, language-aware pipeline dispatch through `[languages] enabled`, and a real production parse-validation use of `rustpython-parser`. To avoid an LGPL-3.0-only transitive (`malachite-bigint`), the dependency disables default features and selects `num-bigint` (MIT/Apache-2.0) for Python int-literal representation. Owner/test sub-slice adds syntax-first `def` / `async def` / method owner facts, pytest and unittest test discovery, preview `owner_kind` output, and Python owner/test fixture families. Assertion/oracle sub-slice adds syntax-first pytest, unittest, boolean, broad-error, and mock-call oracle facts plus fixture families. Probe-shape sub-slice adds syntax-first predicate, return-value, error-path, field-assignment, call, and mock-initializer probe families plus fixture coverage. Related-test sub-slice adds direct-call, import-alias, same-stem, and unrelated-mention fixtures with conservative weak proximity behavior. Static-limit sub-slice adds structured `dynamic_dispatch`, `decorator_indirection`, `mocked_module`, `missing_import_graph`, `metaprogramming`, and `unsupported_syntax` facts plus fixture coverage while preserving strong related-test classifications. Final corpus-completion audit records that owner/test/assertion/probe/related-test/static-limit Python preview facts are fixture-backed and editor-projectable. |
| `lsp/editor-language-routing` | done | Extended the VS Code extension activation events and document selector to TypeScript, TypeScript React, JavaScript, JavaScript React, and Python while preserving Rust saved-workspace defaults. Routed stale-buffer guards through the same supported-language selector set; preview findings now carry language, status, owner, and static-limit metadata through LSP diagnostic data, hover shows the preview syntax-first/advisory boundary before RIPR evidence, and status text surfaces preview/static-limit counts from refresh logs. LSP analysis remains config-gated by `[languages]` through the existing adapter layer. Landed without analyzer behavior changes, source edits, generated tests, provider calls, mutation execution, policy/gate/default-blocking behavior, CodeLens, inlay hints, semantic tokens, or unsaved-buffer overlays. |
| `ci/language-aware-grouping` | done | Generated GitHub CI summaries now read enabled languages through the public `ripr doctor` surface, keep the language grouping section hidden for Rust-only config, and group TypeScript/Python advisory artifact entries, preview-status counts, classifications, and static-limit kinds only when preview adapters are configured. Preview groups remain advisory presentation only; `ripr gate evaluate` remains configured pass/fail authority. |
| `docs/language-adapter-preview-workflow` | done | `docs/LANGUAGE_ADAPTER_PREVIEW.md` documents enabling preview adapters, reading mixed-language reports, interpreting preview labels, the static-limit boundary, editor projection, generated-CI language grouping, gate authority, and rollback; Quickstart, Configuration, Support Tiers, capability, traceability, and documentation-index surfaces link to it. |
| `dogfood/language-adapter-preview-receipts` | done | `cargo xtask dogfood` now checks TypeScript and Python preview receipts for preview labels, structured static limits, disabled-language behavior, and no cross-language related-test routing. The dogfood report records generated-CI preview grouping as checked while preserving advisory defaults, Rust-default behavior, gate authority, and inline-comment defaults. |
| `campaign/language-adapter-preview-closeout` | done | Closed after the spec, adapter boundary, Rust adapter, output metadata, preview adapters, editor routing, CI grouping, workflow docs, dogfood receipts, closeout handoff, and validation showed preview adapters are syntax-first, opt-in, advisory, and label-correct, while Rust behavior is preserved. |

References:

- [RIPR-PROP-0001: Multi-Language Adapter Preview](proposals/RIPR-PROP-0001-multi-language-adapter-preview.md)
- [RIPR-SPEC-0026: Language adapter contract](specs/RIPR-SPEC-0026-language-adapter-contract.md)
- [RIPR-SPEC-0027: TypeScript preview static facts](specs/RIPR-SPEC-0027-typescript-preview-static-facts.md)
- [RIPR-SPEC-0028: Python preview static facts](specs/RIPR-SPEC-0028-python-preview-static-facts.md)
- [RIPR-PROP-0003: Editor Preview Routing](proposals/RIPR-PROP-0003-editor-preview-routing.md)
- [RIPR-SPEC-0036: Editor Preview Routing](specs/RIPR-SPEC-0036-editor-preview-routing.md)
- [RIPR-SPEC-0037: Editor Preview Static-Limit Projection](specs/RIPR-SPEC-0037-editor-preview-static-limit-projection.md)
- [ADR 0011: Editor Preview Routing Is Projection-Only](adr/0011-editor-preview-routing-is-projection-only.md)
- [Lane 3 editor preview routing plan](../plans/campaign-27/lane3-editor-preview-routing.md)
- [Repo tracking model](REPO_TRACKING_MODEL.md)
- [Architecture](ARCHITECTURE.md)
- [Static exposure model](STATIC_EXPOSURE_MODEL.md)
- [Output schema](OUTPUT_SCHEMA.md)
- [Roadmap](ROADMAP.md)

Blocking conditions:

- Rust analyzer behavior, recommendation ranking, gate semantics, LSP/editor
  behavior for Rust seams, mutation execution, provider behavior, source
  files, generated tests, branch protection, `pull_request_target` defaults,
  or default CI blocking change
- output schema versions change instead of gaining additive optional fields
- preview adapters run by default
- preview adapters claim parity or adequacy with Rust
- preview adapters depend on a runtime typechecker, build graph, or other
  external tool by default
- workspace splits into a second published crate, binary, LSP server, or
  editor extension
- preview adapters introduce a second JSON schema, second SARIF rule set,
  or second LSP server
- preview adapters reinterpret the existing exposure vocabulary
- preview adapters bypass the `unsafe_code = "forbid"`, panic-family,
  allow-attribute, dependency, process, or network policy rails

Commands:

```bash
cargo xtask check-spec-format
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-output-contracts
cargo xtask check-architecture
cargo xtask check-workspace-shape
cargo xtask check-public-api
cargo xtask check-file-policy
cargo xtask check-dependencies
cargo xtask check-pr
```

Next:

- Campaign 27 is closed and archived. Campaign 28 (First Useful PR Loop) is
  also closed and archived. Keep TypeScript and Python preview evidence
  opt-in, visibly preview/advisory, and outside default gate authority until a
  later promotion policy explicitly changes that boundary.

## Campaign 28: First Useful PR Loop

Campaign ID: `first-useful-pr-loop`

Status: closed

RIPR now has the structural repair loop: first-pr packets, actionable gaps,
ranked repair packets, dry-run attempt context, receipts, outcomes, generated
CI, editor packets, and agent handoffs. The next adoption gap is not another
abstract control plane. The next gap is making one Rust PR feel obvious,
useful, and safe for a new user who has not learned RIPR's internal artifact
graph.

Objective:

```text
Make a new user, reviewer, or coding agent get from one changed Rust behavior
to one trustworthy repair receipt with almost no interpretation burden.
```

End state:

- `ripr first-pr --root . --base origin/main --head HEAD` is the obvious
  front door for one changed Rust PR
- the first screen names one top repairable gap or a clear no-action state
  without making users open secondary reports
- the recommendation explains changed behavior, current proof weakness,
  missing discriminator, repair intent, verify command, receipt command, and
  static-evidence boundary
- `ripr outcome` receipts are reviewer-native and explain before/after
  movement without claiming mutation, runtime, or coverage proof
- a tiny fixture or demo story proves the before -> recommendation -> focused
  repair -> outcome -> receipt loop
- generated CI, editor, and agent packet surfaces mirror the same one-screen
  repair story instead of inventing parallel wording
- support/status claims remain mapped to proof and preserve advisory/static
  boundaries

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `context/proof-stack-reconciliation` | done | Reconciled proof-stack language into RIPR's existing context system, accepted source-of-truth stack, and roadmap end-goal framing without adding a runner-local goals namespace or another operating namespace. |
| `goals/active-freshness-validation` | done | Goal validation now rejects a closed active campaign unless it declares a successor or explicit no-current-goal marker, so agents cannot silently continue from stale execution state. |
| `first-pr/front-door-polish` | done | `ripr first-pr` now writes a read-only preflight section for root, Git/base/head/diff, Cargo workspace, config defaults, output path, mode, and next-command guidance while preserving advisory artifact selection. |
| `first-pr/one-screen-recommendation` | done | Start-here now has a golden-backed one-screen recommendation with top gap/no-action, changed behavior, current evidence strength, missing discriminator, focused proof intent, verify command, receipt command, receipt path, and static-advisory boundary. |
| `outcome/reviewer-native-receipts` | done | `ripr outcome` now writes reviewer-native JSON/Markdown receipt sections covering before flags, focused proof signals, movement after verification, remaining weak/unknown seams, and reviewer claim boundaries. |
| `fixtures/first-pr-boundary-gap-demo` | done | Added a checked boundary-gap demo story for before -> `ripr first-pr` -> focused external proof -> `ripr outcome` -> reviewer receipt. |
| `surfaces/one-screen-loop-convergence` | done | Generated CI, VS Code/editor handoff, and agent packet surfaces now mirror changed behavior, missing discriminator, focused proof intent, verify and receipt commands, receipt artifacts, and the static-advisory boundary. |
| `campaign/first-useful-pr-loop-closeout` | done | Closed Campaign 28 with a handoff, archived active goal manifest, proof commands, claim and policy boundary, remaining limits, and no selected successor goal. |

References:

- [README](README.md)
- [Quickstart](QUICKSTART.md)
- [Context system](agent-context/CONTEXT_SYSTEM.md)
- [First Useful PR Loop plan](../plans/first-useful-pr-loop/implementation-plan.md)
- [Scoped PR contract](SCOPED_PR_CONTRACT.md)
- [Support tiers](status/SUPPORT_TIERS.md)
- [Agent workflows](AGENT_WORKFLOWS.md)

Blocking conditions:

- source edits generated by RIPR
- generated tests
- provider/model calls
- runtime mutation execution
- runtime, coverage, or correctness proof claims
- default blocking gates or public badge semantic changes
- TypeScript/Python preview promotion
- a parallel runner-local goals namespace or source-of-truth operating model
  outside the accepted `docs/source-of-truth/` stack

Commands:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-pr
```

Next:

- Campaign 28 is closed and archived. `.ripr/goals/active.toml` records
  `no_current_goal = true` until a successor campaign is selected from the
  roadmap or a new accepted source-of-truth stack.

## Focused Lane 1 Tracker: Evidence Quality Leadership

Status: closed in documented scope. Campaign 28 is also closed and archived;
`.ripr/goals/active.toml` records `no_current_goal = true` until a successor is
selected. This focused Lane 1 tracker is not the active execution manifest.

Sources of truth:

- [Lane 1 Evidence Quality Leadership tracker](lanes/LANE_1_EVIDENCE_QUALITY_LEADERSHIP.md)
- [RIPR-PROP-0002](proposals/RIPR-PROP-0002-lane-1-evidence-quality-leadership.md)
- [RIPR-SPEC-0034](specs/RIPR-SPEC-0034-evidence-quality-scorecard.md)
- [RIPR-SPEC-0035](specs/RIPR-SPEC-0035-evidence-quality-benchmark-corpus.md)
- [RIPR-SPEC-0040](specs/RIPR-SPEC-0040-static-runtime-confidence-expansion.md)
- [ADR 0010](adr/0010-fixture-first-evidence-confidence.md)
- [closeout handoff](handoffs/2026-05-13-lane-1-evidence-quality-leadership-closeout.md)

Objective:

```text
Make RIPR self-aware about evidence quality: what it believes, why it believes
it, which fixture or calibration class supports that belief, what remains
unknown, and which evidence-class repair should happen next.
```

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `report/evidence-quality-scorecard` | done | #850 added the repo-local scorecard over the Lane 1 audit. |
| `fixtures/evidence-quality-benchmark-corpus` | done | #851 added the manifest-only benchmark corpus with positive cases and must-not-claim guards. |
| `analysis/static-limitation-taxonomy` | done | #861 normalized static limitation categories and repair routes without turning limitations into user test gaps. |
| `analysis/oracle-semantics-audit-fixes` | done | #871 tightened clear custom helper, opaque helper, and duplicative equality semantics from audit-backed cases. |
| `calibration/runtime-fixtures-v3` | done | #881 added checked imported-runtime calibration classes while keeping runtime-only signal from creating static gaps. |
| `report/evidence-quality-trend` | done | #885 added scorecard/audit snapshot trend reporting with explicit no-history states. |
| `campaign/evidence-quality-leadership-closeout` | done | Closed after scorecard, benchmark corpus, two audit-driven improvements, runtime-fixtures-v3, trend reporting, class-scoped capabilities, traceability, and handoff proof. |

Future Lane 1 work should open only when the scorecard, audit, or a documented
consumer requirement identifies a new measured evidence class. Do not reopen
Lane 1 for PR/CI front-panel work, LSP/editor polish, gate policy, generated
tests, provider calls, mutation execution, or score redefinition.

## Focused Lane 1 Tracker: User-Visible Output Evidence

Status: closed as a focused Lane 1 tracker. Campaign 28 is also closed and
archived; `.ripr/goals/active.toml` records `no_current_goal = true` until a
successor is selected. This focused tracker is not the active execution
campaign.

Sources of truth:

- [Lane 1 User-Visible Output Evidence tracker](lanes/LANE_1_USER_VISIBLE_OUTPUT_EVIDENCE.md)
- [RIPR-PROP-0005](proposals/RIPR-PROP-0005-user-visible-output-evidence.md)
- [RIPR-SPEC-0043](specs/RIPR-SPEC-0043-presentation-text-evidence.md)
  presentation text evidence
- [RIPR-SPEC-0045](specs/RIPR-SPEC-0045-finding-to-gap-alignment.md)
  finding-to-gap alignment
- [ADR 0010](adr/0010-fixture-first-evidence-confidence.md)
- [Lane 1 User-Visible Output Evidence closeout](handoffs/2026-05-14-user-visible-output-evidence-closeout.md)

Objective:

```text
Make changed presentation/help/report/table text one evidence-quality-aware
action, no-action state, or static limitation instead of raw duplicate
line-local notices.
```

End state:

- changed presentation text is a distinct evidence class;
- visibility is `user_visible`, `internal_only`, or `unknown`;
- observer shape is snapshot, CLI help output, report render, table render,
  golden output, none, or unknown;
- declaration and literal raw seams group into one canonical evidence item;
- raw line-local findings remain supporting evidence and roll up into canonical
  evidence items with explicit state, actionability, repair, confidence, and
  proof;
- actionability distinguishes snapshot/help-output/report test, already
  observed, internal no-action, and static limitation states;
- scorecard or trend fields track presentation-text evidence quality;
- downstream lanes receive a consumer contract but no rendering change lands in
  this Lane 1 tracker.

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `docs/proposal-user-visible-output-evidence` | done | #904 opened RIPR-PROP-0005 and the Lane 1 tracker. |
| `docs/spec-presentation-text-evidence` | done | #909 added RIPR-SPEC-0043 for visibility, observer, actionability, canonical grouping, static limitation, and must-not-claim behavior. |
| `docs/spec-finding-to-gap-alignment` | done | #927 defined raw finding to canonical evidence item alignment before behavior changes. |
| `fixtures/finding-alignment-benchmark` | done | #931 pinned grouping, no-action, already-observed, static limitation, and actionable raw-to-canonical cases. |
| `fixtures/presentation-text-evidence-benchmark` | done | #900 added the screenshot-derived benchmark after the proposal/spec foundation. |
| `analysis/finding-alignment-evidence-fields` | done | #935 added additive `raw_findings[]`, `canonical_item`, and nullable `presentation_text` fields to `evidence_record` without changing rendering, gates, scores, generated tests, provider calls, or mutation execution. |
| `analysis/presentation-text-evidence-fields` | done | #935 reserved nullable evidence-record fields for the class. |
| `analysis/presentation-text-canonical-grouping` | done | #943 groups supported presentation-text constant declaration plus adjacent literal raw findings into one visibility-unknown canonical limitation item in `ripr check --json`. |
| `analysis/presentation-text-visibility` | done | #947 classifies fixture-backed help/report/internal output evidence as actionable, observed, internal-only, or visibility-unknown while keeping unsupported routes as limitations. |
| `analysis/presentation-text-actionability` | done | #951 extended repair routes with concrete repair kind, target test type, and suggested assertion fields beyond the initial visibility states. |
| `report/presentation-text-scorecard-trend-fields` | done | #957 reports presentation-text quality counts and deltas in scorecard and trend output. |
| `docs/presentation-text-consumer-handoff` | done | #959 documents downstream rendering contract without changing PR/CI or editor surfaces. |
| `campaign/user-visible-output-evidence-closeout` | done | #966 records proof, remaining unknowns, final observer-unknown benchmark guard, and the next repair boundary for this focused Lane 1 evidence class. |

Blocking conditions:

- PR/CI rendering changes
- LSP/editor polish
- gate-policy changes or default blocking
- generated tests or source edits
- provider/model calls
- mutation execution
- score redefinition
- user-visible claims inferred through opaque helpers or unsupported output
  paths

Commands:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

Closeout:

- [Lane 1 User-Visible Output Evidence closeout](handoffs/2026-05-14-user-visible-output-evidence-closeout.md)
  records the PR chain, proof, remaining limitations, and boundary that
  downstream PR/CI and editor rendering work belongs to the owning lanes.

## Focused Lane 1 Tracker: Finding Alignment Burn-Down

Tracker ID: `lane1-finding-alignment-burndown`

Status: active execution rail. `.ripr/goals/active.toml` now selects this
focused tracker after Start-Here Surface Convergence closed with
`no_current_goal = true`.

Sources of truth:

- [Lane 1 Finding Alignment Burn-Down tracker](lanes/LANE_1_FINDING_ALIGNMENT_BURNDOWN.md)
- [Lane 1 Finding Alignment Burn-Down implementation plan](../plans/lane1-finding-alignment-burndown/implementation-plan.md)
- [Lane 1 Shippable Finding Alignment closeout](handoffs/2026-05-17-lane1-shippable-finding-alignment-closeout.md)
- [Finding Alignment Consumer Contract v2](handoffs/2026-05-16-finding-alignment-consumer-contract-v2.md)
- [RIPR-SPEC-0045](specs/RIPR-SPEC-0045-finding-to-gap-alignment.md)
  finding-to-gap alignment
- [RIPR-SPEC-0048](specs/RIPR-SPEC-0048-config-policy-constant-evidence.md)
  config and policy constant evidence

Objective:

```text
Keep RIPR operating on canonical evidence items instead of raw findings as new
alignment gaps are measured, without reopening completed presentation-text or
config/policy base scope.
```

End state:

- alignment coverage by evidence class is auditable;
- canonical items have placement and supporting-span evidence where safe;
- top named static limitation buckets become fixture-backed repair queues;
- config/policy unsupported-flow expansion is scoped by spec and fixtures;
- actionable canonical items preserve repair-route and verify-command coverage;
- internal scorecards keep actionable canonical gaps as the leading work count;
- runtime confidence coverage is visible by canonical evidence class;
- dogfood and downstream handoff docs refresh only when material burn-down
  deltas land.

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `report/finding-alignment-coverage-audit` | done | [swarm #229](https://github.com/EffortlessMetrics/ripr-swarm/issues/229) / [source #1140](https://github.com/EffortlessMetrics/ripr/issues/1140) audits aligned, unaligned, duplicate, unnamed-limitation, missing-repair, and missing-verify queues by evidence class. |
| `analysis/named-static-unknown-invariant` | done | [swarm #233](https://github.com/EffortlessMetrics/ripr-swarm/issues/233) / [source #1141](https://github.com/EffortlessMetrics/ripr/issues/1141) preserves named static limitations for user-facing static unknowns. |
| `analysis/canonical-primary-anchor-raw-spans` | open | [#1158](https://github.com/EffortlessMetrics/ripr/issues/1158) completes placement and supporting raw-span evidence for canonical items. |
| `analysis/top-static-limitation-bucket-burndown` | done | [swarm #238](https://github.com/EffortlessMetrics/ripr-swarm/issues/238) / [source #1159](https://github.com/EffortlessMetrics/ripr/issues/1159) burned down the sampled `call_presence` / `activation_owner_call_unresolved` bucket with fixture-backed positive and must-not-claim coverage. |
| `docs/spec-config-policy-unsupported-flow-expansion` | done | [swarm #241](https://github.com/EffortlessMetrics/ripr-swarm/issues/241) / [source #1142](https://github.com/EffortlessMetrics/ripr/issues/1142) selected `opaque_config_lookup` as the next fixture-backed expansion while keeping generated, macro, dynamic-dispatch, and unsupported cross-file flows as named limitations. |
| `fixtures/config-policy-unsupported-flow-burndown` | done | [swarm #246](https://github.com/EffortlessMetrics/ripr-swarm/issues/246) / [source #1143](https://github.com/EffortlessMetrics/ripr/issues/1143) pinned macro-generated config/schema output and dynamic config dispatch as named limitation benchmark cases before analyzer work. |
| `analysis/config-policy-unsupported-flow-support` | done | [swarm #250](https://github.com/EffortlessMetrics/ripr-swarm/issues/250) / [source #1144](https://github.com/EffortlessMetrics/ripr/issues/1144) landed in swarm #252; fixture-backed `opaque_config_lookup` moved out of limitation while unsupported flows stayed named. |
| `analysis/actionable-repair-route-completeness` | done | [swarm #254](https://github.com/EffortlessMetrics/ripr-swarm/issues/254) / [source #1145](https://github.com/EffortlessMetrics/ripr/issues/1145) landed in swarm #257; config-policy repair-route coverage now requires the same top-level structured `repair_route` predicate as the overall actionable summary. |
| `analysis/actionable-verify-command-coverage` | done | [swarm #258](https://github.com/EffortlessMetrics/ripr-swarm/issues/258) / [source #1146](https://github.com/EffortlessMetrics/ripr/issues/1146) landed in swarm #261; config/policy verify coverage now rejects missing sentinels and benchmark fixtures require concrete verify commands for actionable records. |
| `report/actionable-canonical-gaps-scorecard-lead` | ready | [swarm #262](https://github.com/EffortlessMetrics/ripr-swarm/issues/262) / [source #1147](https://github.com/EffortlessMetrics/ripr/issues/1147) preserves scorecard-leading actionable canonical gaps. |
| `calibration/runtime-confidence-coverage-audit` | open | [#1160](https://github.com/EffortlessMetrics/ripr/issues/1160) reports calibrated-supported versus static-only canonical items by class. |
| `dogfood/finding-alignment-examples-refresh` | open | [#1149](https://github.com/EffortlessMetrics/ripr/issues/1149) refreshes examples only after material burn-down deltas. |
| `docs/canonical-alignment-contract-refresh` | open | [#1153](https://github.com/EffortlessMetrics/ripr/issues/1153) refreshes downstream handoff docs only when fields or guidance change. |

Blocking conditions:

- PR/CI rendering changes
- inline PR comment publishing
- LSP/editor polish
- gate-policy or default-blocking changes
- public badge or score redefinition
- generated tests
- automatic source edits
- provider/model calls
- mutation execution
- treating named static limitations as user test debt

Commands:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

## Focused Lane 2 Tracker: Policy Readiness and Preview Evidence Governance

Tracker ID: `policy-readiness-preview-evidence-governance`

Status: tracker

GitHub issue: [#755](https://github.com/EffortlessMetrics/ripr/issues/755)

Campaign 28 is closed and archived in the machine-readable manifest. This
focused Lane 2 tracker is not a replacement for `.ripr/goals/active.toml`; it
records the policy boundary that Campaign 27 and later policy work must not
cross.

Objective:

```text
Make RIPR policy decisions auditable across stable Rust evidence and preview
language-adapter evidence. Preserve advisory defaults, keep preview findings
visible but non-gating by default, and define when evidence is eligible for
baseline, waiver, suppression, calibration, RIPR Zero, and gates.
```

End state:

- policy readiness defines which mode is safe for a repo right now
- preview-language evidence is visible and advisory by default
- preview-language evidence is not gate-eligible, RIPR Zero blocking debt, or
  mutation-calibrated confidence unless a later explicit policy promotes it
- waivers remain visible acknowledgements
- suppressions remain durable policy exceptions with owner, reason, scope, and
  review state
- baselines remain adoption checkpoints, not acceptance forever
- generated CI can surface readiness artifacts only as advisory evidence

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `spec/policy-readiness-report` | done | RIPR-SPEC-0029 defines the read-only policy readiness report answering which mode is safe for the repo right now, including statuses, inputs, fields, warnings, preview-boundary health, and no-mutation/no-gate authority. |
| `spec/preview-evidence-policy-boundary` | done | RIPR-SPEC-0030 specifies that TypeScript and Python preview evidence is visible/advisory by default, carries preview/static-limit labels, and is not gate, RIPR Zero, default baseline-check, or mutation-calibrated confidence eligible without later explicit promotion. |
| `report/policy-readiness` | done | `ripr policy readiness` writes policy-readiness JSON and Markdown over explicit existing artifacts only, with independent readiness axes and preview-evidence zero-count boundaries, without posting, source edits, hidden analysis, baseline mutation, gate execution, or CI failure authority. |
| `report/waiver-aging` | done | `ripr policy waiver-aging` writes advisory waiver-aging JSON and Markdown from current PR evidence ledgers plus optional JSONL history, keeping repeated waiver visible as a repair or policy-review signal without pass/fail authority. |
| `policy/suppression-ledger-health` | done | `ripr policy suppression-health` writes advisory suppression-health JSON and Markdown over `.ripr/suppressions.toml`, flags missing owner, missing reason, stale review windows, overbroad scope, unknown selectors, and preview-language suppressions without `language_status = "preview"`, and keeps suppressed findings visible with `still_visible = true`. |
| `policy/baseline-refresh-guardrails` | done | Shrink-only `ripr baseline update --remove-resolved` remains the only refresh path, `--adopt-new` is rejected, generated CI is pinned to read-only `ripr baseline diff`, and docs state that CI never rewrites or auto-adopts baseline entries. |
| `policy/exception-ledger-convergence` | done | `docs/POLICY_ALLOWLISTS.md` now aligns no-panic, Clippy, non-Rust, workflow, RIPR suppression, baseline, and waiver ledgers around one reviewed reason per exception, semantic identity where available, and stale-entry behavior by class. |
| `docs/blocking-readiness-guide` | done | `docs/BLOCKING_READINESS.md` now uses policy readiness as the ceiling for advisory, visible-only, acknowledgeable, baseline-check, and calibrated-gate promotion, including calibration, baseline, waiver, suppression, and preview-evidence health. |
| `ci/policy-readiness-advisory-projection` | done | Generated CI writes, uploads, and summarizes waiver-aging, suppression-health, and policy-readiness artifacts as advisory-only projections: no pass/fail authority, no new required checks, no default blocking, and no comment posting. |
| `campaign/policy-readiness-closeout` | done | Closed the focused Lane 2 tracker after policy readiness, preview boundary, waiver aging, suppression health, baseline refresh guardrails, exception ledger semantics, blocking readiness guidance, and advisory CI projection landed. The closeout audit is recorded in `docs/handoffs/2026-05-12-policy-readiness-closeout.md`; Campaign 28 is now closed and archived, and `.ripr/goals/active.toml` records `no_current_goal = true`. |

References:

- [Policy readiness tracker](policy/POLICY_READINESS.md)
- [RIPR-SPEC-0029: Policy readiness report](specs/RIPR-SPEC-0029-policy-readiness-report.md)
- [RIPR-SPEC-0030: Preview evidence policy boundary](specs/RIPR-SPEC-0030-preview-evidence-policy-boundary.md)
- [Output schema: Policy readiness report](OUTPUT_SCHEMA.md#policy-readiness-report)
- [Output schema: Suppression health report](OUTPUT_SCHEMA.md#suppression-health-report)
- [Focused Lane 2 tracker manifest](../.ripr/goals/lane2-policy-readiness.toml)
- [Language Adapter Preview](#campaign-27-language-adapter-preview)
- [Calibrated gate policy](CALIBRATED_GATE_POLICY.md)
- [RIPR blocking readiness](BLOCKING_READINESS.md)
- [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md)
- [RIPR Zero reporting workflow](RIPR_ZERO_REPORTING_WORKFLOW.md)
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md)

Blocking conditions:

- analyzer behavior changes or recommendation ranking changes
- LSP/editor, PR summary rendering, provider, mutation, source-edit,
  generated-test, release, or security changes
- default CI blocking, new required checks, or comment posting
- automatic baseline adoption
- preview-language gate promotion without explicit later policy
- hidden runtime mutation or proof claims
- treating suppressions as invisible success or waivers as durable exceptions

Commands:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-pr
```

## Focused Lane 2 Tracker: Policy Operations and Promotion Readiness

Status: closed as a focused Lane 2 tracker. Campaign 28 is closed and archived
in the machine-readable manifest.

Sources of truth:

- [Policy operations tracker](policy/POLICY_OPERATIONS.md)
- [Focused Lane 2 policy operations manifest](../.ripr/goals/lane2-policy-operations.toml)
- [Policy readiness tracker](policy/POLICY_READINESS.md)

Objective:

```text
Make RIPR policy adoption operational. The policy layer should tell maintainers
what policy posture is safe now, what blocks stricter modes, what changed over
time, and what explicit promotion packet would be required before
baseline-check, calibrated-gate, or preview-language evidence promotion. All
outputs are read-only and advisory unless an existing explicit gate mode is
configured.
```

End state:

- maintainers can see the current safe policy ceiling
- maintainers can see the next safe policy action
- baseline, waiver, suppression, calibration, and preview-boundary blockers are
  named before stricter modes are recommended
- promotion packets make manual policy changes reviewable without mutating
  config
- preview promotion packets keep TypeScript and Python evidence visible but
  non-gating until explicit promotion evidence exists
- policy history shows whether readiness improved or regressed over time
- generated CI may surface advisory artifacts without pass/fail authority

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `campaign/policy-operations-tracker` | done | Opened `docs/policy/POLICY_OPERATIONS.md`, `.ripr/goals/lane2-policy-operations.toml`, roadmap, plan, and campaign references without behavior changes. Current `main` already uses `RIPR-SPEC-0034` through `RIPR-SPEC-0037`, so policy operations specs must use the next available IDs. |
| `spec/policy-operations-report` | done | RIPR-SPEC-0039 defines a read-only report composing policy-readiness, waiver-aging, suppression-health, baseline-delta, gate-decision, calibration, and preview-boundary inputs. |
| `policy/operations-report` | done | `ripr policy operations` writes JSON and Markdown over explicit existing artifacts only, with current ceiling, next safe action, promotion blockers, grouped actions, warnings, unknowns, and input artifact status. |
| `spec/policy-history-ledger` | done | RIPR-SPEC-0041 defines a read-only policy history report and optional append-only input without gates, telemetry, dashboards, required history files, or automatic appends. |
| `policy/history-report` | done | `ripr policy history` writes read-only JSON and Markdown trend packets over explicit policy operations and optional history JSONL inputs without automatic appends. |
| `spec/policy-promotion-packets` | done | RIPR-SPEC-0042 defines read-only promotion packets for `visible-only`, `acknowledgeable`, `baseline-check`, and `calibrated-gate` without config, baseline, suppression, workflow, CI, history, or preview-eligibility mutation. |
| `policy/promotion-packet-report` | done | `ripr policy promote --to ...` writes manual-review packets from policy operations and optional policy history without mutating config, baselines, suppressions, workflows, CI defaults, history, or preview eligibility. |
| `spec/preview-evidence-promotion-packet` | done | RIPR-SPEC-0044 defines preview-language promotion packets with default `allowed_now = false`, explicit required/supplied/missing evidence accounting, advisory generated-CI posture, rollback guidance, and no actual promotion. |
| `policy/preview-promotion-packet-report` | done | Added `ripr policy preview-promote --language ... --class ...` while preserving advisory preview defaults. |
| `docs/policy-operator-workflow` | done | `docs/POLICY_OPERATIONS_WORKFLOW.md` documents readiness, operations, history, promotion packets, preview packets, manual config review, post-change monitoring, and hard boundaries for maintainers. |
| `ci/policy-operations-advisory-projection` | done | Generated CI renders, uploads, indexes, and summarizes policy operations, history, promotion, and configured preview-promotion artifacts as advisory-only packets without pass/fail authority, required checks, comment posting, baseline mutation, config mutation, or default blocking. |
| `campaign/policy-operations-closeout` | done | Closed after operations, history, promotion packets, preview promotion packets, workflow, advisory CI projection, capability, metrics, traceability, and handoff surfaces landed; see `docs/handoffs/2026-05-13-policy-operations-closeout.md`. |

Blocking conditions:

- analyzer truth changes or evidence identity rewrites
- recommendation ranking changes
- LSP/editor behavior changes
- PR/CI front-panel redesign
- generated tests, provider calls, or mutation execution
- default CI blocking, required checks, or comment posting
- automatic config mutation, baseline adoption, or suppression creation
- preview-language gate promotion without explicit later policy
- runtime-proof claims from static evidence

Commands:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-campaign
cargo xtask check-pr
git diff --check
```

Closeout:

- [Policy Operations closeout](handoffs/2026-05-13-policy-operations-closeout.md)
  records the PR chain, prompt-to-artifact audit, validation plan, and the
  boundary that future policy promotion or preview promotion work should open
  explicitly.

## Focused Repo Operations Lane: Generated Evidence Discipline

Status: closed as a repo-operations lane. This lane does not replace the
machine-readable active campaign.

Objective:

```text
Make generated evidence, authored source-of-truth, deterministic repair,
judgment-required decisions, and review receipts mechanically distinct so
agents can prepare reviewable PRs without hand-editing generated trust markers
or relying on chat memory for process rules.
```

End state:

- ordinary PRs cannot carry generated badge endpoint diffs or target residue
  without an explicit generated-artifact refresh context
- public badge endpoint numbers are generated by `cargo xtask badges` or the
  Badge Endpoints workflow, not hand-authored
- `check-pr` stays non-mutating for committed badge endpoint files
- workers can run a worktree doctor before opening or updating PRs
- operators can produce board-level PR triage and single-PR merge-readiness
  reports without ad hoc polling
- spec numbering and campaign/source-of-truth drift have mechanical checks
- receipts, critic reports, and deterministic suggested-fixes patches stay
  generated under `target/ripr/`
- contributor docs explain which surfaces are authored truth, generated
  evidence, deterministic repair, and judgment-required decisions
- `cargo xtask pr-ready` gives agents one local PR readiness packet before
  opening or updating a PR
- `cargo xtask cockpit` gives maintainers one repo-level advisory action queue
  for board state, generated-evidence hygiene, source-of-truth checks, and next
  commands
- report packets have Markdown for humans and JSON for agents instead of
  requiring prose scraping

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `badge/generated-endpoint-workflow` | done | #874 added the Badge Endpoints workflow plus badge endpoint docs and commands. |
| `devex/generated-clean-check` | done | #930 added `cargo xtask check-generated-clean` and PR gate wiring for generated residue. |
| `badge/endpoint-ownership-policy` | done | #938 added `cargo xtask check-badge-diff-policy`, allowed README badge layout edits, and rejected ordinary endpoint JSON diffs. |
| `devex/check-pr-non-mutating-badges` | done | #938 routes PR validation through non-mutating badge checks instead of refreshing committed endpoint JSON. |
| `devex/worktree-doctor` | done | #941 added `cargo xtask worktree doctor` for dirty main, behind branches, generated residue, untracked sample targets, and broad diff warnings. |
| `docs/spec-numbering-helper` | done | #946 added `cargo xtask specs next` and `cargo xtask check-spec-numbering`. |
| `devex/pr-triage-report` | done | #950 added `cargo xtask pr-triage-report` for duplicate families, stale drafts, behind branches, validation gaps, and sensitive surfaces. Follow-up JSON output makes the same advisory queue packet agent-readable under `target/ripr/reports/pr-triage.json`. |
| `devex/gh-pr-status` | done | #952 added `cargo xtask gh-pr-status --pr <number>` with safe next action guidance. Follow-up JSON output makes the same merge-readiness packet agent-readable under `target/ripr/reports/gh-pr-status.json`. |
| `policy/lane2-reopening-triggers` | done | #958 documented when future policy authority changes must reopen explicit Lane 2 work. |
| `devex/campaign-source-of-truth-hardening` | done | #965 hardened focused-tracker, done-item command, spec, closeout, and active-manifest checks. |
| `automation/gate-receipts` | done | #55 already supplied target-local receipt commands; this lane treats them as generated evidence. |
| `automation/critic-report` | done | #84 already supplied the advisory critic report; this lane keeps it target-local and reviewer-focused. |
| `automation/suggested-fixes-patch` | done | #971 added deterministic `suggested-fixes.{patch,md}` output under `target/ripr/reports/`. |
| `docs/generated-evidence-discipline` | done | #975 added `docs/GENERATED_EVIDENCE.md` and linked contributor/automation docs. |
| `devex/command-mutability-catalog` | done | Adds `cargo xtask commands` to classify xtask commands by mutability, generated-output paths, external-state access, and judgment-required boundaries. |
| `campaign/generated-evidence-discipline-closeout` | done | Closed after the generated-clean, badge diff policy, worktree, triage, PR status, spec numbering, campaign hardening, receipt, critic, suggested-fixes, and docs surfaces aligned; see `docs/handoffs/2026-05-14-generated-evidence-discipline-closeout.md`. |
| `devex/pr-triage-json` | done | Commit `9cf2c039` added `target/ripr/reports/pr-triage.json` so the board-level advisory packet is agent-readable. |
| `devex/gh-pr-status-json` | done | #1011 added `target/ripr/reports/gh-pr-status.json` for single-PR merge readiness. |
| `reports/repo-ops-packet-index` | done | #1015 added repo-ops packet status to `cargo xtask reports index`, including command mutability, cockpit, PR-ready, worktree doctor, PR triage, merge readiness, generated-clean, badge policy, critic, receipts, suggested fixes, and check-pr artifacts. |
| `devex/check-command-catalog` | done | #1018 added `cargo xtask check-command-catalog` so new xtask commands cannot bypass mutability classification. |
| `devex/pr-ready-cockpit` | done | #1025 added `cargo xtask pr-ready`, writing `target/ripr/reports/pr-ready.{md,json}` as the local PR front door. |
| `devex/repo-cockpit` | done | #1035 added `cargo xtask cockpit`, writing `target/ripr/reports/cockpit.{md,json}` as the repo-level maintainer front door. |
| `docs/merge-watch-policy` | done | #1036 added `docs/MERGE_WATCH_POLICY.md` for polling cadence, branch-refresh decisions, REST fallback, Droid/advisory checks, and local merge limits. |
| `automation/suggested-fixes-expansion` | done | #1039, #1041, #1044, and #1053 expanded deterministic suggested fixes for docs index ordering, traceability ordering, capability ordering, and command catalog ordering while preserving judgment-required boundaries. |
| `devex/pr-triage-queue-disposition` | done | #1047 added advisory queue dispositions for merge candidates, stale/duplicate work, rebase needs, fresh-validation gaps, owner decisions, and wrong-lane work. |
| `campaign/repo-ops-ux-cockpit-closeout` | done | Closed after the front-door packet flow landed; see `docs/handoffs/2026-05-16-repo-ops-ux-cockpit-closeout.md`. |

Blocking conditions:

- analyzer semantics, evidence identity, or recommendation ranking changes
- LSP/editor behavior changes
- generated tests, provider calls, or mutation execution
- branch protection, default CI blocking, baseline adoption, suppression
  creation, or preview-language promotion
- manual badge endpoint number edits
- deterministic repair for judgment-required decisions

Commands:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-generated-clean
cargo xtask pr-ready
cargo xtask cockpit
cargo xtask check-campaign
cargo xtask check-pr
git diff --check
```

Closeout:

- [Generated Evidence Discipline closeout](handoffs/2026-05-14-generated-evidence-discipline-closeout.md)
- [Repo-Ops UX cockpit closeout](handoffs/2026-05-16-repo-ops-ux-cockpit-closeout.md)
  records the front-door follow-up PR chain, prompt-to-artifact audit,
  validation plan, and boundary that future repo-operations guardrails should
  open explicitly.

## Future Campaign: Editor Evidence UX

Campaign ID: `editor-evidence-ux`

Status: complete as an explicit parallel Lane 3 closeout. Campaign 17, RIPR
Zero Adoption, is complete; Editor Evidence UX closed without replacing that
machine-readable campaign.

The saved-workspace LSP path already has alpha diagnostics, evidence hover,
seam actions, related-test opening, context collection, agent-loop commands,
verify commands, receipt commands, and refresh/status surfaces. The next editor
product risk is not existence; it is making the evidence loop feel like the
right way to work in the editor.

Objective:

```text
Make RIPR's saved-workspace LSP path project one evidence spine from
diagnostic to hover, related test, focused context packet, one test, verify,
and receipt without automatic edits, generated tests, runtime mutation
execution, or runtime adequacy claims.
```

End state:

- diagnostics carry stable seam identity and are not reinterpreted from message
  text
- hover is the primary human explanation surface for the seam class, evidence
  path, missing observation, related test, suggested assertion shape, verify
  command, receipt command, and static limits
- code actions appear only when the supporting evidence or command context
  exists
- a canonical evidence context packet command gives external agents one bounded
  work packet without coupling RIPR to an LLM provider
- protocol-level and VS Code smoke tests prove the editor loop from server
  startup through diagnostics, hover, actions, copy payloads, related-test
  opening, and restart/status paths
- status and staleness make stale, failed, disabled, or unavailable evidence
  visible rather than presenting it as fresh
- user-facing docs describe the saved-workspace editor workflow and its limits

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `campaign/editor-evidence-ux-audit` | done | Added `docs/EDITOR_EVIDENCE_UX.md` and the audit handoff, mapping diagnostic data, hover, actions, context collection, VS Code proof, LSP cockpit status, status/staleness, and non-goals into one editor evidence contract without behavior changes. |
| `lsp/evidence-hover-hardening` | done | Hardened hover as the primary explanation surface for seam class, evidence path, missing discriminator, related test location, suggested assertion/test shape, packet and brief handoff commands, verify command, receipt command, and static limits. |
| `lsp/evidence-aware-actions` | done | Tightened action visibility so targeted-test briefs require related-test or suggested-assertion context, suggested assertions and related-test opening remain evidence-gated, stale seam diagnostics fail closed, and agent-loop commands stay available for stable seam diagnostics. |
| `lsp/context-packet-command` | done | Added `ripr.collectEvidenceContext`, a schema `0.1` LSP execute-command packet with seam identity, evidence path, missing discriminator, related test, suggested test, shared agent-loop commands, and static limits without source edits, generated tests, provider coupling, broad analysis reruns, or runtime mutation execution. |
| `test/lsp-protocol-smoke` | done | Extended framed LSP proof through initialize, saved-workspace refresh, a real boundary-gap seam diagnostic, hover, codeAction, `ripr.collectEvidenceContext`, and shutdown without relying on the VS Code client. |
| `test/vscode-extension-smoke` | done | Extended the live VS Code e2e smoke so the real boundary-gap server path reaches a seam diagnostic, hover, code actions, copied seam packet and verify payloads, related-test opening, and restart callability without adding editor features. Bad-server-path status remains in the status/staleness slice. |
| `lsp/editor-status-and-staleness` | done | Made disabled config, missing workspace, unavailable server, queued, running, complete, no-actionable-seam, stale, and failed states explicit in the VS Code status bar and Show Status path. Dirty Rust buffers keep stale status visible until save or close so saved-workspace completion does not look fresh for unsaved evidence. |
| `docs/editor-evidence-workflow` | done | Added `docs/EDITOR_EVIDENCE_WORKFLOW.md`, a user-facing saved-workspace editor path from install and status through diagnostic, hover, related test, context packet, one focused test, after snapshot, verify, receipt, and refresh with explicit static-evidence limits. |
| `campaign/editor-evidence-ux-closeout` | done | Closed after hover, actions, context packet, protocol proof, VS Code proof, status/staleness, and docs aligned without analyzer, policy, CI, or runtime-claim drift; see the closeout handoff. |

Dependencies:

- Campaign 10 supplies the editor-agent loop and command surfaces.
- Campaign 11 supplies shared agent-loop command templates, workflow packets,
  receipts, and reviewer summaries.
- Campaign 12 supplies first-hour editor status and intent-titled action
  framing.
- Campaign 13 supplies PR guidance without making RIPR a free-form reviewer.
- Campaign 17 is complete; this lane closed as an explicit parallel Lane 3
  decision.
- `docs/EDITOR_EVIDENCE_UX.md` and the audit handoff define the contract that
  future behavior PRs should follow.

Closeout:

- [Editor Evidence UX closeout](handoffs/2026-05-09-editor-evidence-ux-closeout.md)
  records the prompt-to-artifact audit, PR chain, validation commands, and the
  boundary that future editor work should be opened explicitly.

Commands:

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

Blocking conditions:

- analyzer behavior changes
- policy or gate behavior changes
- generated workflow behavior changes
- automatic source edits
- generated tests
- runtime mutation execution
- runtime adequacy claims
- unsaved-buffer overlays in this campaign
- CodeLens, inlay hints, semantic tokens, or other speculative editor surfaces
- new public crates

## Lane 3 Campaign: Editor First-Run and Repair Usability

Campaign ID: `editor-first-run-usability`

Status: complete as an explicit Lane 3 closeout.

Editor Gap Cockpit made the saved-workspace evidence loop projectable. Editor
First-Run and Repair Usability made that loop self-orienting for a user who
does not already know RIPR's artifact graph.

Objective:

```text
Make the VS Code path explain setup, no-output states, one evidence-backed gap,
one bounded repair action, verification, receipt state, and refresh without
adding analyzer, policy, source-edit, generated-test, provider, mutation, PR,
or gate authority to Lane 3.
```

End state:

- `ripr: Diagnose Setup` and `ripr: Show Status` name the active workspace,
  resolved server state, config, enabled languages, artifact presence,
  freshness, receipt state, and next safe action.
- No-output states distinguish missing workspace, server unavailable, missing
  config, disabled language, unavailable adapter, missing artifacts, stale
  artifacts, no actionable gap, and preview-limited evidence.
- First-repair actions appear only when typed gap identity, repair route,
  related-test, verify command, and receipt command evidence is safe.
- Receipt projection consumes existing receipt artifacts only and fails closed
  for stale, wrong-root, malformed, unsupported-schema, or gap-mismatched
  receipts.
- Preview-language evidence remains opt-in, advisory, syntax-first, and
  static-limit labeled before action language.

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `docs/lane3-editor-first-run-usability-stack` | done | Added RIPR-PROP-0008, RIPR-SPEC-0049, RIPR-SPEC-0050, ADR-0013, the implementation plan, lane tracker state, indexes, and traceability. |
| `vscode/setup-diagnosis-status-model` | done | Added the setup status model for server path/version, workspace root, config path, enabled and build-available languages, artifacts, freshness, receipt state, and next safe action. |
| `vscode/diagnose-setup-command` | done | Added `ripr: Diagnose Setup` as a read-only report in the output channel. |
| `test/vscode-first-run-no-output-states` | done | Smoke-tested no workspace, server unavailable, server available, missing config, Rust default, preview disabled, adapter unavailable, stale evidence, no actionable gap, and actionable gap states. |
| `lsp/receipt-status-in-show-status` | done | Projected existing receipt state in Show Status without producing receipts or claiming runtime adequacy. |
| `lsp/first-repair-action-packet` | done | Added a bounded first-repair packet action gated by typed gap identity, repair route, verify command, receipt command, and path safety. |
| `fixtures/editor-first-run-usability` | done | Added setup, server missing, config missing, language disabled, adapter unavailable, artifact missing, artifact stale, receipt found, receipt mismatch, receipt improved, and receipt unchanged fixtures. |
| `docs/editor-first-run-to-first-receipt` | done | Documented the install/open, Diagnose Setup, diagnostic, hover, related-test or packet, verify, receipt, and refresh loop. |
| `dogfood/lane3-first-run-repair-receipts` | done | Recorded first-run repair dogfood receipts and limitations without adding editor behavior. |
| `campaign/lane3-editor-first-run-usability-closeout` | done | Closed the campaign in #1040 with validation evidence and explicit non-goals. |

Closeout:

- [Editor First-Run Usability closeout](handoffs/2026-05-16-editor-first-run-usability-closeout.md)
  records the PR chain, prompt-to-artifact audit, validation commands, known
  limitations, and future-work boundary.

Commands:

```bash
cargo xtask lsp-cockpit-report
cargo xtask check-fixture-contracts
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-traceability
cargo xtask check-pr
git diff --check
```

Blocking conditions:

- analyzer truth changes
- policy or gate behavior changes
- PR or CI rendering changes
- source edits or generated tests
- provider/model calls
- runtime mutation execution
- runtime adequacy, Rust-parity, or gate-eligibility claims for preview evidence
- unsaved-buffer overlays, CodeLens, inlay hints, semantic tokens, or inline
  patch application in this campaign

## Cross-Surface Campaign: Start-Here Surface Convergence

Campaign ID: `start-here-surface-convergence`

Status: complete.

The editor, CLI, generated CI, PR evidence, report packet index, receipts,
preview-language reports, and install/release docs are useful independently.
This campaign makes those surfaces lead with the same safe next-action unit so
users do not need to understand RIPR's internal artifact graph before acting.

Objective:

```text
Make every start-here surface answer: what is the one repairable gap, why does
it matter, where should the focused test go, what verifies movement, what
receipt proves it, and what remains limited or advisory?
```

End state:

- PR/CI summaries and report packets lead with a canonical gap or no-action
  state instead of raw finding counts.
- CLI front-door commands use the same safe-next-action and recovery-state
  vocabulary.
- Receipt lifecycle state is consistent across CLI, PR/CI, editor projection,
  first-pr packets, and docs.
- No-output and fail-closed states are explicit outside the editor.
- Preview-language promotion criteria are visible and policy-owned.
- External-style dogfood proves the converged path on normal repo shapes and
  failure states.

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `docs/start-here-surface-convergence-stack` | done | #201 accepted the proposal/spec/ADR stack, replaced source issue numbers with swarm issue rails, and activated the campaign manifest. |
| `report/pr-ci-start-here-canonical-unit` | done | #202 made PR evidence ledger, PR review front-panel Markdown, and the CI-appended PR evidence summary lead with `Start here` canonical repair fields before raw counts. |
| `cli/start-here-command-language` | done | #203 aligned CLI/front-door wording on `Start Here`, generated workflow summary copy, `doctor` setup guidance, safe next action, fail-closed state names, verify command, receipt command, receipt path, recovery states, and advisory boundaries without changing packet schema. |
| `receipt/lifecycle-state-convergence` | done | #204 standardizes receipt found/missing/stale/mismatch/improved/unchanged/not-applicable states across agent receipt, first-pr, PR evidence, front-panel, actionable-gap outcome, and editor projection fixtures. |
| `output/no-output-fail-closed-states` | done | #205 standardizes clean, no-action, missing, stale, wrong-root, disabled, unavailable, malformed, partial, and unsafe output states outside the editor. |
| `policy/preview-promotion-proof-criteria` | done | #206 defines proof criteria before preview evidence can claim a stronger tier and keeps TypeScript, JavaScript, and Python preview evidence advisory until a policy-owned packet closes the criteria. |
| `dogfood/external-style-start-here-receipts` | done | #207 records normal-repo and failure-state receipts for the converged path in [Start-here convergence receipts](handoffs/2026-05-22-start-here-surface-convergence-receipts.md). |
| `campaign/start-here-surface-convergence-closeout` | done | #208 closes the campaign in [Start-here surface convergence closeout](handoffs/2026-05-22-start-here-surface-convergence-closeout.md), archives the active goal, and records `no_current_goal = true`. |

Commands:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-traceability
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

Blocking conditions:

- analyzer behavior changes in the docs/issue setup PR
- output schema changes without a scoped behavior PR
- generated CI blocking or default gate behavior changes
- preview-language policy promotion without a promotion packet
- PR comment publishing changes
- source edits, generated tests, provider/model calls, mutation execution, or
  editor UI-sprawl work

## Lane 3 Campaign: Editor Adoption Assurance

Campaign ID: `editor-adoption-assurance`

Status: closed.

The editor cockpit, first-run repair loop, first-pr bridge, and preview routing
are closed. This campaign makes the first-use editor path safer for outside
users by hardening compatibility, active-root, multi-root, receipt mismatch,
and first-pr packet mismatch diagnosis without making Lane 3 a producer.

Objective:

```text
Make the editor explain what is active, what is incompatible or unsafe, and
what is safe to do next before a user or agent receives a repair packet.
```

End state:

- current Rust/default editor cockpit behavior remains pinned;
- extension/server compatibility diagnosis names version, path, schema, and
  safe next action;
- active workspace root and multi-root ambiguity are explicit;
- wrong-root, stale, malformed, unsupported, receipt-mismatched, and first-pr
  mismatched states fail closed;
- fixtures and VS Code smoke prove success and fail-closed states;
- install-to-first-pr docs and external-style dogfood receipts prove the path;
- Lane 3 remains read-only and projection-only.

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `docs/lane3-editor-adoption-assurance-stack` | done | Added proposal, spec, ADR, plan, indexes, traceability, and GitHub issues. |
| `test/lsp-editor-adoption-baseline` | done | Pinned the closed Lane 3 contract before compatibility/root behavior changes. |
| `vscode/extension-server-compatibility-diagnosis` | done | #1262 landed the #1247 implementation; close the still-open issue from that merged work. |
| `vscode/workspace-root-multi-root-diagnosis` | done | #1267, #1270, #1272, and #1274 landed the #1248 implementation; close the still-open issue from those merged changes. |
| `fixtures/editor-adoption-assurance` | done | Added setup, mismatch, first-pr, receipt, and preview-unavailable fixtures. |
| `test/vscode-editor-adoption-assurance` | done | Smoked the packaged extension path for adoption assurance. |
| `docs/editor-install-to-first-pr` | done | Documented install/open through first-pr packet inspection and recovery states. |
| `dogfood/lane3-editor-adoption-receipts` | done | Recorded external-style setup, root, receipt, first-pr, preview-unavailable, and fail-closed adoption receipts. |
| `campaign/lane3-editor-adoption-assurance-closeout` | done | Recorded closeout proof, accepted the proposal/spec, and closed the issue burn-down. |

Issue reconciliation on 2026-05-18 found #1247 and #1248 satisfied on `main`.
#1249, #1250, #1251, and #1252 are satisfied by the fixture corpus, VS Code
smoke, install-to-first-pr guide, and dogfood receipts. #1253 is satisfied by
the closeout. Do not restart the compatibility or root-diagnosis slices unless
a new regression appears.

Commands:

```bash
cargo xtask check-spec-format
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-traceability
cargo xtask check-pr
git diff --check
```

Blocking conditions:

- analyzer truth changes
- policy or gate behavior changes
- PR or CI producer behavior
- release publishing, binary download, binary install, or config mutation
- source edits or generated tests
- provider/model calls
- runtime mutation execution
- runtime adequacy, Rust-parity, policy-eligibility, or gate claims
- unsaved-buffer overlays, CodeLens, inlay hints, semantic tokens, or inline
  patch application in this campaign

## Lane 3 Campaign: Editor Actionable Gap Queue

Campaign ID: `editor-actionable-gap-queue`

Status: closed.

Editor Adoption Assurance is closed. The next selected Lane 3 slice projects
the existing Lane 1 `actionable-gaps` artifact into the editor as a bounded
local repair queue. Lane 3 validates and projects the artifact; it does not
produce the artifact, re-rank gaps, decide policy, or create PR/CI output.

Objective:

```text
Make the editor answer what is safe to work on now from existing typed
actionable-gap artifacts.
```

End state:

- post-adoption editor behavior remains pinned;
- `target/ripr/reports/actionable-gaps.json` is validated before use;
- Show Status names the top actionable gap or no-action state;
- Copy Current Repair Packet is available only for validated actionable gaps;
- Copy Repo Gap Map is read-only orientation;
- stale, wrong-root, malformed, unsupported, unsafe, disabled, unavailable,
  receipt-mismatched, first-pr-mismatched, and actionable-packet-mismatched
  states fail closed;
- fixtures, VS Code smoke, docs, dogfood receipts, and closeout proof the
  path;
- Lane 3 remains read-only and projection-only.

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `docs/lane3-editor-actionable-gap-queue-stack` | closed | Source-of-truth proposal, spec, ADR, plan, indexes, traceability, capability wiring, lane tracker, and issue burn-down landed. |
| `test/lsp-post-adoption-editor-contract` | closed | Diagnose Setup, Show Status, first-pr, receipt, Rust diagnostics, preview labels, and fail-closed behavior were pinned before queue projection. |
| `lsp/actionable-gap-packet-validation` | closed | `target/ripr/reports/actionable-gaps.json` validates as a safe input seam. |
| `lsp/show-status-repair-queue` | closed | Show Status projects bounded queue summaries and fail-closed/no-action states. |
| `lsp/copy-current-repair-packet` | closed | Copy one bounded repair packet only when typed safety fields validate. |
| `lsp/copy-repo-gap-map` | closed | Copy read-only queue orientation without gate, runtime, mutation, policy, or merge-readiness claims. |
| `fixtures/editor-actionable-gap-queue` | closed | Fixture corpus covers top-gap, multiple-gap, no-action, static-limit-only, stale, wrong-root, malformed, improved, and unchanged states. |
| `test/vscode-actionable-gap-queue` | closed | Packaged extension smoke covers status, packets, repo map, receipt state, and unsafe-state suppression. |
| `docs/editor-actionable-gap-queue` | closed | Document the queue workflow and recovery states. |
| `dogfood/lane3-actionable-gap-queue-receipts` | closed | [Editor Actionable Gap Queue dogfood receipts](handoffs/2026-05-20-editor-actionable-gap-queue-receipts.md) record actionable, no-action, static-limit-only, wrong-root, stale, receipt, and preview-advisory proof. |
| `campaign/lane3-actionable-gap-queue-closeout` | closed | [Editor Actionable Gap Queue closeout](handoffs/2026-05-20-editor-actionable-gap-queue-closeout.md) records the PR chain, validation, remaining limits, and future-work boundary. |

Commands:

```bash
cargo xtask check-spec-format
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

Blocking conditions:

- analyzer truth changes
- `actionable-gaps` producer or schema changes
- policy or gate behavior changes
- PR or CI producer behavior
- release publishing, binary download, binary install, or config mutation
- source edits or generated tests
- provider/model calls
- runtime mutation execution
- runtime adequacy, Rust-parity, policy-eligibility, gate, or merge-readiness
  claims
- unsaved-buffer overlays, CodeLens, inlay hints, semantic tokens, or inline
  patch application in this campaign
