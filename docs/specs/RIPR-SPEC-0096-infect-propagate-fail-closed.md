# RIPR-SPEC-0096: INFECT/PROPAGATE Fail-Closed (Parts A, B, C)

Status: proposed

Owner: product / swarm

Created: 2026-06-13

Linked proposal:

- None

Linked ADRs:

- None

Linked plan:

- None

Linked issues:

- #1219 — INFECT/PROPAGATE stages fail-open → exposed inflation

Linked PRs:

- None yet

Support-tier impact:

- Narrows false-actables from `exposed` to `infection_unknown` or
  `propagation_unknown` for three provably-non-observable patterns:
  wildcard discards (`let _ = …`), swallowed call-chain tails (`.ok();`,
  `drop(…)`), and stdout macros (`println!`/`eprintln!`).
  Tier governance and tier labels for "Rust static exposure loop" remain
  unchanged (see [SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)).
  This is a honesty improvement: fewer false-actionable findings.

Policy impact:

- No new crates, binaries, dependencies, parsers, or LSP servers.
- Register this spec in `policy/doc-artifacts.toml`.
- No schema version bump (no new JSON output fields; existing
  `infection_unknown` / `propagation_unknown` classifications already
  exist in the schema).

## Problem

Three sub-cases where `infection.rs` and `flow.rs` emit `Yes` (→ `exposed`)
when they provably cannot observe the changed behavior:

### A — Wildcard discard (INFECT)

`infection.rs:80-94` `_ =>` catch-all returns `StageState::Yes` on bare
test-presence when the probe expression is a wildcard-discard binding
(`let _ = ...`).  A discarded value cannot infect any sink.

### B — Swallowed call-chain tail (PROPAGATE)

`flow.rs:75-82` (`SideEffect|CallDeletion` else branch) falls through to
`effect_sink_kind` for `let _ = f()`, `f().ok();`, `drop(f())` — none of
which propagate the value to an observable boundary.

### C — Non-escaping effect sinks: stdout macros and local receivers (PROPAGATE)

`flow.rs:17-29` scores `Yes` for `sink.kind != Unknown`, so:
- `println!`/`eprintln!` (not a static-observable boundary; `log::`/
  `tracing::` are capturable and kept as `LogMessage`).
- `.push/.insert/.write` on a provably function-local-dropped receiver.
- Trait-object receivers (`&dyn`/`Box<dyn`): dispatch target unknown.

### D (deferred)

