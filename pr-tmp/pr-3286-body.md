## Production delta

Closes #3286 — restores the helper-mediated production evidence silently lost by #3273, preserving its intended exclusion of test plumbing from production subjects.

**Regression (reproduced on main before the fix):** #3273 widened `FunctionFact::is_test` to plain helpers inside inline `#[cfg(test)]` modules; the helper-relation graph and local-name resolver filtered `!function.is_test`, so a `#[test] → cfg(test) helper → owner` path lost its `HelperOwnerCall` relation and degraded to the weak `SameTestFile` fallback — exactly the "evidence silently lost" invariant #3286 names.

**Fix (one seam, two builders):** helper-graph admission in `test_grip_evidence/related_tests/context.rs` now excludes only *actual* tests — `TestFact` members keyed on exact `(file, name, start_line)` identity — in `helper_owner_calls_by_file_with_fanout` (which both graph variants route through) and `local_function_names_by_file` (so sibling test-module names resolve as locals, not potential owner calls). Seam inventory, probe seeding, and production-owner name sets keep the wider #3273 role: evidence helpers never become production subjects.

## Evidence delta

- **Regression fixture written first and verified failing on main** (`given_full_evidence_when_cfg_test_module_helper_calls_owner_then_relation_is_retained`: pins the retained `HelperOwnerCall` relation with its oracle kind/strength, the non-promotion to `TestFact`, and the unchanged `is_test` evidence-role classification). Fixed build passes; **removal experiment** restoring the `!is_test` filter makes it fail again.
- Full suite 4290 green — including #3273's own producer controls, the same-file shadow controls, the delegation suite, and the physical `tests/**` helper relations. **Zero golden drift** (credible: no corpus fixture has an inline cfg(test) helper mediating an owner call — which is why the regression slipped through; this fixture closes that hole).
- Adversarial review (separate agent, all six challenge areas): no blocking findings; its suggestions applied — exact-identity keying (closing the same-file same-name over-exclusion and the triple-collision corner) and oracle pinning. Its verified boundary note: the target-affinity builders deliberately keep the wider role (cross-file qualified calls into cfg(test) modules are not a legal integration-test shape).

## Non-claims

- No Cargo target/bench/filename-convention role work (#3283 owns it); no assertion-form parity (#3284); no cross-surface projection (#3285).
- Cache generation intentionally untouched — #3287 bumps the fact/seam cache generations immediately after this lands to invalidate pre-repair semantic entries.

Candidate head: `a94dcbfe` (base `origin/main` @ `7563793a`).
