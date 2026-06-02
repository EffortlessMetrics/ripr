# Closeout: Python Repair Routing Usable Alpha

Date: 2026-05-31

Owner: Lane 1 evidence-to-repair

Linked proposal: [RIPR-PROP-0017](../proposals/RIPR-PROP-0017-python-repair-routing-lane.md)

Linked spec: [RIPR-SPEC-0028](../specs/RIPR-SPEC-0028-python-preview-static-facts.md)

Linked plan item: `campaign/python-usable-alpha-promotion`

## Decision

The scoped Python repair-routing loop is promoted to `usable alpha`.

This is not a broad Python parity claim. The promoted surface is the advisory
loop that turns selected direct weak Python findings into bounded test-repair
work with a canonical gap, missing discriminator, suggested test target, verify
command, stop conditions, agent packet, and before/after receipt.

Broader Python static facts, static limits, generated-CI grouping, badges,
baselines, gates, and RIPR Zero remain preview/advisory unless a later policy
change explicitly promotes those roles.

## What Landed

| Surface | Evidence |
| --- | --- |
| Repair cards | Direct weak Python findings emit `python_repair_card` output with changed owner, changed behavior, current test evidence, missing discriminator, test shape, suggested assertion/location, verify command, receipt status/command, stop conditions, and preview/advisory limits. |
| Public projections | Human output, JSON, SARIF, GitHub summary, `ripr pilot`, `ripr first-pr`, and LSP actions share the same canonical gap and repair-card fields for packetable Python findings. |
| Agent packets and queue | GapRecord-derived Python packets carry allowed test files, forbidden production files, conflict groups, verify commands, receipt commands/status, and stop conditions; static limits and no-action states are excluded from assignment. |
| Attempt ingestion | `ripr swarm ingest --result` classifies Python attempts as closed, partially improved, verify failed, forbidden edit, stopped, stale, or uncertain without trusting success blindly. |
| Outcome receipts | Python before/after check-output fixtures pin closed, unchanged, opened, strengthened, and weakened canonical-gap movement for boundary gaps, plus closed return, exception, field/object, and output/log gaps. |
| Dogfood receipts | `fixtures/real-repair-attempts` and `fixtures/python-real-repo-evals` record test-only Python repair attempts, bounded agent packets, focused pytest/unittest verify passes, and closed outcome receipts for controlled, normal pytest, async return-value, parametrized-boundary, CLI/output, Click CLI output, Typer CLI output, CLI exit-code, pytest exception, custom exception, and unittest exception paths, API status-code, API JSON detail, Flask route JSON detail, FastAPI route JSON detail, API exception-to-response, mixed, decorated-route, unittest return-value, and dataclass/model-field cases. The eval corpus also records dynamic-dispatch, decorator-indirection, missing-import-graph, metaprogramming, mocked-module, opaque-custom-helper, property-based, unresolved-fixture, and unsupported-syntax static-limit no-packet cases so unsupported shapes stay visible without becoming repair work. |
| Route-quality metrics | `cargo xtask dogfood` measures top-1 usefulness, ranked top-3 precision, verify-command validity, agent-packet boundary validity, discriminator coverage, suggested-location coverage, false-actionable/crash rates, receipt closure, unsupported limitation distribution, and no-action static-limit distribution. |
| Noise control | Dynamic import, decorator indirection, monkeypatch/module mocks, unresolved pytest fixtures, property-based tests, opaque custom helpers, metaclass declarations, unsupported syntax, generated files, and same-line duplicate signals are limited, excluded, or collapsed instead of becoming noisy repair work. |

## Proof Commands

The final metrics slice before promotion passed:

```bash
cargo test -p xtask dogfood_python_real_repo_eval_receipts_are_checked -- --test-threads=1
cargo xtask dogfood
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-pr
git diff --check
```

Closeout validation for this support-tier PR is:

```bash
cargo xtask check-support-tiers
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask dogfood
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Claim and Support-Tier Changes

The support-tier map now has two Python rows:

- `Python repair routing` -> `usable alpha`
- `Python preview static facts` -> `preview`

The `usable alpha` row is limited to selected static pytest/unittest repair
routes with direct weak evidence. It does not authorize production-code edits,
generated tests, provider calls, arbitrary imports, default test execution,
runtime mutation execution, gate eligibility, baseline debt, RIPR Zero
inclusion, public badge authority, correctness claims, or mutation adequacy.

## Policy Ledger Updates

No generated CI default, branch protection, badge basis, baseline, RIPR Zero,
waiver, suppression, release, publish, signing, marketplace, or source-repo
authority changes are made by this closeout.

Source `ripr` remains the release and distribution authority. `ripr-swarm`
records the development proof and support-tier boundary.

## Remaining Work

- Post-closeout application slices have now made HTTP/API, CLI/output,
  async return-value,
  Click CLI output,
  Typer CLI output,
  Flask route JSON detail,
  FastAPI route JSON detail,
  parameterized-boundary, existing-test-strengthening, and simple model-field
  repair cards fixture-backed while preserving fail-closed dynamic/framework
  limits.
- `dogfood/python-stability-evals-v1` is the next checkpoint: extend
  post-usable-alpha evidence with more real or external-repo-style Python evals,
  including top findings, packets, verify commands, receipts or no-receipt
  reasons, false-positive notes, limitation notes, and checked no-action
  static-limit cases.
- Stable Python support still requires longer dogfood, lower false-actionable
  rates across external repos, rollback proof, policy signoff, and an explicit
  later support-tier update.

## What Not To Do

- Do not describe Python as stable or Rust parity.
- Do not make Python preview facts default gate, baseline, RIPR Zero, or public
  badge inputs.
- Do not let Python agent packets edit production source.
- Do not treat verify success alone as closure without before/after RIPR
  movement.
- Do not add arbitrary imports, dependency installation, generated tests,
  provider calls, or mutation execution as promotion cleanup.
