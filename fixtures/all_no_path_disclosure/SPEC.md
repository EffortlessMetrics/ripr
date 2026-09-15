# Fixture: all_no_path_disclosure

Spec: RIPR-SPEC-0090

## Given

A Rust crate with a changed boundary predicate (`>` to `>=` in `apply_fee`) and
no co-located tests at all. Every finding the analyzer produces is `no_static_path`
because there is no test that reaches the changed owner.

## When

```bash
cargo xtask fixtures all_no_path_disclosure
```

or:

```bash
ripr check --root fixtures/all_no_path_disclosure/input --diff fixtures/all_no_path_disclosure/diff.patch --mode fast
```

## Then

`ripr` should emit the all-no-path advisory disclosure line after the findings:

```
Note: ripr found no static test path for any of the 1 changed expression(s) in this diff. This is not a coverage assessment — it means no co-located test was found that statically discriminates the changed behavior.
```

The disclosure must appear in the human output only (not JSON). The JSON output
must remain unchanged — no new fields, no schema version bump.

## Must Not

- Emit the disclosure in JSON output or modify `check.json` shape.
- Use mutation-testing runtime vocabulary that is banned by `check-static-language`.
- Claim the code is safe, that tests pass, or that behavior is correct.
- Suppress or alter the per-finding `no_static_path` output that already appears.
- Claim "no static test path" when a finding actually reaches. The unknown
  classes (`static_unknown` / `infection_unknown` / `propagation_unknown`) can
  carry `reach: yes` — a test does reach the change, ripr just cannot classify or
  propagate it. A reaching test IS a static test path, so the disclosure is
  suppressed whenever any finding's reach stage is `yes`, even if every finding's
  class is in the no-path-or-unknown set. (Dogfood: anyhow `Chain::len` — a
  `static_unknown` change reached by an integration test must not be reported as
  "no static test path".)
