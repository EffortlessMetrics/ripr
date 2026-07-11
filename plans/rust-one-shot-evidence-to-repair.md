# Rust One-Shot Evidence-to-Repair Plan

Status: active  
Owner: product-swarm  
Plan artifact: RIPR-PLAN-0062  
Linked goal: `.ripr/goals/active.toml`  
Linked issues: #1423, #1424, #1425, #1427, #1440  
Starting PRs: #1489, #1487, #1483

## Outcome

Make RIPR's Rust pull-request loop turn changed behavior into one exact, safe,
test-only repair and a verified before/after receipt. Avoid artifact archaeology,
known-ineffective recommendations, silent limitations, and full pipeline replay.

This supersedes RIPR-PLAN-0061 as the active execution sequence. Its accepted
contracts remain useful historical source, but its pending statuses and
maintainer-fixed order are not the current queue.

## Spec-system authority

Cargo-allow's opt-in `spec-system` profile is the authoritative structural
validator for governed artifact paths, kinds, lifecycle states, and graph links.
RIPR xtask remains the repo-facing proof executor and may invoke cargo-allow,
but must not independently reimplement the same graph rules. Every campaign
control-plane PR records cargo-allow doctor, audit, and worklist outputs.

The profile is advisory while its findings are made low-noise. The sole RIPR
execution manifest at `.ripr/goals/active.toml` is deliberately not enforced
until cargo-allow issue #2119 can validate its dialect without creating a second
active goal or discarding execution metadata. The installed cargo-allow 0.1.8
requires `--config .allow/profiles/spec-system.toml` for this owned profile;
cargo-allow issue #2117 tracks the owned-versus-legacy default-path friction.

## Reconciled sequence

| Order | Work item | Dependency | Evidence |
| ---: | --- | --- | --- |
| 0A | `control-plane/cargo-allow-spec-system-adoption` | — | cargo-allow doctor, audit, and worklist artifacts |
| 0B | `control-plane/rust-one-shot-goal` | 0A | goals/doc/plan checks and structural indexing |
| 0C | `control-plane/cargo-allow-active-goal-dialect` | 0A | blocked on cargo-allow #2119 or separately approved migration |
| 1A | `output/bounded-start-here` | 0 | human/human-full fixtures and output contracts |
| 1B | `docs/first-screen-agent-loop` | 1A | README/doc checks |
| 2A | `review/card-oracle-projection` | 0 | review-card schema and traceability checks |
| 2B | `review/canonical-working-set-id` | 2A | canonical identity output contracts |
| 3A | `gate/exact-repair-route` | 1A, 2B | failure-time packet fixture |
| 3B | `gate/concrete-targeted-mutation` | 3A | candidate/limitation matrix |
| 4A | `analysis/field-constant-observation` | 2B | positive and adversarial corpus |
| 4B | `analysis/constructor-field-observation` | 2B | same-crate ambiguity corpus |
| 5 | `perf/targeted-rerun` | 4A, 4B | benchmark and cache receipt |
| 6 | `dogfood/route-quality-closeout` | 3B, 5 | authorized receipts and support review |

Work items 1A and 2A may use isolated worktrees. Items 4A and 4B may be
parallel only after their file/contract overlap is checked. Dependencies are
landing dependencies, not approval gates.

## Contract

The default human surface has one `Start here:` state, at most one selected
item, an omitted count, and direct paths to exhaustive and JSON evidence.
Limited results never look clean. Actionable Rust packets use domain-supplied
canonical identity and complete repair, verify, receipt, and evidence fields;
partial packets fail closed. Gate output names the seam, missing discriminator,
test intent, verification, receipt, and explain route. Targeted mutation emits
a bounded candidate and command or a named limitation. Value-flow additions are
conservative and fixture-first. Targeted reruns disclose cache reuse and
invalidation, never silently serving stale evidence.

## Targeted rerun benchmark slice (2026-07-11)

The benchmark receipt command and controlled fixture are now available through
`cargo xtask targeted-rerun-benchmark`. The registered local receipt records a
matched parity result, warm targeted p50 of 204 ms, and 6.4069x cold-full to
warm-targeted p50 speedup on benchmark revision `29130a95`. It also exercises
an explicit file-fact cache reset and records `recomputed_file_facts`.

This closes only the benchmark-receipt slice. Broader input invalidation
attribution, test-node selectors, and full campaign closeout remain pending;
the active `perf/targeted-rerun` item stays open until those requirements are
demonstrated. PR #1532 additionally ships explicit `path::test_node`
selection for changed-test reruns; broader input invalidation attribution and
full campaign closeout remain pending.

## Gate route dogfood status (2026-07-10)

The structured route and shared human/generated-CI projection are shipped. A
CLI-backed calibrated blocking receipt now pins the full route directly in
`gate-decision.{json,md}` and rejects artifact-download or PR-guidance lookup as
the repair step.

The #1440 call-observation acceptance is not yet closed. Real current
CallPresence probes exercised through `ripr review-comments` remain
`static_limitation` when propagation to a side-effect/call-effect sink is
unknown; they therefore do not carry a policy-eligible receipt. Keep
`gate/exact-repair-route` open and fail closed rather than synthesizing a
blocking call-observation receipt. Resolving that producer limitation belongs
in an analysis-authority slice, not a gate renderer.

## Boundaries and closeout

No preview promotion, automatic test generation, default mutation execution,
default CI hardening, release/signing/publishing/credential work, workspace
restructure, or unsupported static-evidence claims. New dependencies and public
CLI/schema choices need separately accepted scope. Each item yields one PR,
planning update, or blocked report. Close only when items are landed,
superseded with evidence, or blocked; current-main proof and the protected
check are recorded; and source truth, receipts, benchmarks, limits, and support
review agree.
