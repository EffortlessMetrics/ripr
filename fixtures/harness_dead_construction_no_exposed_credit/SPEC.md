# Harness dead construction keeps its oracle credit below exposed

Spec: RIPR-SPEC-0173

## Given

A registered libtest-mimic harness target (`tests/mimic.rs`,
`harness = false`) whose two trials reference the production functions
`parse_limit` and `limit_exceeds_default` only through dead
construction — an unused helper never passed to a harness run entry
point. The reachability authority (#3636) excludes the subjects from
the executable-test denominator with per-trial
`registration_unreachable` limitations while retaining the subject
facts and their syntactic claims.

## When

`diff.patch` changes the behavior of `parse_limit` (its fallback
value), which `limit_exceeds_default` also calls.

## Then

The changed production functions still produce findings, and every
finding classifies below `exposed`: the dead trials' oracles must not
credit exposure, because no runnable discriminator observes the
changed behavior.

## Must Not

No finding may be classified `exposed` on the strength of the dead
construction's oracles; the harness subjects must not re-enter the
executable-test denominator through a golden re-bless.
