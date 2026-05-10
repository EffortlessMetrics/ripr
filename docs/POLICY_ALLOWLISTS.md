# Policy Allowlists

`ripr` uses TOML allowlist files under `policy/` as the control plane for
intentional exceptions to workspace-wide rules. Each allowlist entry is
structured, reviewed, and expiring. No exception lives as a bare comment or an
undocumented override.

## Canonical allowlists

| File | What it controls | Checked by |
| --- | --- | --- |
| `policy/no-panic-allowlist.toml` | Panic-family call sites (schema 0.3) | `cargo xtask check-no-panic-family` |
| `policy/non-rust-allowlist.toml` | Non-Rust programming files | `cargo xtask check-file-policy` |
| `policy/clippy-lints.toml` | Active and planned Clippy lint policy | `cargo xtask check-lint-policy` |
| `policy/clippy-debt.toml` | Temporary Clippy debt entries | `cargo xtask check-lint-policy` |
| `policy/clippy-exceptions.toml` | Per-site Clippy suppression receipts | `cargo xtask check-allow-attributes` |
| `policy/dependency_allowlist.txt` | Allowed crate dependencies | `cargo xtask check-dependencies` |
| `policy/ci-budget.toml` | LEM bands and enforcement posture | `cargo xtask ci plan` |
| `policy/ci-lane-whitelist.toml` | Lane definitions and base LEM | `cargo xtask ci plan` |
| `policy/ci-risk-packs.toml` | Changed-path → risk-pack mapping | `cargo xtask ci plan` |
| `policy/ripr-soft-gate.toml` | Soft-gate threshold and calibration | `cargo xtask check-pr` |

## Entry shape requirements

Every allowlist entry must include:

- **`id`** — stable identifier referenced in PR descriptions and ADRs.
- **`path`** or **`selector`** — file or structural location of the exception.
- **`owner`** — team or area responsible for the exception.
- **`reason`** or **`explanation`** — why the exception exists.
- **`expires`** — date the entry must be re-justified (no more than 12 months).

Entries without `owner`, `reason`, and `expires` are rejected by the relevant
`cargo xtask check-*` gate.

## Suppression governance

Source-level suppressions follow the same TOML-receipt model.

**Allowed form:**

```rust
#[expect(clippy::indexing_slicing, reason = "policy:clippy-0007: AST TextRange is prevalidated at construction")]
```

**Rejected forms:**

```rust
#[allow(clippy::indexing_slicing)]          // bare allow, no reason
#[allow(clippy::indexing_slicing, reason = "...")] // allow instead of expect
#[expect(clippy::indexing_slicing)]         // expect without reason
```

`clippy::allow_attributes_without_reason` is denied at the workspace level.
`cargo xtask check-allow-attributes` enforces that every source suppression
has a matching `policy/clippy-exceptions.toml` entry.

## No-panic allowlist transition

`policy/no-panic-allowlist.toml` (schema 0.3) is the canonical allowlist for
panic-family exceptions and is read by `cargo xtask check-no-panic-family`.
`.ripr/no-panic-allowlist.toml` (schema 0.2) is retained as a legacy
compatibility mirror while older branches drain.

The checker prints structured sections for allowed findings, advisory
`last_seen` drift, stale entries, unallowed findings, and warnings. Stale
entries, unallowed findings, duplicate semantic identities, unknown selector
kinds, blank explanations, and ambiguous selector matches fail the gate.

See `docs/NO_PANIC_POLICY.md` for the full policy and `docs/NO_PANIC_SEMANTIC_ALLOWLIST.md`
for the selector reference.

## Non-Rust file allowlist

`policy/non-rust-allowlist.toml` is the canonical allowlist for non-Rust
programming files. Every `.ts`, `.js`, `.py`, `.sh` file in the repo must
appear in this file or fail `cargo xtask check-file-policy`.

Required fields per entry: `owner`, `surface`, `classification`, `reason`,
`covered_by`.

See `docs/FILE_POLICY.md` for the full policy.
