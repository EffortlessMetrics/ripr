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

## Generated Files

Generated build output, downloaded release assets, archives, packaged VSIX
files, and local cache output should not be checked in.

Intentional generated artifacts must be allowlisted:

- Cargo and npm lockfiles
- fixture golden outputs
- future generated schema or metrics artifacts with explicit owners

Generated-file exceptions are listed in
[generated_allowlist.txt](../policy/generated_allowlist.txt).

## Dependency Surfaces

Dependency managers are approved only on explicit surfaces:

- Cargo workspace and package manifests
- the VS Code extension npm manifest and lockfile
- fixture input manifests used as analyzer inputs

New dependency manager files must be allowlisted with owner and reason. The
allowlist lives in [dependency_allowlist.txt](../policy/dependency_allowlist.txt).

## Process And Network Surfaces

Process spawning and network access belong in explicit adapter, tooling, release,
or extension surfaces.

Allowlisted process-spawn surfaces live in
[process_allowlist.txt](../policy/process_allowlist.txt). Allowlisted network
surfaces live in [network_allowlist.txt](../policy/network_allowlist.txt).

## Checks

Run:

```bash
cargo xtask check-file-policy
cargo xtask check-executable-files
cargo xtask check-workflows
cargo xtask check-generated
cargo xtask check-dependencies
cargo xtask check-process-policy
cargo xtask check-network-policy
```

These checks are also included in `cargo xtask ci-fast` and CI.
