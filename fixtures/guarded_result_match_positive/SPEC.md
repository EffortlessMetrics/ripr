# Fixture: guarded_result_match_positive

Spec: RIPR-SPEC-0175

## Given

An owner `expect_ready(kind, len) -> Result<usize, ParseError>` whose
early-Err guards decide the returned error, and a test harness that
exercises the owner only through guarded Result matches over the direct
call — the same shape as the historical `expect_response` harness from
the #13162 comparison (#3709):

- a guarded Err arm with an exact error-variant equality in the guard
  and an empty accept body, routed by a loud catch-all arm
  (`result => panic!(..)`) — no `Ok` arm at all;
- a classic `Ok(..) => .., Err(Variant) => panic!(..)` form whose Err
  pattern names the exact variant.

## When

The diff flips the unready-kind rejection payload from
`ParseError::UnexpectedEof` (base) to `ParseError::InvalidData`
(current), changing which error variant the owner's Result carries for
the unready-kind input.

## Then

Both `error_path` and `return_value` probes on the changed
`return Err(ParseError::InvalidData);` line gain a producer-owned
observation: the guarded Result match oracle is bound to the owner
`expect_ready` (the match scrutinee is the direct call), so
`observe` and `discriminate` reach `yes` and the findings classify at
least `weakly_exposed` (the oracle text names the exact variant pin).

## Must Not

- Credit any oracle for the owner's bare name alone, a diagnostic
  string, or the harness's terminal `Ok(())`.
- Report the guarded matches as unrecognized smoke-only harness flow.
