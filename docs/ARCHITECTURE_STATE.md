# Architecture State (2026-07-19)

This document captures the current architecture, what's working, what's
structurally wrong, and where the next investment should focus. It is a
complement to `docs/LEARNINGS.md` (dated lessons) and `AGENTS.md` (operating
rules) — this doc is the structural snapshot.

## Scale

- **248,402 lines** of Rust across 371 source files
- **130 specs** (RIPR-SPEC-0001 through 0130)
- **20 ADRs** (0001 through 0019 + README)
- **200 fixtures** with golden-checked `expected/check.json`
- **16 CI workflows** including the routed-rust lane, scratch-GC, ub-review,
  droid-review, coverage, security, and badge endpoints
- **3,668 test functions** (2,355 inline + 122 cli_smoke + 1,022 xtask + LSP)
- **4 language adapters**: Rust (stable), TypeScript (preview), Python
  (usable alpha), Perl (preview, feature-gated off default)
- **130 spec entries**, **38 honesty-corpus cases**, **1,800 lines of
  LEARNINGS.md**

## Module Layout

```text
crates/ripr/src/
├── domain/         # Types: ExposureClass, Finding, Probe, FixInstruction,
│                   #   CandidateRelation, TestEvidenceSummary, DiagnosticWitness,
│                   #   StaticLimitKind, EvidenceState vocabulary
├── analysis/       # Pipeline: diff loading, parsing, probe generation,
│                   #   classification, oracle extraction, repair-route readiness,
│                   #   seam inventory, test-grip evidence, value resolution
│   ├── diff/       #   Git diff loading and unified-diff parser
│   ├── facts/      #   Rust index build (ra_ap_syntax parsing)
│   ├── probes/     #   Probe generation from changed lines
│   ├── classify/   #   Owner resolution, related-test matching, decision cascade
│   ├── extract/    #   Oracle extraction and classification
│   ├── language/   #   Language adapters (rust, python, typescript/, perl/)
│   └── syntax/     #   ra_ap_syntax adapter for Rust fact extraction
├── app/            # Orchestration: check workspace, explain, context,
│                   #   agent brief/packet/verify/receipt, pr-summary,
│                   #   annotations, pr-evidence, impacted-evidence, ripr_plus
├── output/         # Rendering: human, JSON, SARIF, GitHub annotations,
│                   #   gate decision, repair packet, receipt, badge,
│                   #   evidence record, first-pr, review-comments, doctor
├── cli/            # Command dispatch: parse, execute, help, doctor report
│   └── commands/   #   Subcommand modules (agent, receipt, swarm, cache, policy)
├── lsp/            # LSP server: backend, diagnostics, hover, actions,
│                   #   capabilities, position, diagnostic_budget, refresh_scheduler,
│                   #   input_identity, agent_protocol, state, uri, gap_artifacts
├── agent/          # Repair loop: loop_commands, provenance
└── config/         # ripr.toml loading, typed model, Python project detection
```

## What's Working Well

### Core analysis pipeline

Diff → parse → probe → classify → output is clean, deterministic, well-tested,
and fail-closed on missing data. The classification cascade
(`classify/decision.rs`) is conservative: unknown is a valid terminal state.
The oracle matching covers 9 `OracleKind` values × 6 `OracleStrength` levels.

### Honesty infrastructure

- **Evidence-promotion-honesty corpus** (38 cases) catches dishonest re-blesses
  independently of goldens
- **No-panic-family gate** enforces panic-free code via semantic allowlist
- **Static-language gate** bans runtime-outcome vocabulary (`killed`, `survived`)
- **Spec lifecycle** (130 specs, numbered, format-checked, traceability-pinned)
- **Goldens** (200 fixtures, all green, zero drift)

### Fail-closed posture

Missing diff scope → disclosure. Dirty worktree → disclosure. Oversized diff
→ hard error with remediation. Missing discriminator → static limitation.
Incomplete repair route → no packet. These are not aspirational — they are
enforced in code and tested.

### CI policy gates

20+ `cargo xtask check-*` commands encode invariants that would otherwise drift:
file policy, executable files, no-panic-family, allow-attributes, workflows,
spec format, spec numbering, fixture contracts, traceability, capabilities,
workspace shape, architecture, public API, output contracts, generated files,
dependencies, network policy, process policy. These are the repo's structural
immune system.

## What's Structurally Wrong

### Integration boundaries are where bugs live

The core analysis is clean; the integration points — where the result meets
the gate, the LSP, the receipt, or the CI consumer — are where trust breaks
down. Each integration point has its own trust model, error handling, and
vocabulary, and these diverge from each other and from the core.

### The gate cannot block (#1933)

