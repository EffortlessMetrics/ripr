# Routed Rust command disposition (ripr#1446)

The source repository's routed Rust workflow moved from a self-hosted router to a
single GitHub-hosted route. That deleted roughly four hundred lines of workflow,
so every command and step the old workflow ran receives one explicit disposition
here. The distinction that matters is:

```text
proof disappeared              defect
host maintenance disappeared   correct
```

`routed_rust_command_disposition_is_complete` in `xtask/src/tests.rs` enforces
this table against both workflow versions, so it cannot drift from the workflow
it describes.

## Dispositions

| Command or step | Disposition | Basis |
| --- | --- | --- |
| `cargo fmt` | `retained_required` | Runs on the GitHub-hosted lane. |
| `cargo check` | `retained_required` | Runs on the GitHub-hosted lane. |
| `cargo clippy` | `retained_required` | Runs on the GitHub-hosted lane. |
| `cargo nextest` | `retained_required` | Runs on the GitHub-hosted lane. |
| `cargo test` | `retained_required` | Runs on the GitHub-hosted lane. |
| `cargo build` | `retained_required` | Runs on the GitHub-hosted lane. |
| `cargo xtask precommit` | `retained_required` | Shared gate table, one required lane plus docs-gate. |
| `cargo xtask check-*` (all) | `retained_required` | Lane-only gates remain enumerated. |
| `cargo xtask goldens check` | `retained_required` | Lane-only gate. |
| `cargo xtask fixtures` | `retained_required` | Lane-only gate. |
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

## Counts

```text
old workflow command surface   27
new workflow command surface   23
removed                         4

removed as host maintenance     3   cargo clean, sccache, ci-disk-guard
removed as wrong architecture   1   organization runner discovery
proof commands removed          0
```

No proof command was traded for resource convenience. The GitHub-hosted lane
already ran the full product/gate denominator before this change; what it lacked
were the persistent-host maintenance steps that have no meaning on an ephemeral
runner.

## Related surfaces not changed here

Self-hosted capacity is still requested by the advisory bot lanes
(`droid.yml`, `droid-review.yml`, `droid-security-scan.yml`, group
`em-ci-review`) and by `scratch-gc.yml`, which garbage-collects the scratch
mounts those runners share. None of them gate the required source pull-request
proof, and this issue owns the routed Rust route, so they are recorded rather
than changed.
