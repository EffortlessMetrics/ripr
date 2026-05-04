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
| `campaign/test-efficiency-closeout` | done | Campaign 4A marked complete here and in `.ripr/goals/active.toml`. Final architecture: per-test ledger + class/reason metrics from `cargo xtask test-efficiency-report`; `.ripr/test_intent.toml` declarations and `.ripr/suppressions.toml` exceptions wired into the `ripr+` count; diff-scoped PR badge artifacts via `cargo xtask badge-artifacts` (#195); repo-scoped baseline via `cargo xtask repo-badge-artifacts` (#204) on schema 0.2 with `scope: "repo"`; checked-in `badges/ripr.json` and `badges/ripr-plus.json` rendered through `img.shields.io/endpoint?url=https://raw.githubusercontent.com/EffortlessMetrics/ripr/main/badges/...` (#209). Final dogfood snapshot at this campaign close: `ripr 163`, `ripr+ 163` (`main` = `6b4b2b0`); snapshot, **not** a fixture expectation. PR chain: #195, #198, #199, #200, #204, #205 (DEFERRED.md), #206 (friction-log graduation), #208 (stale-`317`-headline correction), #209. Issue #207 was the endpoint design-plan. Pages was rejected for v1 dogfood; hosted badge service is `deferred/hosted-badge-service`. The Voice B seam-inventory + test-grip product reframe is **next-campaign work** (`deferred/seam-inventory-test-grip`), not unfinished 4A work. |

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

Status: active

Objective:

```text
Inventory behavior seams across the repo, classify how strongly current tests
grip each seam through RIPR evidence, and turn actionable gaps into editor
diagnostics and agent-ready test packets.
```

The Voice A baseline shipped in Campaign 4A
(`badge/repo-scope-artifacts`, #204) becomes a special case of seam
classification rather than the analyzer's only repo mode. Voice B is
the editor/agent loop with the right substrate: first-class
`RepoSeam` and `SeamGripClass` underneath, evidence-first hover and
agent packets on top.

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
- Voice B does not pretend to prove mutation adequacy

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
| `output/repo-exposure-report-v1` | ready | Markdown + JSON repo report. Schema versioned; existing repo-badge-artifacts schema unchanged unless the spec opts in. |
| `lsp/repo-seam-diagnostics-v1` | blocked | Surface ungripped or under-gripped seams in the editor with stable diagnostic codes. |
| `lsp/seam-evidence-hover-v1` | blocked | Hover renders the RIPR evidence path with cited related tests. **PR #211 is merged** (pre-4B evidence-rich hover over the current Finding / AnalysisSnapshot model); this seam-native hover will extend or revise it once the seam model and diagnostics schema are settled. |
| `context/agent-seam-packets-v1` | ready | Agent context packet schema (RIPR-SPEC-0003 successor or addendum). Load-bearing fields: changed expression, owner, related tests, oracle strength, observed values, missing discriminator. |
| `docs/agent-dispatch-workflow-v1` | blocked | How a coding agent uses a seam packet to write the missing test. |
| `cache/repo-seam-facts-v1` | ready | Optional fact-layer cache (file-facts, owner-index, seam-facts; never final outputs). Gated on real performance signal. Subsumes Campaign 5's `cache/persistent-cache-v1`. |
| `calibration/cargo-mutants-v1` | ready | Optional scaffold for comparing static `SeamGripClass` against cargo-mutants outcomes. Advisory only; static output adopts no mutation-runtime language. Subsumes Campaign 5's `calibration/cargo-mutants-scaffold`. |

Dependencies:

- `spec/repo-seam-inventory` landed in #223, `analysis/repo-seam-model-v1`
  in #229, `analysis/repo-seam-inventory-v1` in #235,
  `analysis/test-grip-evidence-v1` in #236, and
  `analysis/repo-ripr-classification-v1` follows. With classification
  complete, `output/repo-exposure-report-v1` (the renderer) is the
  single recommended next step; `context/agent-seam-packets-v1`,
  `cache/repo-seam-facts-v1`, and `calibration/cargo-mutants-v1` are
  also unblocked but optional.
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

## Campaign 5: Adoption and Calibration

Objective:

```text
Make `ripr` practical in repositories, CI, and external operationalization
loops.
```

End state:

- repository config exists
- SARIF and CI policy modes exist

Cache/calibration work that depends on stable seam IDs migrated to
Campaign 4B (`cache/repo-seam-facts-v1`, `calibration/cargo-mutants-v1`).
Campaign 5 retains the broader operationalization concerns that do
not require the seam model.

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `config/ripr-config-v1` | blocked | Depends on stable analyzer conventions. |
| `ci/sarif-ci-policy` | blocked | Depends on output contract stability. |