`repair_route.rs:114` hardcodes `seam_id: None` for gap-ledger candidates,
making `gate_repair_route_is_complete` always false. The blocking path is
non-functional for its primary use case. Tests named `*_fails_closed_*`
actually assert exit-0 (fail-open).

### The LSP loop re-runs everything (#1908)

Every `did_save` triggers `git diff` + repo walk + Rust re-index. The cache
exists but isn't wired into the diff path. The revision counter defeats dedup.
No debounce. 33ms–11s per save.

### Receipts are fabricable (#1941)

`ripr agent verify` reads two JSON files — no re-execution. `ripr receipt
write` stamps any caller-supplied SHA. The chain is advisory at the
verification layer.

### CI defaults to advisory (#2008/#2009/#2010)

`cargo install ripr --locked` on every PR (minutes). `continue-on-error` on
~30 steps. `RIPR_GATE_MODE` defaults to empty (never blocks). The default
consumer experience is green-nothing.

## What's Accumulating Debt

### Monoliths (29 files over 2,000 lines)

| File | Lines | Status |
|---|---|---|
| `xtask/src/main.rs` | 124,717 | Mostly inline tests (1,022 test fns) |
| `cli/commands.rs` | 10,514 | 133 handlers; 5 split, 128 inline |
| `analysis/language/python.rs` | 10,020 | Adapter + 2,000 lines of inline tests |
| `analysis/test_grip_evidence/tests.rs` | 11,645 | Test-only |
| `lsp/tests.rs` | 7,780 | Test-only |
| `output/agent_seam_packets.rs` | 4,884 | Cohesive but large |

### `Result<_, String>` (2,474 sites)

Every public function returns `Result<_, String>`. Zero typed error enums in
production. 1,254 `map_err(|err| format!("..."))` sites. This blocks
programmatic error handling for library consumers.

### Cross-language divergence

- `SeamKind` (7 variants) is Rust-only; preview adapters use `ProbeFamily`
  (8 variants) with no crosswalk
- Perl has its own `OracleKind`/`OracleStrength` (12/5 vs domain 9/6)
- Python/TS never emit `ReachableUnrevealed`, `InfectionUnknown`,
  `PropagationUnknown`
- `analyze_repo` is a stub for Python and TypeScript
- Run-status logic duplicated between `backend.rs` and `diagnostics.rs` with
  drift

### Single-threaded index build

`ra_ap_syntax` parsing is CPU-bound and embarrassingly parallelizable across
files, but the loop at `facts/build.rs:87` is sequential. No `rayon`.

## Next-Phase Investment Priorities

Ranked by product-contract impact:

### 1. Fix the gate blocking path (#1933)

Without a working blocking path, ripr cannot be a CI gate — which is its
product contract. One line (populate `seam_id` from the gap record) or one
honest documentation pass.

### 2. Wire the cache into the diff path (#1912)

`build_index_from_loaded_files_with_cache` exists and works. Wiring it into
`RustAdapter::analyze_diff` turns warm `ripr check` and LSP saves from "full
reparse" into "hash + small reads." Biggest performance win for smallest
change.

### 3. Add standard LSP progress (#1909)

`window/workDoneProgress` around the analysis lifecycle. Without it, the
editor experience feels broken (11s with no signal).

### 4. Enforce the diagnostic budget on push (#1911)

`evaluate_diagnostic_budget` is computed but not applied to the publish loop.
Apply it so large diffs don't overwhelm the client.

### 5. Introduce typed errors (#1914)

`thiserror`-based `RiprError` enum. Migrate module by module. Makes the
library API credible and enables programmatic error handling.

### 6. Decompose the monoliths (#1915, #1719)

`cli/commands.rs` and `python.rs` should follow the `cli/commands/{agent,
receipt, swarm, cache, policy}.rs` pattern. Zero golden drift is the proof.

### 7. Unify cross-language vocabulary (#1937, #1938, #1939)

Either produce `SeamKind` from each adapter or document it as Rust-only.
Reconcile Perl's oracle vocabulary. Extract the duplicated run-status logic.

### 8. Add LSP debounce + fix dedup identity (#1908)

Remove `saved_workspace_revision` from the dedup comparison or use a content
hash. Add a 200ms debounce in the refresh scheduler.

### 9. Harden the receipt chain (#1941)

Stamp the receipt with `git rev-parse HEAD` (not caller-supplied). Validate
snapshot provenance. At minimum, bind the head SHA to the actual repo state.

### 10. Improve the CI consumer experience (#2008, #2009, #2010)

Cache or pre-build the ripr binary. Remove `continue-on-error` from
load-bearing steps. Document the gate-mode variables inline.
