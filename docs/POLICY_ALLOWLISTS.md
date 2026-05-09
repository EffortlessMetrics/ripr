# Policy Allowlists

`ripr` uses structured TOML allowlists to track every intentional exception to the default
deny posture. This page catalogs what exists, where each file lives, who owns it, and what
checker validates it.

## Principle

Allowlists are the **control plane for exceptions**. The defaults are strict everywhere.
Exceptions are visible, reviewable, and expiring. An exception without a TOML receipt is a
violation, not a gray area.

Every allowlist entry records: what is allowed, why, who owns it, which checker validates it,
and when the justification expires.

## Allowlist Catalog

### `policy/no-panic-allowlist.toml` (target) / `.ripr/no-panic-allowlist.toml` (current)

**Purpose**: Track intentional panic-family call sites (unwrap, expect, panic!, unreachable!,
todo!, unimplemented!) in production and test code.

**Checker**: `cargo xtask check-no-panic-family`

**Schema**: 0.3 (target in `policy/`); 0.2 (current in `.ripr/`)

**Identity**: `path + family + selector` — never `path + line + column`

**Key fields**: `id`, `path`, `family`, `classification`, `owner`, `explanation`, `expires`,
`[allow.selector]` (kind + container + callee)

**Transition**: PR 04 makes `policy/no-panic-allowlist.toml` canonical and updates the checker.

See [`docs/NO_PANIC_POLICY.md`](NO_PANIC_POLICY.md) for the full policy.

---

### `policy/non-rust-allowlist.toml`

**Purpose**: Track every non-Rust programming file in the repository. Rust is the default;
non-Rust files require an explicit receipt.

**Checker**: `cargo xtask check-file-policy`

**Key fields**: `path`, `owner`, `surface`, `classification`, `reason`, `covered_by`

**Surfaces**: `vscode_extension`, `github_actions`, `fixture_input`, `docs`, `generated`,
`config`, `lock`

Today's only retained programming surface outside Rust is the VS Code extension TypeScript
under `editors/vscode`, because it runs inside the VS Code Extension Host and binds directly
to VS Code's TypeScript API.

See [`docs/FILE_POLICY.md`](FILE_POLICY.md) for the full policy.

---

### `.ripr/allow-attributes.txt`

**Purpose**: Track every `#[allow(...)]` and `#[expect(...)]` attribute in the workspace so
that suppressions are visible and reviewable.

**Checker**: `cargo xtask check-allow-attributes`

**Format**: One entry per line: `path:item:lint`

**Policy**: Every suppression must carry `reason = "..."`. Bare `#[allow(...)]` without
reason is rejected by `allow_attributes_without_reason = "deny"`. After PR 06, the xtask
checker will also reject `#[expect(...)]` without a reason and `#[allow(...)]` not
referenced in this file.

For durable exceptions, the reason should reference a policy ID:
```rust
#[expect(clippy::indexing_slicing, reason = "policy:clippy-0007: AST text range is prevalidated")]
```

---

### `policy/clippy-lints.toml`

**Purpose**: Declarative ledger of the Clippy lint policy. Documents which lints are active,
at what level, and why. Also records planned lints with the MSRV at which they will be
activated.

**Checker**: `cargo xtask check-lint-policy`

**Key sections**: `[[active.*]]` (active by category), `[[planned]]` (queued for MSRV flip)

The authoritative lint configuration lives in `[workspace.lints.*]` in `Cargo.toml`.
`policy/clippy-lints.toml` is the human-readable ledger that records intent and trajectory.

---

### `policy/clippy-debt.toml`

**Purpose**: Track temporary Clippy debt — lint hits that are not yet fixed and are deferred
with an expiry. Every entry must have an owner and a deadline.

**Checker**: `cargo xtask check-lint-policy` (advisory, not blocking on debt count)

**When to use**: When a lint flip exposes widespread hits that cannot be fixed atomically in
the activation PR. Record the debt, fix it promptly.

