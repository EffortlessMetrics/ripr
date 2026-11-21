# Fixture: plain_diff_multifile_boundary

Spec: RIPR-SPEC-0076

## Given

A plain unified diff (no `diff --git` headers) spanning two Rust files:

- `src/a.rs` — changes `fn beta`'s predicate from `if x > 0` to `if x >= 0`.
- `src/b.rs` — changes `fn delta`'s predicate from `if x > 0` to `if x >= 0`.

The two file sections are separated only by `--- a/src/b.rs` / `+++ b/src/b.rs`
markers. Before the fix (issue #1222 RANK 2), when the parser encountered
`--- a/src/b.rs` while still inside the first hunk, it treated the line as a
hunk-body removed line (stripping the first `-`). This caused:

1. `src/b.rs`'s changes to be attributed to `src/a.rs` (wrong-target).
2. The `+++ b/src/b.rs` marker to appear as a phantom probe expression on `a.rs`.
3. `src/b.rs` to be omitted from `changed_rust_files`.

## When

```bash
cargo xtask fixtures plain_diff_multifile_boundary
```

or:

```bash
ripr check --root fixtures/plain_diff_multifile_boundary/input \
           --diff fixtures/plain_diff_multifile_boundary/diff.patch \
           --mode fast
```

## Then

`ripr` correctly identifies two changed files (`changed_rust_files=2`), emits
predicates probes for `fn beta` on `src/a.rs` and for `fn delta` on `src/b.rs`,
and does NOT emit any phantom probe whose expression text looks like a path
marker (`++ b/src/b.rs` or similar).

## Must Not

- Attribute `src/b.rs` changes to `src/a.rs`.
- Emit a phantom probe with `expression` matching a diff path-marker string.
- Report `changed_rust_files < 2`.
- Change the static-language outcome vocabulary.
