# Routed Rust command disposition (ripr#1446)

The source repository's routed Rust workflow moved from a self-hosted router to a
single GitHub-hosted route. That deleted roughly four hundred lines of workflow.
The complete pre-change Cargo/xtask command surface and every removed
host-specific command or interface receive an explicit disposition here. YAML
plumbing that remains (checkout, toolchain setup, artifact upload) is enforced by
the workflow/action contracts rather than counted as a proof command. The
distinction that matters is:

```text
proof disappeared              defect
host maintenance disappeared   correct
```

`routed_rust_command_disposition_is_complete` in `xtask/src/tests.rs` owns the
exact pre-change command inventory: every retained entry must occur in this
table and the current workflow, while every removed host entry must occur in the
table and be absent from the current workflow.

## Dispositions

| Command or step | Disposition | Basis |
| --- | --- | --- |
| `cargo fmt` | `retained_required` | Runs on the GitHub-hosted lane. |
| `cargo check` | `retained_required` | Runs on the GitHub-hosted lane. |
| `cargo clippy` | `retained_required` | Runs on the GitHub-hosted lane. |
| `cargo nextest` | `retained_required` | Runs on the GitHub-hosted lane. |
| `cargo xtask precommit` | `retained_required` | Shared gate table, one required lane plus docs-gate. |
| `cargo xtask check-evidence-promotion-honesty` | `retained_required` | Lane-only gate. |
| `cargo xtask check-agent-skills` | `retained_required` | Lane-only gate and docs-gate command. |
| `cargo xtask check-dependencies` | `retained_required` | Lane-only gate. |
| `cargo xtask check-process-policy` | `retained_required` | Lane-only gate. |
| `cargo xtask check-network-policy` | `retained_required` | Lane-only gate. |
| `cargo xtask goldens check` | `retained_required` | Lane-only gate. |
| `cargo xtask fixtures` | `retained_required` | Lane-only gate. |
| `cargo xtask test-efficiency-report` | `retained_advisory` | Advisory report; failures remain non-gating. |
| `cargo xtask badge-artifacts` | `retained_advisory` | Advisory report; failures remain non-gating. |
| `cargo xtask pr-summary` | `retained_advisory` | Advisory report; failures remain non-gating. |
| `cargo xtask receipts` | `retained_advisory` | Advisory report; failures remain non-gating. |
| `cargo xtask reports index` | `retained_advisory` | Advisory report; failures remain non-gating. |
| `cargo xtask ripr-pr` and `--check` | `retained_required` | Pull-request evidence producer and verifier. |
| `cargo xtask ripr-review-comments` and `--check` | `retained_required` | Review-comment evidence producer and verifier. |
| `cargo xtask impacted-evidence` and `--check` | `retained_required` | Impact evidence producer and verifier. |
| `cargo xtask ripr-pr-summary` and `--check` | `retained_required` | PR summary producer and verifier. |
| `cargo xtask ripr-annotations` and `--check` | `retained_required` | Annotation producer and verifier. |
| `cargo xtask proof route` | `retained_required` | Advisory dry-run artifact, `\|\| true`. |
| `cargo clean` | `not_applicable_ephemeral_host` | Appeared only inside `Clean scratch` steps that wipe `/mnt/ci-scratch` on persistent runners. A GitHub-hosted runner is discarded after the job, so there is nothing to reclaim. |
| `sccache` | `not_applicable_ephemeral_host` | Shared compile cache for persistent hosts; no cross-run cache exists on ephemeral runners. |
| `ci-disk-guard` | `not_applicable_ephemeral_host` | Free-space floor for shared scratch mounts that the hosted route does not use. |
| `gh api orgs/.../actions/runners` | `removed_wrong_architecture` | Organization runner discovery. Self-hosted capacity is `ripr-swarm` authority; the source repository must not query it. |
| `Select runner` | `removed_wrong_architecture` | Router step that chose between self-hosted pools. |
| `Prepare toolchain temp` | `not_applicable_ephemeral_host` | Created a scratch `TMPDIR` on shared hosts. |
| `Prepare scratch` / `Prepare CPX42 scratch` | `not_applicable_ephemeral_host` | Age-gated cleanup of shared scratch before use. |
| `Clean scratch` / `Clean CPX42 scratch` | `not_applicable_ephemeral_host` | Post-run scratch reclamation on shared hosts. |
| `Install sccache` / `Start sccache server` | `not_applicable_ephemeral_host` | Compile-cache daemon for persistent hosts. |

## Enforced conclusion

```text
proof commands removed          0
```

No proof command was traded for resource convenience. The GitHub-hosted lane
already ran the full product/gate denominator before this change; what it lacked
were the persistent-host maintenance steps that have no meaning on an ephemeral
runner.

`cargo test` and `cargo build` are intentionally absent from this disposition:
the immediately preceding workflow did not invoke either command. Its test
authority was `cargo nextest run --workspace`, and compilation was covered by
the retained `cargo check`, Clippy, nextest, and xtask invocations. Listing an
unobserved command as retained would make the table stronger than the workflow
history it describes.

## Related surfaces not changed here

Self-hosted capacity is still requested by the advisory bot lanes
(`droid.yml`, `droid-review.yml`, `droid-security-scan.yml`, group
`em-ci-review`) and by `scratch-gc.yml`, which garbage-collects the scratch
mounts those runners share. None of them gate the required source pull-request
proof, and this issue owns the routed Rust route, so they are recorded rather
than changed.
