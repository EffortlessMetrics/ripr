# CI Lane Whitelist

`policy/ci-lane-whitelist.toml` is the map of every CI lane this repo is
allowed to spend LEM on. Each lane records its job key, runner class,
intent, failure mode, proof obligation, evidence, and review/expiry dates.

## Why a whitelist

The risk this guards against is **silent CI growth**: workflow steps
accumulating without a recorded reason. A whitelist forces the question
"why is this lane here" to be answered once and reviewed periodically,
instead of relitigated implicitly in every cost discussion.

It also supports the LEM forecast: every lane carries a `base_lem` so the
PR Plan job (PR 07) can sum a forecast from the union of selected lanes.

## Schema

The file has top-level metadata followed by an array of `[[lane]]` blocks.

```toml
schema_version  = "1.0"
policy          = "ci-lane-whitelist"
owner           = "release/ci"
status          = "active"

[[lane]]
id              = "stable identifier"
workflow        = ".github/workflows/<name>.yml"
job             = "job-key"
kind            = "test | policy | build | release | report | self-dogfood"
tier            = "frontdoor | deep | release | dogfood | docs"
default_pr      = true | false
blocking        = true | false
runner          = "ubuntu_latest | windows_latest | macos_latest | node_extension | external_ai_review"
base_lem        = <integer>
owner           = "team/area"
intent          = "one-line"
failure_mode    = "bug class blocked"
proof_obligation = "the command/check this lane evaluates"
evidence        = ["artifacts/logs"]
allowed_triggers = ["pull_request", "push", "workflow_dispatch", ...]
duplicate_of    = []
review_after    = "YYYY-MM-DD"
expires         = "YYYY-MM-DD"
```

## Lint rules (PR 03)

The CI lane whitelist lint (xtask command, deferred to a follow-up PR) will
warn first, then fail on:

- Workflow job has no lane entry.
- `default_pr = true` and an expensive lane (Windows/macOS/external) without
  an exception.
- Exception expired.
- Lane lacks `owner`, `intent`, `failure_mode`, or `proof_obligation`.
- `blocking = true` lane lacks `evidence`.
- `duplicate_of` target does not exist.
- Windows / macOS / Node / external lane lacks the matching multiplier.

## Risk packs

`policy/ci-risk-packs.toml` maps changed paths to the lanes that should run.
When a PR touches paths in a pack, PR Plan selects the pack's `lanes` plus,
when applicable, its `deep_lanes`.

## Exceptions

`policy/ci-whitelist-exceptions.toml` records intentional exceptions with
owner, reason, and expiry. The empty file is the desired state.

## Reference

- `docs/ci/cost-and-verification-policy.md`
- `docs/ci/lem-budgeting.md`
- `docs/ci/inventory.md`
