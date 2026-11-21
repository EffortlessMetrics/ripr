# Modularization Campaign: One-Crate SRP Refactoring

**Campaign ID**: `modularize-ripr-submodules`

**Status**: active

## Objective

Refactor internal modules under `crates/ripr/src/` so each module has one product responsibility, improving maintainability, testability, and reasoning about the system without splitting the package.

`ripr` must remain:

```text
- One published crate (ripr)
- One library (lib)
- One binary (ripr CLI)
- No internal crate splits (ripr-core, ripr-cli, ripr-lsp, etc.)
```

## Why It Matters

Current internal structure mixes responsibilities across modules. For example:

- `analysis/mod.rs` orchestrates pipeline, sorts results, and counts summaries
- `analysis/rust_index.rs` parses Rust syntax, builds indices, and extracts facts
- Classification spread across multiple functions in `analysis/` with shared helpers
- `app.rs` mixes use-case orchestration, format selection, and badge rendering

This makes:

- Changes to one concern ripple across module boundaries
- Testing isolated logic harder (need to set up large contexts)
- Future modularization (async, parallelism, caching) more complex
- New contributors harder to reason about
- Agent handoff documentation less precise (module boundary = responsibility boundary)

The refactoring unwinds these tangles while keeping the public API and all behavior unchanged.

## End State

```text
crates/ripr/src/
  domain/           — stable data model and DTOs
  app/              — use-case orchestration (check, explain, context)
  analysis/         — facts, probes, classification pipeline
    diff/           — diff parsing and loading
    workspace/      — file discovery and scope selection
    facts/          — fact model and index building
    syntax/         — syntax adapter and parser-backed extraction
    extract/        — fact extraction helpers (calls, literals, oracles)
    probes/         — probe generation and seeding
    classify/       — RIPR stage-by-stage classification
  output/           — rendering (human, JSON, GitHub, badge)
  cli/              — argv parsing and command execution
  lsp/              — LSP server (largely preserved)
  xtask/            — repo automation (split by concern)
```

Each module in this layout carries one responsibility:

| Module               | Responsibility                                        |
| -------------------- | ----------------------------------------------------- |
| `domain`             | Model definitions; no I/O or orchestration           |
| `app`                | Use-case entry points; delegates to analysis/output  |
| `analysis/diff`      | Unified diff parsing                                  |
| `analysis/workspace` | File discovery and mode-based scope selection        |
| `analysis/facts`     | Fact model; index construction from syntax           |
| `analysis/syntax`    | Syntax adapter; parser-backed and lexical adapters   |
| `analysis/extract`   | Fact extraction helpers                              |
| `analysis/probes`    | Probe seeding and family classification              |
| `analysis/classify`  | Classification pipeline (stage by stage)             |
| `output`             | Format rendering                                      |
| `cli`                | Argv parsing and command dispatch                    |
| `lsp`                | Language server protocol sidecar                     |

## Constraints

**Hard constraints** — do not:

- Split into multiple crates
- Change JSON schemas (except with explicit new version)
- Change static output language (no `killed`, `survived`, `proven`, `adequate`)
- Add new probe families
- Change exposure classification behavior
- Add dependencies (unless unavoidable for the refactoring itself)
- Rewrite CLI parsing with clap or similar
- Mix analyzer behavior changes with module movement
- Re-bless goldens unless the PR intentionally changes output

**Preserve**:

- Existing public behavior
- Current CLI commands and flags
- Current LSP command surface
- Current JSON output
- Current badge behavior
- All tests and fixture expectations

## Phases

### Phase 1: Low-Risk Analysis Extraction

**PRs 1–2**: Extract duplicated analysis helpers.

#### PR 1: `analysis-summary-extraction`

Remove duplicated sorting and summary-counting logic.

**Create**:

```text
crates/ripr/src/analysis/summary.rs
crates/ripr/src/analysis/sort.rs
```

**Implement**:

