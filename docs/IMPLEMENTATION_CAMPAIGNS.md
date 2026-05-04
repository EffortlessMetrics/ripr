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

Status: active

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
| `badge/publish-main-endpoint` | ready | Trunk-only Shields endpoint published from `main`; README badges point at it. Requires `policy/network_allowlist.txt` entry and consumes repo-scoped artifacts only. PR workflows do not publish. |

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

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `lsp/evidence-hover-actions` | blocked | Depends on evidence-first output shape. |
| `context/agent-context-v2` | blocked | Depends on stable evidence and oracle fields. |
| `docs/how-to-use-agent-context` | blocked | Depends on context v2. |

## Campaign 5: Adoption and Calibration

Objective:

```text
Make `ripr` practical in repositories, CI, and offline calibration loops.
```

End state:

- repository config exists
- SARIF and CI policy modes exist
- cargo-mutants calibration scaffold exists
- persistent cache exists after fact model earns caching

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `config/ripr-config-v1` | blocked | Depends on stable analyzer conventions. |
| `ci/sarif-ci-policy` | blocked | Depends on output contract stability. |
| `calibration/cargo-mutants-scaffold` | blocked | Depends on improved static facts. |
| `cache/persistent-cache-v1` | blocked | Depends on stable fact model. |
