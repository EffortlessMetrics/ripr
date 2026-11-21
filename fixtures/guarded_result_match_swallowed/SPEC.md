# Fixture: guarded_result_match_swallowed

Spec: RIPR-SPEC-0175

## Given

The same owner shape as `guarded_result_match_positive`
(`expect_ready(kind, len) -> Result<usize, ParseError>` reached by
guarded Result matches over the direct call), but the harness never
discriminates: one match logs the Err arm and moves on
(`Err(error) => { eprintln!(..) }`), the other panics through a
wildcard arm with no variant pin (`Err(_) => panic!(..)`).

## When

The diff flips the unready-kind rejection payload from
`ParseError::UnexpectedEof` (base) to `ParseError::InvalidData`
(current).

## Then

The guarded matches carry no recognized discriminator, so the probes on
the changed line keep their existing weaker meaning: findings stay at
`weakly_exposed` (the `Ok`-arm value assertions remain ordinary exact
value oracles) and `discriminate` reports the unconfirmed state with the
missing `ParseError::InvalidData` discriminator named.

## Must Not

- Credit a guarded Result match oracle for a swallowed or wildcard Err
  arm.
- Promote either finding to `exposed`.
