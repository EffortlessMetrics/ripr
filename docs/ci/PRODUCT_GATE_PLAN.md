# Canonical product-gate plan

Issue #3330 establishes the typed authority for ordinary product gates before
any workflow migration. The implementation lives in
`xtask/src/product_gate_plan.rs` and is intentionally pure: it describes gate
meaning and applicability without selecting a runner, invoking a provider, or
changing required-check routing.

## Current producer inventory

The current producer is the `Required Rust gates` step in
`.github/workflows/routed-rust.yml`. The same command set is repeated in the
CX43, CPX42, and CX53 routed jobs. Runner identity and matrix shape are route
details, not product meaning.

| Canonical gate | Current command | Role | Surface |
| --- | --- | --- | --- |
| `product.rust.formatting` | `cargo fmt --check` | required | Rust |
| `product.rust.workspace_check` | `cargo check --workspace --all-targets` | required | Rust |
| `product.rust.clippy` | `cargo clippy --workspace --all-targets -- -D warnings` | required | Rust |
| `product.rust.workspace_tests` | `cargo nextest run --workspace` | required | Rust |
| `product.repository.precommit` | `cargo xtask precommit` | required | repository policy |
| `product.evidence.promotion_honesty` | `cargo xtask check-evidence-promotion-honesty` | required | evidence |
| `product.repository.agent_skills` | `cargo xtask check-agent-skills` | required | repository policy |
| `product.repository.dependencies` | `cargo xtask check-dependencies` | required | repository policy |
| `product.repository.process_policy` | `cargo xtask check-process-policy` | required | repository policy |
| `product.repository.network_policy` | `cargo xtask check-network-policy` | required | repository policy |
| `product.evidence.goldens` | `cargo xtask goldens check` | required | evidence |
| `product.evidence.fixtures` | `cargo xtask fixtures` | required | evidence |

The following current workflow producers are deliberately not ordinary
product-gate rows: advisory reports, uploaded artifacts, PR summaries,
coverage telemetry, release readiness, package listing, publish dry-runs, and
scheduled or release-only qualification. They remain operational or
release-specific evidence until a separate contract promotes them.

## Selection boundary

`ProductGatePlan::for_subject` selects applicable rows when selector authority
is present. A missing selector authority or an external-tree trust class
selects the complete route with an explicit reason. It never returns an empty
green plan for an unknown subject. A selected gate claims only the proposition
listed in its definition; its non-claim is explicit and remains separate from
the command that happens to produce it.

This is observational groundwork. Existing workflows remain authoritative, and
future migration must compare their selected rows with this plan before any
required check is rerouted.
