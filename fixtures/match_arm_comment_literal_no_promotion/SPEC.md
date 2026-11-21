# Fixture: match_arm_comment_literal_no_promotion

Spec: RIPR-SPEC-0108

## Given

The changed production match arm maps `sensor` to `sensor-v2` instead of
`sensor-v1`. Its pattern comment contains the quoted text `"focused-test"`.
The only test calls `route("focused-test")` and compares the unchanged
sibling result `"proof"`. The comment does not add an alternative pattern.

## When

```bash
cargo xtask fixtures match_arm_comment_literal_no_promotion
```

or:

```bash
ripr check --root fixtures/match_arm_comment_literal_no_promotion/input --diff fixtures/match_arm_comment_literal_no_promotion/diff.patch --mode fast
```

## Then

The exact changed `match_arm` at `src/lib.rs:3` must remain
`weakly_exposed`, with `observation_unverified`. Preserve the changed arm's
source identity and the real sibling assertion; do not discard either to
avoid the finding. The enclosing match cannot substitute for the arm.

## Must Not

- Promote the changed sensor arm to `exposed` from a value found only in a
  comment.
- Treat nested comments or raw-string-looking comment text as arm values.
- Drop genuine raw or cooked string values containing comment delimiters.
- Weaken the existing aligned input/result positive controls.

## Evidence and qualification boundary

Source carry of swarm `22160dddd` (ripr-swarm#3766) under ripr#1714. The
donor's public-API negative reported `Exposed` instead of `WeaklyExposed`
pre-repair (fourteen of fifteen integration cases passing, the comment
negative failing); all fifteen pass post-repair. Source qualification
re-witnesses that red/green pair in this tree rather than inheriting it.

This fixture's source and full diff were checked for exact application and
reversal. Expected outputs (`check.json`, `human.txt`, `human-full.txt`)
were generated after the production correction and reviewed: one
`weakly_exposed` finding with `observation_unverified`, zero `exposed`.
The `rust_match_arm_comment_literal_oracle` honesty-corpus case pins that
non-promotion independent of the golden. No observed bad output is
accepted as a golden.
