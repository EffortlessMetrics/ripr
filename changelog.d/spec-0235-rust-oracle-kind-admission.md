<!-- section: Docs -->
- Spec RIPR-SPEC-0235 (proposed) defines how a Rust related test earns its
  `oracle_kind` and `oracle_strength`: the existing precedence chain becomes
  normative, and six admission rules stop `assert_ne!`, guarded `Err(e)`
  patterns, `Ok(_)` side checks, substring identifier matches and inequality
  helper names from reading as strong or effect oracles (#5513).
  RIPR-SPEC-0107 gains one narrow `?` exception from RIPR-SPEC-0227.