Confidence aggregation in `decision.rs` (part D of #1219) is a calibration
concern; it is tracked separately and not addressed in this PR.

## Behavior

### A — INFECT: wildcard discard → `infection_unknown`

If `probe.expression.trim_start().starts_with("let _ =")` or
`starts_with("let _:")` (exact wildcard discard, NOT `let _name`), return
`StageEvidence { state: Unknown, confidence: Low, summary: "Changed value
is bound to a discard pattern; it cannot infect a sink" }`.

Named bindings (`let _name = …`, `let result = …`) are NOT affected —
they may be used downstream and remain `Yes`.

### B — PROPAGATE: swallowed tail → `FlowSinkKind::Unknown`

`value_is_swallowed(text)` matches the call-chain TAIL:

- `let _ = <expr>` — wildcard-discard binding
- trailing `.ok();` — Result→Option conversion and drop
- `drop(…)` wrapper — explicit discard
- `= ();` — unit assignment

NOT: a loose `contains(".ok(")` — `x.ok().map(f)` is not swallowed.

When `value_is_swallowed` fires in the `SideEffect|CallDeletion` else
branch, emit `FlowSinkKind::Unknown`.  `propagation_evidence` already
filters `kind != Unknown` → `propagation_unknown`.

### C — PROPAGATE: non-escaping sinks → `FlowSinkKind::Unknown`

`is_non_escaping_effect(text, owner_fn)` covers three sub-cases:

1. `println!(…)` / `eprintln!(…)` — stdout macros route to
   `FlowSinkKind::Unknown`.  `log::` / `tracing::` are NOT matched;
   they stay as `LogMessage`.
2. `.push/.insert/.write` on a provably function-local-dropped receiver:
   receiver is not `self`, has a `let [mut] <recv> = …` binding in
   `owner_fn.body`, and no subsequent `return <recv>` / `self.<f> = <recv>`.
3. `&dyn` / `Box<dyn` trait-object receiver — dispatch target is opaque.

Preserve sink-KIND labeling (display); gate only the propagate STATE on
escape.

## Non-Goals

- Part D: confidence aggregation (deferred to follow-up PR).
- Phase 2 of A: `dominated_by_earlier_unconditional_terminator` and
  `clobbered_before_read` (deferred; these require deeper AST analysis).
- Changing `decision.rs`, `reveal.rs`, or output renderers.
- Schema version bump.

## Acceptance Examples

### A — Wildcard discard (must downgrade)

```
probe family:  call_deletion
expression:    let _ = compute_fee(amount * 9);
test:          assert_eq!(process(42), 42);

Before fix:    exposed / confidence 1.0
After fix:     infection_unknown / confidence 0.74
```

### A — Named binding (must stay exposed)

```
probe family:  call_deletion
expression:    let result = helper(amount * 9);
test:          assert_eq!(score(10), 10);

Before fix:    exposed / confidence 1.0
After fix:     exposed / confidence 1.0   (unchanged — named binding still used)
```

### B — Swallowed tail (must downgrade)

```
probe family:  call_deletion
expression:    self.persist(amount * 9).ok();
test:          assert_eq!(ledger.balance(), 145);

Before fix:    exposed / confidence 1.0
After fix:     propagation_unknown / confidence 0.87
```

### B — Returned call (must stay)

```
probe family:  side_effect / call_deletion
expression:    return self.persist(amount * 9);
test:          assert!(ledger.apply(5).is_ok());

Before fix:    weakly_exposed / confidence N
After fix:     weakly_exposed / confidence N   (unchanged — return is not swallowed)
```

### C — stdout macro (must downgrade)

```
probe family:  side_effect
expression:    println!("amount is {}", amount * 9);
test:          report(5);  // smoke only

Before fix:    exposed / confidence N
After fix:     propagation_unknown / confidence N
```

### C — log macro (must stay)

```
probe family:  side_effect
expression:    log::info!("amount is {}", amount * 9);
test:          report(5);  // smoke only

Before fix:    reachable_unrevealed / propagation yes
After fix:     reachable_unrevealed / propagation yes   (unchanged — log:: is capturable)
```

## Required Evidence

### A — Wildcard discard

- `fixtures/infect_wildcard_discard`: `let _ = compute_fee(amount * 9)` +
  strong test → must produce `infection_unknown`.
- `fixtures/infect_value_returned` (control): `let result = helper(…)`
  used in `result + 1` → must stay `exposed`.

### B — Swallowed tail

- `fixtures/propagate_swallowed_ok`: `self.persist(amount * 9).ok();` +
  strong test → must produce `propagation_unknown`.
- `fixtures/propagate_value_returned` (control): `return self.persist(…)`
  → must stay `exposed`/`weakly_exposed` (not `propagation_unknown`).

### C — Stdout macro / log

- `fixtures/propagate_stdout_macro`: `println!(…)` → must produce
  `propagation_unknown`.
- `fixtures/propagate_log_message` (control): `log::info!(…)` → must NOT
  produce `propagation_unknown` (should be `reachable_unrevealed` or
  `exposed` depending on oracle strength).

## Inputs

- A diff touching a `SideEffect`, `CallDeletion`, or `ReturnValue` probe
  whose expression matches one of the provably-non-observable patterns.

## Outputs

- `infection_unknown` (severity: warning) for wildcard-discard probes.
- `propagation_unknown` (severity: note) for swallowed-tail or
  non-escaping-effect probes.
- No new JSON fields; `infection_unknown` / `propagation_unknown` already
  exist in `schema_version: "0.2"`.

## Test Mapping

- `crates/ripr/src/analysis/classify/infection.rs::tests::wildcard_discard_is_infection_unknown_even_with_related_tests`
- `crates/ripr/src/analysis/classify/infection.rs::tests::typed_wildcard_discard_is_infection_unknown`
- `crates/ripr/src/analysis/classify/infection.rs::tests::named_binding_is_not_a_discard_stays_yes`
- `crates/ripr/src/analysis/classify/infection.rs::tests::return_value_read_into_tail_stays_exposed`
- `crates/ripr/src/analysis/classify/flow.rs::tests::swallowed_ok_tail_yields_unknown_sink`
- `crates/ripr/src/analysis/classify/flow.rs::tests::wildcard_discard_binding_yields_unknown_sink`
- `crates/ripr/src/analysis/classify/flow.rs::tests::drop_wrapper_yields_unknown_sink`
- `crates/ripr/src/analysis/classify/flow.rs::tests::returned_call_is_not_swallowed_stays_return_value`
- `crates/ripr/src/analysis/classify/flow.rs::tests::chained_ok_map_is_not_swallowed`
- `crates/ripr/src/analysis/classify/flow.rs::tests::println_macro_yields_unknown_sink`
- `crates/ripr/src/analysis/classify/flow.rs::tests::eprintln_macro_yields_unknown_sink`
- `crates/ripr/src/analysis/classify/flow.rs::tests::log_info_macro_stays_log_message_not_downgraded`
- `crates/ripr/src/analysis/classify/flow.rs::tests::local_vec_push_yields_unknown_sink`
- `crates/ripr/src/analysis/classify/flow.rs::tests::self_field_push_stays_state_write_not_downgraded`
- `fixtures/infect_wildcard_discard`
- `fixtures/infect_value_returned`
- `fixtures/propagate_swallowed_ok`
- `fixtures/propagate_value_returned`
- `fixtures/propagate_stdout_macro`
- `fixtures/propagate_log_message`

## Implementation Mapping

- `crates/ripr/src/analysis/classify/infection.rs` — `is_wildcard_discard`
  + guard in `_ =>` arm.
- `crates/ripr/src/analysis/classify/flow.rs` — `value_is_swallowed`,
  `is_non_escaping_effect`, `is_stdout_macro`, `has_trait_object_receiver`,
  `collection_receiver`, `is_function_local_dropped_receiver`,
  `looks_like_field_store_of`.

## CI Proof

- `cargo xtask fixtures infect_wildcard_discard`
- `cargo xtask fixtures infect_value_returned`
- `cargo xtask fixtures propagate_swallowed_ok`
- `cargo xtask fixtures propagate_value_returned`
- `cargo xtask fixtures propagate_stdout_macro`
- `cargo xtask fixtures propagate_log_message`
- `cargo xtask goldens check`
- `cargo test -p ripr`

## Metrics

- `wildcard_discard_downgrades_to_infection_unknown`: fixture
  `infect_wildcard_discard` produces `infection_unknown` for
  `let _ = compute_fee(amount * 9)` (validated by fixtures runner).
- `swallowed_ok_downgrades_to_propagation_unknown`: fixture
  `propagate_swallowed_ok` produces `propagation_unknown` for
  `self.persist(amount * 9).ok()` (validated by fixtures runner).
- `stdout_macro_downgrades_to_propagation_unknown`: fixture
  `propagate_stdout_macro` produces `propagation_unknown` for
  `println!(…)` (validated by fixtures runner).

## Failure Modes

- A: `let _:` prefix check covers typed discards but NOT destructuring
  (`let (_, b) = …`). Those patterns fall through to `Yes` (fail-open).
  This is intentional — tuple destructuring may still use `b`.
- B: Only exact tail patterns are matched; compound chains (e.g.
  `.ok().unwrap()`) fall through to `Yes` (fail-open). Intentional.
- C: Local-receiver detection uses string matching on owner body; it can
  miss complex patterns (closures, nested functions). Intentional — we
  only downgrade what we can prove.
- D (deferred): Confidence aggregation still sums states without reading
  per-stage confidence. Tracked in #1219 part D.
