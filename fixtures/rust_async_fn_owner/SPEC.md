# Fixture: rust_async_fn_owner

Spec: RIPR-SPEC-0001

## Given

An `async fn` owner changes its predicate from `>=` to `>`. A real
`#[tokio::test]` awaits `fetch_limit(100, 50)` and asserts the result equals 90.

## When

The analyzer reads the fixture's exact production diff.

## Then

Owner and test discovery are retained. The bounded equality-execution query
(RIPR-SPEC-0197, #5027) cannot establish the external test macro binding and
async polling path, so the assertion supplies no static oracle credit.
Observe/Discriminate are No/No and strength is None. Classification remains
`propagation_unknown`; advisory confidence changes from 0.66 to 0.43.

This is an explicit conservative usefulness regression, not evidence that the
actual test is ineffective. #5040 owns typed async-harness execution provenance
and must restore the positive without admitting shadowed/custom/unpolled cases.
An attribute-name allowlist is not a substitute for that producer fact.

Two independent limitations remain separate:

- The one-line owner lacks a resolved propagation sink; a synchronous version
  of this source has the same propagation limitation
- Input 100/50 is not equality, so it cannot discriminate `>` from `>=`

## Executed runtime and removal controls

The actual fixture manifest and Tokio macro/runtime were used, without a
substitute executor: rustc 1.95.0, Tokio 1.53.1 and tokio-macros 2.7.2, resolved
offline. All six subjects compiled, listed one test and executed one test.

| Predicate | Original equality | Equality replaced by `let _ = result` |
|---|---|---|
| Current `>` | pass | pass |
| Previous boundary `>=` | pass | pass |
| Inverted `<=` | fails, 100 vs 90 | pass |

The assertion really executes and detects the inverted predicate. The surviving
previous-boundary mutant confirms the separate missing-boundary limitation.
Runtime observations do not manufacture static execution provenance.

## Must Not

- Report `exposed` while propagation or the exact boundary is unestablished
- Lose the async owner or the indexed test
- Describe the real equality as ineffective merely because its static execution
  provenance is unsupported
- Restore strong credit by trusting a Tokio-like spelling, custom/rebound
  attribute, disabled/raw-CFG context, or unpolled future