---

### `policy/clippy-exceptions.toml`

**Purpose**: Track Clippy suppressions that are genuinely permanent or long-lived (e.g.,
AST-bounded slice indexing where the invariant is documented). Short-lived suppressions
go in `clippy-debt.toml`.

**Checker**: `cargo xtask check-lint-policy`

---

### `policy/ci-budget.toml`

**Purpose**: Machine-readable CI budget ledger. Records LEM band definitions, label effects,
and the enforcement state of the budget guard.

**Checker**: `cargo xtask check-ci-lane-whitelist` (structural validation)

**Key fields**: `[[budget_band]]` (id, min_lem, max_lem, posture), `[[label]]` (name, effect,
budget_effect), `[defaults]` (budget_guard, actuals_required)

See [`docs/ci/lem-budgeting.md`](ci/lem-budgeting.md).

---

### `policy/ci-lane-whitelist.toml`

**Purpose**: Registry of allowed CI lane IDs and artifact families. Any lane that runs in CI
must be registered here; unregistered lanes fail `check-ci-lane-whitelist`.

**Checker**: `cargo xtask check-ci-lane-whitelist`

---

### `policy/ci-risk-packs.toml`

**Purpose**: Maps changed path patterns to required, advisory, and on-demand lane sets.
The PR Plan planner reads this to select lanes for a given diff.

**Checker**: `cargo xtask check-ci-lane-whitelist` (structural) + PR Plan planner (runtime)

---

### `policy/dependency_allowlist.txt`

**Purpose**: Track approved crate dependencies. New crates not on the list require an
explicit addition with owner and reason.

**Checker**: `cargo xtask check-dependencies`

---

### `policy/executable_allowlist.txt`

**Purpose**: Track files that are executable (`+x`). Unexpected executables are a supply-
chain risk; this allowlist makes them visible.

**Checker**: `cargo xtask check-executable-files`

---

### `policy/network_allowlist.txt`

**Purpose**: Track network endpoints that production code or CI may contact. Unlisted
endpoints fail policy.

**Checker**: `cargo xtask check-network-policy`

---

### `policy/process_allowlist.txt`

**Purpose**: Track process spawns. Unexpected process spawns are a supply-chain signal;
this allowlist makes them visible.

**Checker**: `cargo xtask check-process-policy`

---

### `policy/generated_allowlist.txt`

**Purpose**: Track generated files. Generated files must be declared so the generator is
known and reviewable.

**Checker**: `cargo xtask check-generated`

---

### `.ripr/traceability.toml`

**Purpose**: Spec → tests → code map. Links spec IDs to test names and source paths for
every tracked behavior.

**Checker**: `cargo xtask check-traceability`

---

### `.ripr/static-language-allowlist.toml`

**Purpose**: Track intentional uses of static-language terms that the `check-static-language`
gate would otherwise reject (e.g., a test that explicitly verifies the gate rejects forbidden
words).

**Checker**: `cargo xtask check-static-language`

---

## Adding a New Allowlist

If a new policy domain needs tracking:

1. Define the schema in a new `policy/*.toml` file with `schema_version`, `owner`, and
   `reason` at the top.
2. Write a `cargo xtask check-<name>` command that reads and validates the file.
3. Register the checker in CI.
4. Document it in this file.
5. Add it to `policy/non-rust-allowlist.toml` if the TOML file needs a non-Rust annotation
   (it usually does not — TOML policy files are exempted from the non-Rust allowlist because
   they are data, not programming language files).

## See Also

- [`docs/FILE_POLICY.md`](FILE_POLICY.md) — non-Rust file policy
- [`docs/NO_PANIC_POLICY.md`](NO_PANIC_POLICY.md) — no-panic policy
- [`docs/CLIPPY_POLICY.md`](CLIPPY_POLICY.md) — lint policy
- [`docs/ENGINEERING.md`](ENGINEERING.md) — engineering rules
