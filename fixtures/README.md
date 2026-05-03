# Fixture Contracts

Fixtures are BDD-style mini workspaces used to prove analyzer behavior and
output contracts. They should be readable by humans and agents without needing
chat context.

Each fixture directory should have this shape:

```text
fixtures/<name>/
  SPEC.md
  input/
    Cargo.toml
    src/lib.rs
    tests/<name>.rs
  diff.patch
  expected/
    check.json
    human.txt
    context.json
    lsp-diagnostics.json
    github.txt
```

Required for every fixture:

- `SPEC.md`
- `diff.patch`
- `expected/check.json`

`SPEC.md` must include:

- `Spec: RIPR-SPEC-NNNN`
- `## Given`
- `## When`
- `## Then`
- `## Must Not`

Optional expected outputs become required when the fixture claims that surface:

- human output: `expected/human.txt`
- agent context: `expected/context.json`
- LSP diagnostics: `expected/lsp-diagnostics.json`
- GitHub annotations: `expected/github.txt`

Run:

```bash
cargo xtask check-fixture-contracts
cargo xtask fixtures
cargo xtask fixtures <name>
cargo xtask goldens check
cargo xtask goldens bless <name> --reason "RIPR-SPEC-NNNN: explain change"
```

`cargo xtask fixtures` and `cargo xtask goldens check` run `ripr check` against
each fixture workspace, write actual JSON and human outputs under
`target/ripr/fixtures/<name>/`, and compare stable expected files. The checked
surfaces are:

- `expected/check.json`
- `expected/human.txt`, when present

`cargo xtask goldens bless <name> --reason "..."` is the only command that
updates expected output. It requires an explicit reason and appends
`expected/CHANGELOG.md`.

The current fixture baseline covers:

- primary behavior gaps: `boundary_gap`, `weak_error_oracle`, `snapshot_oracle`
- negative/noise cases: `format_only_diff`, `comment_only_diff`,
  `import_only_diff`, `unrelated_test_mentions_token`
- strong-oracle controls: `strong_boundary_oracle`, `strong_error_oracle`
- metamorphic syntax variants: `boundary_gap_multiline_assert`,
  `boundary_gap_nested_tests`, `boundary_gap_reordered_tests`,
  `weak_error_oracle_assert_matches`
