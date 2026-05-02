# Fixture: snapshot_oracle

Spec: RIPR-SPEC-0002

## Given

Production code broadens a success condition from `code == 200` to
`code <= 299`.

A related test uses a snapshot assertion over an error rendering path.

## When

```bash
cargo xtask fixtures snapshot_oracle
```

or:

```bash
ripr check --root fixtures/snapshot_oracle/input --diff fixtures/snapshot_oracle/diff.patch --mode fast
```

## Then

`ripr` should detect related reachability and classify the snapshot assertion as
an observing oracle with medium strength while retaining conservative language.

## Must Not

- Claim runtime mutation outcomes.
- Hide unknown/static gaps when only snapshot evidence is present.
