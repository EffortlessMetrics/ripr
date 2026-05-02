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
```

The first fixture-laboratory PR will add concrete fixture directories. Until
then, this README defines the contract and the check passes when no fixture
directories exist.