```rust
// analysis/summary.rs
pub(crate) fn summarize_findings(
    changed_rust_files: usize,
    findings: &[Finding],
) -> Summary { ... }

// analysis/sort.rs
pub(crate) fn sort_findings(findings: &mut [Finding]) { ... }
```

**Update** `analysis/mod.rs` to call these helpers instead of duplicating logic.

**Acceptance**:

```text
- run_analysis and run_repo_analysis behavior unchanged
- No output changes
- Existing tests pass
- cargo test --workspace
- cargo xtask check-pr
```

---

#### PR 2: `analysis-pipeline-extraction`

Make `analysis/mod.rs` a thin façade over pipeline logic.

**Create**:

```text
crates/ripr/src/analysis/pipeline.rs
```

**Move** the bodies of `run_analysis` and `run_repo_analysis` into:

```rust
pub(crate) fn run_diff_pipeline(options: &AnalysisOptions) -> Result<AnalysisResult, String>;
pub(crate) fn run_repo_pipeline(options: &AnalysisOptions) -> Result<AnalysisResult, String>;
```

**Keep** public functions in `analysis/mod.rs` as façade wrappers.

**Acceptance**:

```text
- run_analysis and run_repo_analysis signatures unchanged
- No output changes
- cargo xtask fixtures && cargo xtask goldens check
- cargo xtask dogfood
- cargo xtask check-pr
```

---

### Phase 2: Split Compact Modules

**PRs 3–4**: Break `diff.rs` and `workspace.rs` into modules.

#### PR 3: `diff-module-split`

**Create**:

```text
crates/ripr/src/analysis/diff/mod.rs
crates/ripr/src/analysis/diff/model.rs
crates/ripr/src/analysis/diff/load.rs
crates/ripr/src/analysis/diff/parse.rs
```

**Move symbols**:

| Symbol               | Destination       |
| -------------------- | ----------------- |
| `ChangedFile`        | `diff/model.rs`   |
| `ChangedLine`        | `diff/model.rs`   |
| `load_diff`          | `diff/load.rs`    |
| `parse_unified_diff` | `diff/parse.rs`   |
| `parse_hunk_header`  | `diff/parse.rs`   |
| `parse_start`        | `diff/parse.rs`   |

**Acceptance**: Diff parsing behavior unchanged; parser tests pass.

---

#### PR 4: `workspace-module-split`

**Create**:

```text
crates/ripr/src/analysis/workspace/mod.rs
crates/ripr/src/analysis/workspace/discover.rs
crates/ripr/src/analysis/workspace/scope.rs
crates/ripr/src/analysis/workspace/production.rs
crates/ripr/src/analysis/workspace/paths.rs
```

**Move symbols** (file discovery, scope selection, path policy).

**Acceptance**: Mode scope behavior and production-file detection unchanged.

---

### Phase 3: Shrink `rust_index.rs`

**PRs 5–10**: Extract fact model, syntax adapters, and extractors.

#### PR 5: `facts-model-extraction`

Move fact DTOs out of parser implementation into `analysis/facts/model.rs`.

#### PR 6: `syntax-adapter-type-extraction`

Move syntax adapter abstractions into `analysis/syntax/adapter.rs`.

#### PR 7: `index-builder-extraction`

Move index construction into `analysis/facts/build.rs`.

#### PR 8: `ra-syntax-adapter-extraction`

Move parser-backed summarization into `analysis/syntax/ra.rs`.

#### PR 9: `lexical-syntax-fallback-extraction`

Move lexical fallback into `analysis/syntax/lexical.rs`.

#### PR 10: `extractors-extraction`

Move fact extraction functions into `analysis/extract/` modules (calls, literals, oracles, probe_shapes, text).

**Acceptance**: `rust_index.rs` is deleted or becomes a thin compatibility shim; golden outputs unchanged.

---

### Phase 4: Split Probe Generation

**PRs 11–15**: Break up probe seeding and generation.

#### PR 11: `probe-family-extraction`

