# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Product Contract

`ripr` answers exactly one draft-time question:

> For the behavior changed in this diff, do the current tests appear to contain a discriminator that would notice if that behavior were wrong?

It is a **static** RIPR (Reach-Infect-Propagate-Observe-Discriminate) exposure analyzer. It does **not** run mutants. Keep work aligned with this contract — do not turn `ripr` into a full mutation engine, coverage dashboard, proof system, second rust-analyzer, or generic test generator.

### Language rules (enforced by `cargo xtask check-static-language`)

Findings must use conservative static language only:

- Allowed: `exposed`, `weakly_exposed`, `reachable_unrevealed`, `no_static_path`, `infection_unknown`, `propagation_unknown`, `static_unknown`.
- Forbidden in static output: `killed`, `survived`, `untested`, `proven`, `adequate`. These belong to runtime mutation testing, not `ripr`.

## Workspace Shape

One published package, one binary, one library — do **not** split into `ripr-core` / `ripr-cli` / `ripr-lsp` / `ripr-engine` / `ripr-schema` until there is a real external contract. The shape is enforced by `cargo xtask check-workspace-shape` against `policy/workspace_shape.txt`.

```
crates/ripr        # the only published package (lib + bin "ripr")
xtask              # repo automation, unpublished
editors/vscode     # VS Code extension that hosts `ripr lsp --stdio`
docs/              # specs, ADRs, capability matrix, plans, learnings
fixtures/          # input diffs + expected outputs for golden tests
policy/            # allowlists for non-Rust files, deps, processes, network, generated files, public API, architecture
.ripr/             # in-repo state: traceability.toml, goals/active.toml, allow attribute lists
```

### Internal modules (under `crates/ripr/src/`, enforced by `cargo xtask check-architecture`)

- `domain/` — `Probe`, `RiprEvidence`, `OracleStrength`, `ExposureClass`. Pure data, no I/O.
- `app.rs` — public library API (`check_workspace`, `explain_finding`, `collect_context`) and `CheckInput` / `CheckOutput` / `Mode` / `OutputFormat`.
- `analysis/` — diff loading (`diff.rs`), syntax index (`rust_index.rs`), probe generation (`probes.rs`), classification (`classifier.rs`), workspace discovery.
- `output/` — `human`, `json`, `github` renderers. JSON shape is versioned; do not change without updating `docs/OUTPUT_SCHEMA.md`, fixtures, and golden expectations.
- `cli/` — argv adapter only. All real work goes through `app::*`.
- `lsp/` — experimental `tower-lsp-server` sidecar (`backend.rs`, `diagnostics.rs`, `hover.rs`, `state.rs`, `actions.rs`).

Each module has a single product responsibility — parsing, fact extraction, probe generation, classification, orchestration, or rendering. Don't blur seams.

## Rust Baseline

- Edition 2024, MSRV `1.93` (pinned in `rust-toolchain.toml` to `1.93.1`).
- `unsafe_code = "forbid"` workspace-wide. Also denied: `dbg_macro`, `todo`, `unimplemented`, `const_item_interior_mutations`, `function_casts_as_integer`.
- No new `panic`, `unwrap`, `expect`, `todo`, or `unimplemented` in production or test code (enforced by `cargo xtask check-no-panic-family` against `.ripr/no-panic-allowlist.txt`).
- Allow-attributes need an entry in `.ripr/allow-attributes.txt` (enforced by `cargo xtask check-allow-attributes`).

### Rust-first file policy

Rust is the default for production logic, repo automation, fixture runners, release checks, and policy checks. Adding a non-Rust programming file (shell / Python / JS / TS) requires updating `policy/non_rust_allowlist.txt` and explaining the exception in the PR. The VS Code extension, GitHub Actions YAML, fixture inputs, doc examples, and generated outputs are explicit exceptions covered by policy metadata.

Prefer `cargo xtask <command>` for repo automation rather than scripts.

## Common Commands

### Build, test, lint

```bash
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo doc --workspace --no-deps
```

Run a single test: `cargo test -p ripr <test_name>` (e.g. `cargo test -p ripr check_human_output_reports_sample_findings`). The integration smoke tests live in `crates/ripr/tests/cli_smoke.rs` and shell out to the built binary.

Package / publish dry-run:

