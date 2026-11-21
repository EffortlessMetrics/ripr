# Fixture: multi_hunk_removed_line_wrong_target

Spec: RIPR-SPEC-0075

## Given

A two-hunk git diff against a single Rust file:

- Hunk 1 replaces one body line in `fn one` with three lines (net +2 lines).
- Hunk 2 changes `fn two`'s predicate from `if x > 0` to `if x >= 0`.

Before this fix (issue #1222 RANK 1), the removed-side probe for `if x > 0 {`
was reported using the OLD-side line counter (6) joined to the new file path.
After the +2 net delta from hunk 1, new-file line 6 is `pub fn two(x: i32) ->
bool {` — a completely different function declaration (wrong-target honesty bug).

## When

```bash
cargo xtask fixtures multi_hunk_removed_line_wrong_target
```

or:

```bash
ripr check --root fixtures/multi_hunk_removed_line_wrong_target/input \
           --diff fixtures/multi_hunk_removed_line_wrong_target/diff.patch \
           --mode fast
```

## Then

`ripr` emits the `predicate` probe for the changed `fn two` at the **correct**
new-file line (8, where `if x >= 0 {` now lives), NOT at the wrong-target line
that happens to share the old-side line number.

The added-side `if x >= 0 {` probe at line 8 is the primary signal. The
removed-side `if x > 0 {` probe must not point at a line inside `fn one`.

## Must Not

- Report any probe whose `file` + `line` combination points at code inside
  `fn one` using the removed-side expression text from `fn two`.
- Emit a probe for `++ b/src/lib.rs` or similar phantom path-marker text.
- Change the static-language outcome vocabulary.
