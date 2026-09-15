# Fixture: benches_harness_evidence

Spec: RIPR-SPEC-0153

## Given

A Cargo bench lives under `benches/exposure.rs` (declared `[[bench]]`,
`harness = false`). Its body contains ordinary harness plumbing: a
predicate, a call into the production owner, and control flow that a
production probe would seed obligations for.

The diff changes the production owner `src/lib.rs` (a predicate inside
`price`) and the bench file in the same change.

## When

```bash
cargo xtask fixtures benches_harness_evidence
```

or:

```bash
ripr check --root fixtures/benches_harness_evidence/input --diff fixtures/benches_harness_evidence/diff.patch --mode fast
```

## Then

`ripr` emits findings only for the production owner. The changed bench
file stays in changed-file accounting and stays indexed as evidence, but
its harness plumbing (`bench_price`'s predicate and call) creates no
production probe, no finding, and no recursive obligation. This is the
#3283 source-role control for the diff path; the production owner keeps
its ordinary classification so the fixture also proves evidence routing
does not drop the real gap.

## Must Not

- Seed a production probe from the bench file's predicate, call, or control flow.
- Drop the bench file from changed-file accounting or from the index.
- Drop or weaken the production owner's ordinary classification.
- Reclassify benches files outside Cargo-autodiscovery shapes as evidence.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
