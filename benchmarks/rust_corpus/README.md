# Rust repository corpus

`benchmarks/rust_corpus/manifest.json` pins the real Rust repositories that
ripr has to work well on. Scoreboards, verdict checks, CPU, memory and size
measurements, mutation spot-checks, and first-run tests all run against this
one corpus, so their results are comparable and are not drawn only from
serde, ripgrep, regex, and ripr itself.

The corpus is a deliberate mix of three profile classes:

- **good**: well-maintained, carefully reviewed code in current idioms. ripr
  should be precise here.
- **legacy**: long-lived or abandoned code with older idioms (2015 edition,
  `extern crate`, `try!`), large modules, or thin tests. ripr should stay
  honest and fail closed rather than guess.
- **common**: ordinary team or community code, the kind most users have.
  ripr should be useful here without tuning.

The class is a judgment recorded per repo with the style it represents, and
it is backed by the probe counts. It is not a measurement.

Each repository is pinned to one upstream commit (`sha`) and that commit's
first parent (`base_sha`). A subject therefore has a whole-repository state
and a real upstream diff, the change a reviewer actually saw:

```bash
cargo xtask rust-corpus fetch --allow-network --tier fast
cargo xtask rust-corpus list --tier fast    # id, tier, path, base_sha, sha
cargo xtask rust-corpus smoke --tier fast   # run ripr on every pinned diff
ripr check --root target/ripr/corpus/tokio --base <base_sha> --format json
```

`fetch` writes `target/ripr/corpus/<id>` and `target/ripr/corpus/index.json`
(paths, pins, roles, profile, stresses, probe counts). It fetches only the
two pinned commits per repository (`git fetch --depth 2 <sha>`), verifies
HEAD and its first parent against the manifest. It reuses an existing
checkout only when the pins match and no tracked file is modified, so a
lane that edited a checkout gets the pinned state back on the next fetch.
Untracked build output does not force a refetch. `fetch` replaces only an
empty directory or a checkout that an earlier `fetch` created; it marks those
with `.git/ripr-corpus-checkout`. Any other directory under `--root` makes the
fetch for that repository fail with the path, and nothing is deleted. That
covers your own clone of the same repository. A checkout fetched before the
marker existed is still reused while it matches the pins, but it has to be
deleted by hand before a re-fetch. `--repo <id>` selects
individual repositories, and a partial fetch keeps the index entries an earlier
fetch of the same `corpus_version` recorded for other repositories. Fetching is opt-in
network and never runs on the default CI path. `cargo xtask rust-corpus check`
is the offline manifest validator, and the committed manifest is also checked
by an xtask unit test.

`smoke` runs `ripr check --base <base_sha> --format json` on each fetched
repo and writes `target/ripr/reports/rust-corpus-smoke.{json,md}` with run
status, exit code, wall time, and finding counts per repo and profile class.
A missing or modified checkout, a timeout, unparseable output, a non-zero
exit, a missing summary, or a fail-closed run status makes the receipt
`inconclusive`, and each row that did not run records the reason and the
next step. `smoke` exits 0 whenever it writes a receipt, so gate on the
receipt's `status`, not the exit code. The receipt says nothing about
whether the verdicts are correct, and it does not measure memory.

## Tiers

| Tier | Selects | Repos | Checked out |
|------|---------|------:|------------:|
| fast | `tier = fast` | 20 | ~98 MB |
| full | every repo | 31 | ~403 MB |
| targets | `roles` contains `integration_target` | 6 | n/a |

Use `fast` for pull-request and quick local runs, and `full` for nightly and
release measurement. `full` adds the large workspaces and monorepos. Each
profile class is required in both `fast` and `full`. The `targets` tier
selects the repositories ripr intends to integrate with or be used on (no
upstream contact implied). Some are also representative repos, so they are
pinned only once.

## Repositories

