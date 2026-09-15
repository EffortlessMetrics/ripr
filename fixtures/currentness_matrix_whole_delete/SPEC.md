# Fixture: currentness_matrix_whole_delete

Spec: RIPR-SPEC-0156

## Given

A production owner `price` changed in `src/lib.rs` (boundary predicate
narrowed from `>=` to `>`), plus a whole-file delete of `src/gone.rs`
in the same diff. The input tree retains the deleted file so the diff
applies cleanly; the deletion is the diff's statement of the change.

## When

```bash
cargo xtask fixtures currentness_matrix_whole_delete
```

or:

```bash
ripr check --root fixtures/currentness_matrix_whole_delete/input --diff fixtures/currentness_matrix_whole_delete/diff.patch --mode fast
```

## Then

A whole-file deletion is disclose-not-probe: the deleted file
contributes no probe, no finding, and no repair target in either golden
(the deleted behavior has no candidate-side code to analyze), and the
run discloses the deletion as a named non-claim on the process's
diagnostic stream — that stderr disclosure is the disclosure surface
today and is not carried in `expected/check.json` or
`expected/human.txt`. The changed production owner in the same diff
stays `candidate_current` and actionable, proving the deletion adds no
obligation while the real seam keeps its teeth (#3212 matrix row 3).

## Must Not

- Emit any probe, finding, or repair route for the deleted file.
- Present the deleted file as clean or as a candidate edit target.
- Drop the production owner's ordinary classification.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
