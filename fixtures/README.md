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
- LSP code actions: `expected/lsp-code-actions.json`
- editor workflow projection: `expected/lsp-hover.md`,
  `expected/vscode-status.json`, and
  `expected/first-useful-action-status.json`
- GitHub annotations: `expected/github.txt`
- editor-agent loop: `expected/editor-agent-loop/`

Manifest-only fixture corpora can define their own checked shape when a
contract is not an executable diff workspace. `fixtures/editor_gap_cockpit`
uses nested cases with `expected/lsp-diagnostics.json`,
`expected/lsp-hover.md`, `expected/lsp-code-actions.json`,
`expected/vscode-status.json`, and `expected/gap-projection.json`; the shape is
validated by `cargo xtask check-fixture-contracts` and summarized by
`cargo xtask lsp-cockpit-report`. `fixtures/editor_first_pr_bridge` uses
nested cases with `expected/vscode-status.json`,
`expected/setup-diagnosis.md`, `expected/lsp-diagnostics.json`,
`expected/lsp-code-actions.json`, and `expected/first-pr-status.json` to pin
first-pr packet success and fail-closed states.

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
- defaults-first adoption examples: `opaque_fixture_builder`
- negative/noise cases: `format_only_diff`, `comment_only_diff`,
  `import_only_diff`, `unrelated_test_mentions_token`
- strong-oracle controls: `strong_boundary_oracle`, `strong_error_oracle`
- metamorphic syntax variants: `boundary_gap_multiline_assert`,
  `boundary_gap_nested_tests`, `boundary_gap_reordered_tests`,
  `weak_error_oracle_assert_matches`
- editor/LSP workflow projection: `editor_lsp_workflow`
- editor gap cockpit projection: `editor_gap_cockpit`
- editor first-pr bridge projection: `editor_first_pr_bridge`

For defaults-first adoption examples, see
[`EXAMPLE_CORPUS.md`](EXAMPLE_CORPUS.md). For calibration scenarios, see
[`CALIBRATION_CORPUS.md`](CALIBRATION_CORPUS.md). These indexes map existing
fixtures to proof-loop questions and bounded runtime artifacts without adding a
new fixture runner surface.
