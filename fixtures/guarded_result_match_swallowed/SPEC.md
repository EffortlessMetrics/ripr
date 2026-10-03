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

The guarded matches carry no recognized discriminator. Under RIPR-SPEC-0197,
the conditional `Ok`-arm bare equality assertions contribute no standalone
`return_value` or `error_path` oracle credit: both probes read `reachable_unrevealed` with
Observe `no`, Discriminate `no`, and related tests retained with no oracle.
The exact `ParseError::InvalidData` discriminator remains missing. Error-operand
meaning and guarded-match grammar are unchanged; #5027 applies the shared
execution admission before either family can borrow a conditional Ok equality.

## Must Not

- Credit a guarded Result match oracle for a swallowed or wildcard Err
  arm.
- Promote either finding to `exposed`.
