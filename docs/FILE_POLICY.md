# File Policy

Rust is the default implementation language for this repository.

Use Rust and `xtask` for repo automation, production logic, test harnesses,
fixture runners, release checks, and policy checks. Non-Rust programming files
are allowlisted exceptions, not casual additions.

## Approved Non-Rust Surfaces

Non-Rust files are allowed when they belong to an approved surface:

- VS Code extension TypeScript under `editors/vscode`
- GitHub Actions workflow and issue-template YAML
- fixture inputs used by analyzer tests
- documentation and snippets
- assets
- generated or lock files with a documented owner

The allowlist lives in [non_rust_allowlist.txt](../policy/non_rust_allowlist.txt).

## Adding A Non-Rust Programming File

If a PR adds a non-Rust programming file, it must explain:

- why Rust or `xtask` is not the right place
- which approved surface owns the file
- whether the file is production, test, fixture, generated, config, or docs
- what CI or local check covers it

If the file does not match the current allowlist, update the allowlist with an
owner and reason in the same PR.

## Shell Scripts

Shell scripts are denied by default.

Allowed cases are narrow:

- small workflow `run` blocks that call Cargo, `cargo xtask`, npm, or release
  actions
- documentation examples
- fixture inputs with an explicit fixture spec

Prefer:

```bash
cargo xtask release
cargo xtask ci-fast
cargo xtask check-file-policy
```

Avoid:

```text
scripts/release.sh
scripts/check.sh
scripts/generate-schema.py
```

## Workflow Shell Budget

GitHub Actions YAML is necessary, but workflow `run` blocks should remain small
or delegate to Rust/npm tooling.

Workflow `run` blocks may:

- call Cargo commands
- call `cargo xtask`
- call npm scripts under `editors/vscode`
- set simple variables
- upload or download artifacts through actions

Workflow `run` blocks should not:

- parse JSON with shell
- implement release logic inline when an `xtask` command would be clearer
- use `curl | sh`
- contain complex loops or branching without a policy exception

Known workflow budgets are tracked in
[workflow_allowlist.txt](../policy/workflow_allowlist.txt).

## Executable Bits

Checked-in executable bits are denied by default. Use `cargo xtask` instead of
checked-in scripts. Fixture exceptions must be explicit and documented.

Executable exceptions are listed in
[executable_allowlist.txt](../policy/executable_allowlist.txt). The list should
usually stay empty.

## Checks

Run:

```bash
cargo xtask check-file-policy
cargo xtask check-executable-files
cargo xtask check-workflows
```

These checks are also included in `cargo xtask ci-fast` and CI.