Create `analysis/probes/family.rs`; move `family_for_probe_shape` and `delta_for_family`.

#### PR 12: `probe-expectations-extraction`

Create `analysis/probes/expectations.rs`; move oracle and sink expectations.

#### PR 13: `probe-id-extraction`

Create `analysis/probes/ids.rs`; move ID sanitization and seeding.

#### PR 14: `probe-lexical-fallback-extraction`

Create `analysis/probes/lexical.rs`; move line-level probe classification.

#### PR 15: `probe-seed-diff-repo-split`

Create `analysis/probes/diff.rs` and `analysis/probes/repo.rs`; split diff-scoped and repo-scoped seeding.

**Acceptance**: Probe generation behavior unchanged; fixtures pass.

---

### Phase 5: Split Classification by RIPR Stage

**PRs 16–21**: Break up classification into stage-specific modules.

Each PR moves one or more RIPR stages (reach, activation, infection, propagation, observation, discrimination) into dedicated modules under `analysis/classify/`.

#### PR 16: `classification-context-extraction`

Create `analysis/classify/context.rs`; define `ProbeContext` struct.

#### PR 17–21: Stage extraction PRs

| PR  | Module           | Stages                                            |
| --- | ---------------- | ------------------------------------------------- |
| 17  | `related_tests`  | Related test discovery                            |
| 18  | `reach.rs`       | Reachability evidence                             |
| 19  | `flow.rs`, `propagation.rs` | Flow sinks and propagation |
| 20  | `activation.rs`, `text.rs` | Activation and helper text |
| 21  | `infection.rs`, `reveal.rs`, `decision.rs`, `confidence.rs`, `missing.rs`, `stop_reasons.rs`, `recommendation.rs` | Remaining stages and decision |

**Acceptance**: Classification logic behavior unchanged; test-oracle and dogfood reports stable.

---

### Phase 6: Split App Use Cases

**PRs 22–24**: Break up `app.rs` into use-case modules.

#### PR 22: `app-usecase-split`

Convert `crates/ripr/src/app.rs` into:

```text
crates/ripr/src/app/mod.rs
crates/ripr/src/app/input.rs
crates/ripr/src/app/check.rs
crates/ripr/src/app/explain.rs
crates/ripr/src/app/context.rs
```

**Acceptance**: `check_workspace`, `explain_finding`, `collect_context` behavior unchanged.

#### PR 23: `output-format-extraction`

Move `OutputFormat` from app to `output/format.rs`.

#### PR 24: `render-dispatch-extraction`

Move rendering logic from app to `output/render.rs`; app produces structured output, output renders it.

---

### Phase 7: Split CLI Parsing and Execution

**PRs 25–27**: Decouple CLI parsing from execution.

#### PR 25: `cli-command-model`

Create `cli/command.rs`; define `CliCommand` enum.

#### PR 26: `cli-parse-command`

Update `cli/parse.rs`; add `parse_args` and sub-parsers returning `CliCommand`.

#### PR 27: `cli-execute-command`

Create `cli/execute.rs` and `cli/doctor.rs`; add `execute(CliCommand)`.

**Rule**: `parse.rs` does not run analysis; `execute.rs` does not parse argv.

---

### Phase 8: Context Packet DTO

**PRs 28–30**: Introduce context packet as a domain DTO.

#### PR 28: `context-packet-domain-dto`

Create `domain/context_packet.rs`; define `ContextPacket` struct.

#### PR 29: `json-context-renders-dto`

Update JSON renderer to use `ContextPacket`.

#### PR 30: `lsp-context-uses-dto`

Update LSP context lookup to use `ContextPacket`.

---

### Phase 9: Tighten Public API

**PRs 31–32**: Conservatively hide internal modules.

#### PR 31: `public-api-doc-hidden-internals`

Mark internal modules `#[doc(hidden)]` while keeping them public for compatibility.

#### PR 32: `public-api-private-internals` *(optional, breaking)*