| Repo | Tier | Profile | Kind | Rust lines | Role | Stresses |
|------|------|---------|------|-----------:|------|----------|
| serde | fast | good | library | 43k |  | proc_macro, compile_fail_ui_tests, build_rs, no_std |
| regex | fast | good | library | 161k |  | generated_code, large_library, no_std, workspace |
| ripgrep | fast | good | binary | 56k | target | binary_crate, custom_test_macro, workspace |
| anyhow | fast | good | library | 6k |  | build_rs, compile_fail_ui_tests, no_std, small_crate |
| thiserror | fast | good | library | 5k |  | proc_macro, compile_fail_ui_tests, small_crate |
| heapless | fast | good | library | 21k |  | no_std, embedded |
| rstest | fast | common | library | 21k |  | proc_macro, test_macro_rstest, workspace |
| proptest | fast | legacy | library | 38k |  | test_macro_proptest, proc_macro, custom_test_harness, workspace |
| insta | fast | good | library | 26k |  | snapshot_tests, workspace |
| prost | fast | common | library | 19k |  | generated_code, build_rs, proc_macro, custom_test_harness, workspace |
| axum | fast | good | library | 47k |  | async_tokio, workspace, proc_macro |
| rusqlite | fast | common | library | 51k |  | ffi, build_rs, bundled_c |
| tokio | full | good | library | 185k | target | async_tokio, workspace, custom_test_harness, large_library, loom_model_tests |
| clap | full | good | library | 85k | target | proc_macro, snapshot_tests, custom_test_harness, workspace |
| cargo | full | legacy | binary | 345k |  | binary_crate, custom_test_macro, snapshot_tests, large_workspace, large_monorepo |
| rust-analyzer | full | good | binary | 587k |  | binary_crate, large_monorepo, snapshot_tests, generated_code, large_workspace |
| wasm-bindgen | full | legacy | library | 371k |  | wasm_target, proc_macro, ffi, large_workspace, generated_code |
| embassy | full | common | library | 591k |  | no_std, embedded, async_no_std, build_rs, large_monorepo, ffi |
| bevy | full | common | library | 664k |  | large_monorepo, proc_macro, large_workspace, custom_test_harness |
| rust-openssl | fast | legacy | library | 49k |  | ffi, build_rs, proc_macro, workspace |
| atuin | fast | common | binary | 136k |  | async_tokio, database_layer, test_macro_rstest, test_macro_proptest, workspace, binary_crate |
| image | full | legacy | library | 44k |  | custom_test_harness, large_library |
| lemmy | full | common | binary | 74k |  | async_tokio, database_layer, large_workspace, binary_crate |
| zola | full | common | binary | 25k |  | binary_crate, snapshot_tests, workspace |
| nushell | full | common | binary | 455k |  | binary_crate, large_monorepo, large_workspace, test_macro_rstest, custom_test_harness |
| iron | fast | legacy | library | 4k |  | edition_2015, sparse_tests, workspace |
| rustc-serialize | fast | legacy | library | 7k |  | edition_2015, sparse_tests, small_crate |
| spotify-tui | fast | legacy | binary | 12k |  | binary_crate, sparse_tests |
| cargo-mutants | fast | good | binary | 20k | target | binary_crate, snapshot_tests |
| fd | fast | good | binary | 9k | target | binary_crate, small_crate |
| bat | fast | good | binary | 18k | target | binary_crate, build_rs |

The manifest's `why` field explains each entry, and `profile.style` names the
style it represents. Its `probe` block records lexical counts at the pin:
Rust files and lines, Cargo manifests, build.rs scripts, proc-macro crates,
no_std files, `harness = false` targets, `extern crate` files, files over
3,000 lines, and checkout size. Probe counts describe the subject. They are
not ripr results.

`check` requires every declared stress category to be covered by at least
one repository. A category that is not `full_only` must also be covered by a
fast-tier repository. The `full_only` categories are large_monorepo,
large_workspace, wasm_target, async_no_std, and loom_model_tests.

## Known gaps

`known_gaps` in the manifest lists what the corpus does not yet represent: a
nextest-only test configuration, a cfg(windows)-dominant crate, and tests
that live in another repository. Python subjects stay in
`fixtures/python-eval-sweep/manifest.json`.

## Changing the corpus

Pins are exact. Refreshing a pin, adding a repository, or changing a tier,
role, or profile is a new `corpus_version`, so results recorded against an
earlier version stay attributable. Apply the manifest's `selection_rule`
when choosing a pin: the most recent first-parent commit whose diff changes
1 to 12 non-test `.rs` files and 6 to 400 Rust lines, preferring behavior
fixes over chores. `check` enforces the line bound exactly. The recorded file
count includes test files, so `check` enforces only a looser 1 to 40 file
bound; the non-test file limit is a selection judgment. Archived repositories pin their last qualifying change.
Run `cargo xtask rust-corpus check`, then `fetch` the changed entries. The
fetch fails if the commit or its first parent does not match the pins.
