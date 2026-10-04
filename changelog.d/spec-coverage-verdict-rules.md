<!-- section: Docs -->
- Specs RIPR-SPEC-0232, RIPR-SPEC-0233, and RIPR-SPEC-0227 through0229 (proposed) define five Rust verdict
  rules that had no spec: whole-value `assert_eq!` credit for constructed
  fields, agreement between findings on one line, result-side oracles
  (`is_err`, `is_ok`, `unwrap_err`, `should_panic`), field writes, and
  match-arm selection (#5432). Each marks the product choices left to the
  owner.
