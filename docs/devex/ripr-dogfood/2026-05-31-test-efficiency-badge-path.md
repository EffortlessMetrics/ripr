# Test-Efficiency Badge Path Closeout

Date: 2026-05-31

Issues: #590, #592

## Summary

The local `ripr-swarm` badge path has a deterministic producer for the
`ripr+` auxiliary input:

```bash
cargo xtask test-efficiency-report
```

It writes `target/ripr/reports/test-efficiency.json` and
`target/ripr/reports/test-efficiency.md`. The repo badge endpoint pipeline also
runs that producer before requesting repo `ripr+` badge formats: both
`cargo xtask badges` and `cargo xtask badges --check` enter
`write_repo_badge_artifacts()`, which calls `test_efficiency_report_impl()`
before rendering `repo-badge-plus-json` or `repo-badge-plus-shields`.

That means missing `test-efficiency.json` is no longer the local blocker for
ordinary `ripr+` badge generation. A missing report now has two supported
behaviors:

- the local xtask badge path regenerates it before badge rendering;
- direct `ripr check --format *badge-plus*` calls render a neutral
  `needs test-efficiency` badge instead of hard-failing.

## Verification Snapshot

```bash
cargo xtask test-efficiency-report
```

Result: passed and wrote `target/ripr/reports/test-efficiency.json`.

```bash
cargo run -p ripr --quiet -- check --root . --format repo-badge-plus-shields --gap-ledger fixtures/first_successful_pr/boundary-gap/inputs/reports/gap-decision-ledger.json
```

Result: passed and rendered a measured `ripr+` Shields badge from an explicit
gap ledger.

```bash
cargo xtask badges --check
```

Result: failed after the default 90 second `repo-badge-json` timeout. This is
the large-repo scan/cache blocker tracked by #588/#593, not a missing
test-efficiency input failure.

```bash
cargo xtask badges --check --gap-ledger fixtures/first_successful_pr/boundary-gap/inputs/reports/gap-decision-ledger.json
```

Result: generated repo badge artifacts from the fixture ledger, then failed
because committed endpoint JSON intentionally still reports the current public
snapshot. No badge endpoint JSON was hand-edited.

## Remaining Boundary

The next blocker is scan scaling, not `ripr+` auxiliary input production. Default
no-ledger badge checks still need #588/#593 so the large repo can refresh public
badge endpoints without repeatedly timing out on full repo scans.