Make internal modules private (requires explicit breaking release decision).

---

### Phase 10: Modularize Xtask

**PRs 33–35**: Refactor repo automation.

#### PR 33: `xtask-command-dispatch-split`

Create `xtask/src/command.rs` and `xtask/src/run.rs`.

#### PR 34: `xtask-policy-modules`

Create `xtask/src/policy/` directory; one checker per file.

#### PR 35: `xtask-report-modules`

Create `xtask/src/reports/` directory; move report generation.

---

## Standard PR Acceptance Gate

Every PR must pass:

```bash
cargo fmt --check
cargo test --workspace
cargo xtask check-architecture
cargo xtask check-public-api
cargo xtask check-pr
```

For analyzer changes, also run:

```bash
cargo xtask fixtures
cargo xtask goldens check
cargo xtask dogfood
```

For output/app/CLI changes, add:

```bash
cargo test -p ripr cli
cargo test -p ripr lsp
```

## Review Checklist

Every modularization PR should include this checklist:

```text
Production delta:
- [ ] Pure module extraction / movement only
- [ ] No analyzer behavior change
- [ ] No output contract change
- [ ] No public API change (unless explicitly stated)

Architecture:
- [ ] Responsibilities became narrower
- [ ] No new dependency direction violating architecture
- [ ] New helpers are private or pub(crate), not public

Evidence:
- [ ] cargo fmt --check
- [ ] cargo test --workspace
- [ ] cargo xtask check-architecture
- [ ] cargo xtask check-public-api
- [ ] cargo xtask check-pr
- [ ] (analyzer changes) cargo xtask fixtures && cargo xtask goldens check
```

## Work Planning

**Start here** (establish the pattern):

1. `analysis-summary-extraction`
2. `analysis-pipeline-extraction`
3. `diff-module-split`
4. `workspace-module-split`
5. `facts-model-extraction`

**Then reassess** before diving into the riskier classification/parser logic.

**Recommended order for phases**:

- Phase 1 (low-risk, establishes pattern)
- Phase 2 (straightforward file moves)
- Phase 3 (largest phase; do in multiple sub-PRs)
- Phase 4 (probe generation is isolated)
- Phase 5 (classification; do stage by stage)
- Phase 6–7 (app/CLI boundaries)
- Phase 8–9 (API tightening)
- Phase 10 (xtask automation; lowest priority)

## Blocking Conditions

Stop and reassess if:

- A PR changes output (golden drift) without intentional spec/test evidence
- Architecture gate fails
- Public API gate fails (unless intentional breaking change)
- A PR mixes multiple phases or responsibilities
- Fixtures or goldens fail

## Non-Goals

- Split the crate into multiple packages
- Change JSON schemas or output format (except new schema versions with docs)
- Add new probe families or change classification behavior
- Rewrite the CLI parser
- Add dependencies
- Introduce async, parallelism, or caching (can come after refactoring)

## Success Criteria

At campaign close:

- Every internal module has a single, clear product responsibility
- Module boundaries align with RIPR stages (parsing → facts → probes → classify → render)
- Test coverage is unchanged or improved
- All output artifacts (golden outputs, test reports, dogfood) are stable
- Architecture guard passes with no exceptions
- New contributors can reason about module responsibility by reading the module name and responsibility list
- Agents can produce implementation plans by mapping agent tasks to module boundaries

## Dependencies on Other Campaigns

This campaign is **independent** and can run in parallel with or after Campaign 4B (Repo Seam Inventory). However:

- Do not merge modularization PRs that change `analysis/seams.rs` or seam-related output
- If Campaign 4B adds new analyzer logic, keep it in staged modules (don't add it back to `mod.rs`)

## See Also

- [Architecture](ARCHITECTURE.md)
- [Engineering rules](ENGINEERING.md)
- [CLAUDE.md](../CLAUDE.md) — agent operating rules for this campaign
- [Spec-test-code traceability](SPEC_TEST_CODE.md)
