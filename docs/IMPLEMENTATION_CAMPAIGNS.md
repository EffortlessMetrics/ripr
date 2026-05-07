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

Status: active

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
| `agent/workflow-manifest` | done | Added `ripr agent start --root . --seam-id <id> --out target/ripr/workflow` to write source-edit-free `agent-workflow.json` and `agent-workflow.md` manifests with artifact paths and commands from the shared templates. |
| `agent/receipt-provenance` | ready | Add receipt provenance fields for ripr version, repo root, config fingerprint, artifact hashes, seam_id, before/after class, command template version, and timestamp. |
| `agent/next-action-guidance` | blocked | Emit bounded static next-action guidance for improved, unchanged, regressed, resolved, and new-gap receipt states. |
| `agent/reviewer-summary` | blocked | Add `ripr agent review-summary --root . --json` plus human output that joins status, receipt, cockpit, repo exposure, LSP cockpit when present, and CI artifact status into a compact review packet. |
| `fixtures/llm-work-loop` | blocked | Add LLM work-loop fixtures for happy, unchanged, regressed, missing-artifact, stale-artifact, configured-off, path-with-spaces, and Windows-separator cases. |
| `ci/llm-work-packets` | blocked | Generated CI uploads agent status JSON/Markdown, workflow JSON, review summary JSON/Markdown, receipt, and operator cockpit artifacts as advisory evidence. |
| `docs/llm-operator-guide` | blocked | Document the LLM operator loop from agent status through start, packet or brief, focused test, after snapshot, verify, receipt, and review summary, with anti-goals explicit. |
| `campaign/llm-work-loop-closeout` | blocked | Close Campaign 11 only after LLM work-loop state, commands, provenance, fixtures, CI artifacts, docs, and review summary are aligned without automatic edits, generated tests, runtime mutation execution, speculative LSP features, or new public crates. |

Commands:

```bash
cargo test -p ripr agent_status
cargo test -p ripr agent_workflow
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

Status: queued

This campaign is intentionally separate from Campaign 11. Campaign 11 keeps the
LLM work loop stateful and deterministic through status, command templates,
workflow manifests, receipts, and reviewer summaries. Campaign 12 starts after
that control plane is stable, or earlier only through an explicit product-lane
pivot.

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
- generated CI workflow behavior is pinned by a fixture that covers artifact
  paths, non-blocking posture, optional SARIF, badge output, and agent artifacts
- agent command templates and workflow manifests from Campaign 11 feed these UX
  surfaces instead of creating another command-string source of truth
- README and installed-user docs are organized by user type: VS Code, CI, CLI,
  agent, troubleshooting, and known limits

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `vscode/first-run-status` | queued | Add a discoverable editor status path for server resolution, workspace detection, analysis running/complete/stale/failed, and no-actionable-seam states. |
| `vscode/action-discoverability` | queued | Group and title diagnostic actions around user intent without changing analyzer behavior, adding broad LSP features, or enabling unsaved-buffer overlays. |
| `ci/pr-summary-surface` | queued | Make the generated workflow emit a useful PR or step summary with the top actionable seam, why it matters, suggested test target, artifact links, SARIF status, badge status, and known limits. |
| `ci/generated-workflow-smoke-fixture` | queued | Pin generated workflow artifact paths, top-seam extraction, agent artifact generation, non-blocking posture, optional SARIF, and badge output so CI UX does not drift. |
| `docs/ux-by-user-type` | queued | Rewrite the first-hour docs around VS Code, CI, CLI, agent, troubleshooting, and known limits while keeping README short. |
| `campaign/first-hour-ux-closeout` | queued | Close only after the extension and CI first screens are useful without report archaeology and the CLI remains the shared engine rather than the required first user interface. |

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
