# CI Inventory

Snapshot of the CI surface for `ripr` at the time `policy/ci-lane-whitelist.toml`
was first introduced. Use this document to cross-check that every step of
every workflow is accounted for in the whitelist.

## Workflows in `.github/workflows/`

| File                              | Purpose                                                      |
| --------------------------------- | ------------------------------------------------------------ |
| `ci.yml`                          | Frontdoor PR gate (rust + policy + reports), MSRV, VS Code   |
| `coverage.yml`                    | Coverage lane (currently scheduled / label-driven)           |
| `droid.yml`                       | Droid AI assist                                              |
| `droid-review.yml`                | Droid review on PR                                           |
| `droid-security-scan.yml`         | Droid security scan                                          |
| `publish-extension.yml`           | VS Code extension publish (Marketplace + OpenVSX)            |
| `release-server-binaries.yml`     | LSP server binary release artifacts                          |
| `security.yml`                    | Cargo audit / supply chain                                   |
| `test-analytics.yml`              | Test telemetry upload                                        |

## Frontdoor inventory (cross-reference to whitelist)

The `rust` job in `ci.yml` runs the following lanes (each represented as a
separate entry in `policy/ci-lane-whitelist.toml` so the lane lint can
account for them):

```text
rust_fast_gate
check_static_language
check_no_panic_family
check_allow_attributes
check_local_context
check_file_policy
check_executable_files
check_workflows
check_droid_review_config
check_spec_format
check_fixture_contracts
check_generated
check_dependencies
check_process_policy
check_network_policy
package_list
publish_dry_run
test_efficiency_report
badge_artifacts
pr_summary
report_index
receipts
```

Plus the dedicated jobs:

```text
msrv             (job: msrv)
vscode_extension (job: vscode)
```

## Future lanes (added in later rollout PRs)

| Lane id              | Added in | Default                              |
| -------------------- | -------- | ------------------------------------ |
| `pr_plan`            | PR 07    | runs on every PR (advisory)          |
| `ripr_self_dogfood`  | PR 10    | label-gated, runs on Rust diffs      |
| `ci_actuals_upload`  | PR 11    | runs alongside other lanes           |
| `soft_budget_guard`  | PR 12    | runs after PR Plan                   |
| `future_clippy`      | PR 13    | label / `main` / dispatch only       |
| `mutation_calibration` | future | label-gated; never default           |

## Risk pack coverage

See `policy/ci-risk-packs.toml`. Every production source path should map to
at least one risk pack. Paths that don't match any pack default to the
`docs_only` pack.
