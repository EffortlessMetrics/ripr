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

## Related policy ledgers

Not every policy ledger is an allowlist. Baselines, waivers, and RIPR
suppressions keep different review meanings so policy can stay auditable
without hiding evidence.

| Ledger | Policy role | Checked by |
| --- | --- | --- |
| `.ripr/suppressions.toml` | Durable RIPR finding suppressions with owner, reason, scope, review state, visibility, and language status. | `ripr policy suppression-health` |
| `.ripr/gate-baseline.json` | Reviewed historical debt checkpoint for baseline-aware gate modes. | `ripr baseline diff` |
| PR `ripr-waive` labels and PR evidence history | Visible PR-time acknowledgements and waiver-aging signals. | `ripr pr-ledger record` and `ripr policy waiver-aging` |

## Shared exception semantics

All exception ledgers use the same review model where their concrete schema
allows it:

- one exception maps to one reviewed reason;
- an exception does not create a budget for more exceptions;
- semantic selectors, canonical gap identities, globs, or policy ids are the
  durable identity when available;
- line and column values are locators only, not durable identity;
- stale entries warn or block according to their class and rollout posture;
- suppressed, baselined, and waived evidence remains visible in the relevant
  policy reports.

| Ledger | Exception meaning | Durable identity | Review signal | Stale or growth behavior |
| --- | --- | --- | --- | --- |
| No-panic allowlist | A reviewed panic-family call site that remains exceptional. | `path + family + selector`. | `owner`, `explanation`, and expiry. | Stale, duplicate, ambiguous, or unallowed entries block the no-panic gate. |
| Clippy lint/debt/exception ledgers | Active lint policy, deferred lint flips, and per-site source suppressions. | Lint id plus selector/path and source `#[expect(..., reason = "...")]`. | Ledger `owner`, `reason`, `covered_by`, and source reason. | Planned/debt entries stay explicit; bare or unmatched suppressions block the allow-attribute check. |
| Non-Rust allowlist | A reviewed non-Rust file allowed inside a Rust-first repo. | Glob plus surface/classification. | `owner`, `reason`, `covered_by`, surface, and classification. | Missing or unclassified files block file-policy checks; retired entries are reviewed cleanup candidates. |
| Workflow allowlists | Bounded workflow run-block or action-runtime exceptions. | Workflow path plus line/count or pattern cap. | Reviewed reason in the text ledger. | Caps are ceilings, not budgets; stale entries should be removed when the workflow no longer needs them. |
| RIPR suppressions | A durable policy exception for a configured RIPR finding. | Finding selector or canonical identity plus scope. | `owner`, `reason`, scope, review date, static class, visibility, and language status. | `suppression-health` flags missing metadata, stale review windows, overbroad scope, unknown selectors, and preview suppressions without preview labels. |
| Gate baselines | Known-before-policy debt that remains visible while adoption tightens. | Baseline identity such as canonical gap id, seam id, or finding identity. | Reviewed baseline creation or shrink-only update PR. | Shrink-only refresh can remove resolved debt; new debt is never auto-adopted by CI. |
| Waiver records | PR-local acknowledgement that a visible finding is accepted for that review. | PR number plus finding or canonical gap identity. | Visible `ripr-waive` label and PR evidence ledger entry. | Repeated waiver is a signal for focused test, baseline, or suppression review; it is not a failure by itself. |

## Entry shape requirements

Every TOML allowlist entry that represents a durable exception should include:

- **`id`** — stable identifier referenced in PR descriptions and ADRs.
- **`path`** or **`selector`** — file or structural location of the exception.
- **`owner`** — team or area responsible for the exception.
- **`reason`** or **`explanation`** — why the exception exists.
- **`expires`**, **`target`**, or **`review_by`** — date the entry must be
  re-justified when the ledger schema supports stale-entry review.

The relevant `cargo xtask check-*` gate rejects entries that miss fields
required by that ledger's schema.

Legacy text ledgers such as workflow allowlists have narrower schemas today.
Their `reason` field is still the reviewed exception signal, and their
line/count caps bound the exception instead of authorizing growth.

## Source suppression governance

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

RIPR finding suppressions use `.ripr/suppressions.toml` instead of source
attributes. See `docs/CONFIGURATION.md` for the suppression metadata fields
and `docs/OUTPUT_SCHEMA.md#suppression-health-report` for the advisory health
report shape.

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
