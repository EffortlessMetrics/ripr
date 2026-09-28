# CallPresence evidence packet

Status: blocked pending a qualifying producer receipt  
Goal: `rust-one-shot-evidence-to-repair`  
Issue: [#1543](https://github.com/EffortlessMetrics/ripr-swarm/issues/1543)

## Decision

Do not promote CallPresence actionability from the evidence currently
available. The repository contains extensive positive analyzer tests, but the
positive cases construct source and test files in memory through
`index_from_files`; they are synthetic fixture evidence, not a receipt from a
real changed Rust behavior. The current trust campaign therefore keeps the
CallPresence route fail-closed.

## Evidence inspected

| Basis | Result | Why it does or does not qualify |
| --- | --- | --- |
| `crates/ripr/src/analysis/test_grip_evidence/tests.rs` positive cases, including `given_call_presence_when_direct_owner_call_has_mock_expectation_then_activation_is_yes` and `given_call_presence_when_integration_test_calls_production_wrapper_then_activation_is_yes` | Positive unit coverage | These cases use inline synthetic files and prove analyzer behavior only; they do not prove a real changed behavior, current-head repair, or before/after receipt. |
| Current worktree `docs/rust-evidence-bound-repair-trust` at `79efe8d217730e67be538b87df3612a724da433c` | No qualifying changed behavior | The lane changes source-truth docs, policy metadata, and the scorecard command; it does not contain a production Rust behavior change with a CallPresence seam. |
| Latest recorded current-repository audit on `b921b69adc8572fbb9702869e4e0bb6490ab9f1a` | Bounded negative evidence | `repo-exposure-json` reported `run_status=seam_limit_applied`, 10,000 of 68,976 seams analyzed, 3,973 CallPresence seams, 782 `static_limitation`, 3,191 `not_policy_relevant`, and 0 `policy_eligible`. The SHA is not the current head, and the run does not contain an eligible route. |

The bounded audit is useful evidence that the route is not accidentally
eligible, but it cannot close the issue or establish adoption trust.

## Qualifying route still required

One authorized real or current-repository receipt must identify all of the
following from producer-owned facts:

1. repository and exact revision/head SHA;
2. changed Rust behavior and canonical gap ID;
3. exact CallPresence seam and `file:line`;
4. direct, unambiguous production caller;
5. observable call or effect sink;
6. matching observing test and discriminator;
7. test-only repair intent and allowed edit surface;
8. verify, targeted-rerun, receipt, and inspection commands;
9. before receipt, after receipt, movement, and current-head provenance.

The route must remain limited for dynamic or unresolved receivers,
method-name strings, ambiguous owners or aliases, helper-only reachability,
unrelated assertions, mere call presence without observed behavior, and
opaque macro-generated calls.

## Proof route

When an authorized case exists, run the issue proof before changing the
limitation:

```text
cargo test -p ripr analysis::test_grip_evidence --lib -- --test-threads=1
cargo test -p ripr output::gate --lib -- --test-threads=1
cargo test -p xtask --bin xtask dogfood_blocking_gate_report_is_self_contained -- --nocapture
cargo xtask goldens check
cargo xtask check-evidence-promotion-honesty
cargo xtask check-pr
```

The proof must inspect the emitted producer record and current-head receipt;
passing unit tests alone is insufficient. If no authorized repository,
revision, and qualifying route are available, leave #1543 open and record the
missing evidence rather than rerunning unchanged bounded scans.

## Claim boundary

This packet proves only that the current evidence is insufficient for
CallPresence promotion and records the concrete unlock condition. It does not
claim that CallPresence is impossible, that the analyzer has no positive
cases, or that static evidence is runtime mutation evidence.