```bash
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

### Run the binary against the in-repo sample

```bash
cargo run -p ripr -- --version
cargo run -p ripr -- doctor
cargo run -p ripr -- check  --diff crates/ripr/examples/sample/example.diff
cargo run -p ripr -- check  --diff crates/ripr/examples/sample/example.diff --json
cargo run -p ripr -- explain --diff crates/ripr/examples/sample/example.diff probe:crates_ripr_examples_sample_src_lib.rs:21:error_path
cargo run -p ripr -- context --diff crates/ripr/examples/sample/example.diff --at probe:crates_ripr_examples_sample_src_lib.rs:21:error_path --json
cargo run -p ripr -- lsp --stdio
```

### `cargo xtask` automation surface

The xtask crate is a single binary (`xtask/src/main.rs`) hosting all repo automation. It is the contributor and CI entrypoint — prefer these over remembering individual gates.

Shaping & PR hygiene:

```bash
cargo xtask shape          # safe local normalization: cargo fmt, sort policy/.ripr allowlists, ensure target/ripr/reports, write shape.md
cargo xtask fix-pr         # shape + refresh pr-summary + write fix-pr.md (the safe-repair entrypoint)
cargo xtask pr-summary     # write target/ripr/reports/pr-summary.md from git diff/status
cargo xtask precommit      # cheap non-mutating guardrail
cargo xtask check-pr       # review-ready non-release gate
```

Evidence:

```bash
cargo xtask fixtures              # run fixture diffs, compare actual vs expected, write target/ripr/fixtures/<name>/
cargo xtask goldens check         # fail on golden drift without mutating files
cargo xtask goldens bless <fixture> --reason <reason>   # explicit, recorded re-bless
cargo xtask golden-drift          # advisory drift report (does not fail on drift)
cargo xtask test-oracle-report    # advisory baseline of ripr's own oracle strength
cargo xtask test-efficiency-report
cargo xtask dogfood               # ripr check --mode fast against in-repo fixture diffs
cargo xtask critic                # advisory adversarial review packet
cargo xtask reports index         # write target/ripr/reports/index.md (reviewer front door)
cargo xtask receipts              # machine-readable evidence receipts; pair with `receipts check`
cargo xtask metrics
```

Policy / shape gates (all run in CI):

```bash
cargo xtask check-static-language        # bans killed/survived/etc. in static output
cargo xtask check-no-panic-family        # bans panic/unwrap/expect/todo/unimplemented outside allowlist
cargo xtask check-allow-attributes       # tracks #[allow(..)] via .ripr/allow-attributes.txt
cargo xtask check-local-context
cargo xtask check-file-policy            # non-Rust file allowlist
cargo xtask check-executable-files
cargo xtask check-workflows
cargo xtask check-spec-format
cargo xtask check-fixture-contracts
cargo xtask check-traceability           # spec -> tests -> code mapping in .ripr/traceability.toml
cargo xtask check-capabilities
cargo xtask check-workspace-shape        # one-package surface
cargo xtask check-architecture           # internal module boundaries
cargo xtask check-public-api             # public symbol allowlist (policy/public_api.txt)
cargo xtask check-output-contracts
cargo xtask check-doc-index
cargo xtask check-pr-shape
cargo xtask check-generated
cargo xtask check-dependencies           # policy/dependency_allowlist.txt
cargo xtask check-process-policy
cargo xtask check-network-policy
cargo xtask check-supply-chain
```

Outputs land under `target/ripr/{reports,receipts,fixtures,dogfood}/` — these directories are reviewer artifacts, not source.

### VS Code extension

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
```

The extension resolves the `ripr` LSP server in this fixed order: `ripr.server.path` setting → bundled binary → cached download → verified first-run GitHub Release download → `ripr` on `PATH` → actionable error. **Do not** make `cargo install ripr` a requirement for the normal install path; it is the offline / pinned fallback.

## PR Doctrine

PR size is measured by **production risk**, not line count. A scoped PR changes one production behavior, public contract, or architectural seam, plus the complete evidence package needed to make it reviewable: specs, fixtures, tests, golden outputs, docs, metrics, ADRs, learnings, traceability.

Every material behavior change should preserve this chain:

```
spec -> test or fixture -> code -> output contract -> metric
```

Make production delta, evidence delta, acceptance criterion, and non-goals explicit in PRs and planning docs. Large fixture / golden / spec / docs / ADR diffs are welcome when they make one production behavior reviewable. A small code diff is **not** acceptable if it changes multiple contracts without a spec-test-code trail.

Avoid: bundling unrelated behaviors, mixing schema changes with analyzer rewrites, mixing LSP/UI changes with classifier changes, mixing cleanup with behavior changes.

## Long-Context Workflow

This repo is intentionally organized so agents can resume long-running goals from repository artifacts instead of chat history. When picking up unfamiliar work:

- `docs/ROADMAP.md`, `docs/IMPLEMENTATION_PLAN.md` — current direction and checkpoints
- `docs/IMPLEMENTATION_CAMPAIGNS.md` + `.ripr/goals/active.toml` — active multi-PR campaigns (Codex Goals model)
- `docs/CAPABILITY_MATRIX.md` — current capability status per area
- `docs/PR_AUTOMATION.md` — the shape/check/guide model
- `docs/CODEX_GOALS.md`, `docs/SCOPED_PR_CONTRACT.md` — the PR-shaping contract
- `docs/specs/` + `.ripr/traceability.toml` — spec → tests → code map
- `docs/STATIC_EXPOSURE_MODEL.md`, `docs/OUTPUT_SCHEMA.md` — domain model and stable JSON shape
- `docs/LEARNINGS.md` — repo knowledge worth surviving across sessions; update it when you learn something durable
- `AGENTS.md` — terse rules of engagement for agents (read it once at session start)

Choose the smallest vertical slice with one production delta and one evidence package.
