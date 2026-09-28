# Fixture: match_arm_tuple_derived_return_before_assertion

Spec: RIPR-SPEC-0093

Corpus: RIPR-SPEC-0108 (`rust_tuple_derived_return_before_assertion_no_promotion`)

Issue: #1728 (PR #4068)

## Given

The derived-local tuple-arm shape (`terminalize_proof`): two immutable
booleans derived inside a `filter_map` closure select a relation through
`match (request_identity_matches, task_identity_matches)`, and the diff
changes the request-only arm `(true, false) => "request_identity_v2"`.

The only related test builds the request-only input, calls the owner, and
asserts the projection length and retained receipt identity, then executes a
direct `return;` before the relation assertion
`assert_eq!(terminal[0].1, "request_identity_v2");`.

## When

```bash
cargo xtask fixtures match_arm_tuple_derived_return_before_assertion
```

or:

```bash
ripr check --root fixtures/match_arm_tuple_derived_return_before_assertion/input --diff fixtures/match_arm_tuple_derived_return_before_assertion/diff.patch --mode fast
```

## Then

The changed request-only arm stays `weakly_exposed` with
`observation_unverified`. The derived projection admission only credits
observations that precede the containing test's first direct return; the
unreachable relation assertion cannot certify the arm.

The fixture runner passes a relative root with a directory prefix, so this
case also depends on the derived admission resolving the probe location
against the working directory (`tuple_match_derived.rs::same_current_file`).
Without that, the admission never runs under the runner and the case would
stay `weakly_exposed` for the wrong reason.

## Must Not

- Classify the changed arm `exposed` from an assertion that follows a direct
  `return` in the test body.
- Emit a repair packet, verify command, or receipt command that treats the
  unreachable assertion as the arm's discriminator.
