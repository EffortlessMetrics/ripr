# Python Repair Routing Implementation Plan

Status: proposed

Owner: language-adapter / swarm

Created: 2026-05-29

Plan ID: RIPR-PLAN-0017

Linked proposal:

- [RIPR-PROP-0017: Python Repair Routing Lane](../../docs/proposals/RIPR-PROP-0017-python-repair-routing-lane.md)

Linked specs:

- [RIPR-SPEC-0026: Language Adapter Contract](../../docs/specs/RIPR-SPEC-0026-language-adapter-contract.md)
- [RIPR-SPEC-0028: Python Preview Static Facts](../../docs/specs/RIPR-SPEC-0028-python-preview-static-facts.md)
- [RIPR-SPEC-0057: RIPR Swarm Repair Loop](../../docs/specs/RIPR-SPEC-0057-ripr-swarm-repair-loop.md)
- [RIPR-SPEC-0058: RIPR Swarm External Agent Handoff](../../docs/specs/RIPR-SPEC-0058-ripr-swarm-external-agent-handoff.md)
- [RIPR-SPEC-0061: Lane 1 Canonical Actionability Contract](../../docs/specs/RIPR-SPEC-0061-lane1-canonical-actionability-contract.md)

Linked ADRs:

- None.

Active goal:

- Not active. The active execution manifest remains
  [`.ripr/goals/active.toml`](../../.ripr/goals/active.toml). This plan does
  not supersede the routed-runner proof goal unless a later activation PR
  explicitly selects it.

Support-tier impact:

- None for this plan. Python remains `preview` until a dedicated support-tier
  PR promotes a scoped claim.

Policy impact:

- Register this plan and its proposal in
  [`policy/doc-artifacts.toml`](../../policy/doc-artifacts.toml).

Required evidence for plan edits:

```bash
cargo xtask check-doc-artifacts
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-support-tiers
cargo xtask check-pr-shape
git diff --check
```

Non-goals:

- No analyzer behavior changes in the charter PR.
- No output-schema changes in the charter PR.
- No default CI blocking or gate behavior changes.
- No source edits or generated tests.
- No provider or model calls.
- No runtime mutation execution.
- No release, publish, signing, marketplace, or source-repo authority changes.

Claim boundary:

- This plan makes the Python repair-routing lane reviewable and restartable.
  It does not claim Python is usable alpha, gate eligible, Rust parity, or
  runtime-confirmed.

Rollback:

- Revert the proposal, this plan, index links, support-tier clarifications, and
  document artifact ledger entries. No runtime behavior changes are involved.

## Current state

Python preview evidence already exists under the language adapter preview
contract. The current claim is syntax-first and advisory: Python can provide
owner, test, assertion/oracle, probe, related-test, and static-limit facts when
enabled, but support remains `preview`.

The lane target is higher than parser support. Python should become the first
non-Rust proof that RIPR can turn changed behavior into a bounded repair task:

```text
changed behavior
-> missing evidence
-> focused test repair
-> verify command
-> receipt
```

## Milestones

| Milestone | Work items | User value |
| --- | --- | --- |
| A. Python is recognized | PR 1-5 | RIPR can run on a Python repo without pretending it is Rust. |
| B. Python has real evidence | PR 6-12 | RIPR can identify changed Python behavior and distinguish strong tests from weak tests. |
| C. Python produces repair cards | PR 13-15 | RIPR gives the next test to add. |
| D. Python works in daily workflows | PR 16-19 | CLI, PR, CI, and editor show the same guidance. |
| E. Python becomes application-useful | PR 20-23 | Common API, CLI, field, and parameterized-test shapes become useful. |
| F. Swarm turns it into leverage | PR 24-26 | RIPR creates safe parallel test-repair work and proves what closed. |
| G. Promotion | PR 27-30 | Python support is honest, measured, and ready to promote if evidence supports it. |

## Work items

### Work item: docs/python-repair-routing-charter

Status: done

Linked proposal:

- RIPR-PROP-0017

Linked specs:

- RIPR-SPEC-0026
- RIPR-SPEC-0028
- RIPR-SPEC-0057
- RIPR-SPEC-0058
- RIPR-SPEC-0061

Linked ADR:

- n/a

Blocks:

- `docs/python-current-state-inventory`

Blocked by:

- n/a

Branch:

- `docs-python-repair-routing-charter`

Issue:

- n/a

PR:

- #518

#### Goal

Define what success means for the Python repair-routing lane before behavior
implementation spreads.

#### Production delta

- Add `RIPR-PROP-0017` as the lane charter and support contract.
- Add this implementation plan.
- Register the proposal and plan in `policy/doc-artifacts.toml`.
- Link the charter from proposal, plan, documentation, support-tier, and
  language-preview surfaces.

#### Non-goals

- No Python analyzer behavior changes.
- No fixture or golden changes.
- No CLI, output-schema, LSP, generated-CI, swarm, receipt, or gate behavior
  changes.
- No active-goal manifest changes.
- No support-tier promotion.

#### Acceptance

- Every future Python repair-routing PR can point back to the charter.
- Docs say Python remains static/advisory preview until promoted.
- "Fully working Python" is defined as the repair loop, not parser existence.
- The plan preserves the PR-by-PR lane sequence without making it active.

#### Proof commands

```bash
cargo xtask check-doc-artifacts
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-support-tiers
cargo xtask check-pr-shape
git diff --check
```

#### Rollback

- Revert the proposal, plan, doc index links, support-tier clarification, and
  document artifact ledger entries.

### Work item: docs/python-current-state-inventory

Status: done

Inventory:

- [Python repair routing current-state inventory](current-state-inventory.md)

Branch:

- `docs-python-current-state-inventory`

PR:

- #521

Linked proposal:

- RIPR-PROP-0017

Linked specs:

- RIPR-SPEC-0026
- RIPR-SPEC-0028
- RIPR-SPEC-0057
- RIPR-SPEC-0058
- RIPR-SPEC-0061

Linked ADR:

- n/a

Blocks:

- `analysis/python-project-detection`

Blocked by:

- `docs/python-repair-routing-charter`

#### Goal

Inventory current Python preview code, fixtures, Rust/Cargo assumptions, output
surfaces, and the first fixture matrix before changing behavior.

#### Production delta

- Add a current-state inventory doc or plan section covering existing Python
  preview code, fixtures, CLI assumptions, and output surfaces.
- Define the first fixture matrix:
  `basic_function`, `predicate_boundary`, `changed_return_value`,
  `changed_exception`, `dict_field_change`, `pytest_exact_assert`,
  `pytest_smoke_assert`, `unittest_assert_equal`, `fastapi_route_optional`,
  `cli_output_optional`, and `dynamic_unsupported`.

#### Non-goals

- No behavior change.
- No support-tier promotion.
- No fixture implementation yet unless the inventory finds an existing fixture
  and only indexes it.

#### Acceptance

- Clear map of current state and remaining work.
- Every later PR has a fixture home.
- Rust/Cargo assumptions that block Python-only repos are listed with owners.

#### Proof commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr-shape
git diff --check
```

#### Rollback

- Revert the inventory doc and index links.

### Work item: analysis/python-project-detection

Status: done

Blocked by:

- `docs/python-current-state-inventory`

#### Goal

Let `ripr pilot --root <python-repo>` recognize Python repos without requiring
a Cargo workspace.

#### Acceptance

- Detect `pyproject.toml`, `setup.py`, `setup.cfg`, `requirements.txt`,
  `pytest.ini`, `tox.ini`, `noxfile.py`, `tests/`, and `src/`.
- Exclude `.venv/`, `venv/`, `.tox/`, `.nox/`, `site-packages/`,
  `.pytest_cache/`, `.mypy_cache/`, `dist/`, `build/`, and detectable
  generated files.
- Python-only and mixed repos fail closed with named limitations rather than
  Cargo-specific errors.
- `ripr pilot --root fixtures/python/basic` works without Cargo.

Delivered:

- Missing `ripr.toml` now keeps Rust-only defaults unless Python project
  markers are present.
- Python project detection recognizes root marker files and Python files under
  `src/` or `tests/`, while skipping virtualenv, cache, build, distribution,
  and generated Python files.
- Explicit `ripr.toml` remains authoritative, so `[languages] enabled =
  ["rust"]` still disables Python preview even in Python-shaped repos.
- `fixtures/python/basic` pins the no-config Python project path used by
  `ripr pilot` and diff-scoped `ripr check`.

### Work item: analysis/python-source-facts

Status: complete

Blocked by:

- `analysis/python-project-detection`

#### Goal

Extract stable Python source facts with file, span, owner, and language
metadata.

#### Acceptance

- Facts cover modules, classes, functions, methods, decorators, parameters,
  returns, raises, predicates, comparisons, boolean expressions, calls,
  assignments, attribute writes, dict/list/set literals, string literals, and
  print/log calls.
- Malformed Python produces a named limitation, not a crash.
- No repair recommendations yet.

Delivered:

- Python analysis now flows through an internal source-fact snapshot with
  stable file, span, owner, and `language = "python"` metadata.
- The snapshot records modules, classes, functions, methods, decorators,
  parameters, returns, raises, predicates, comparisons, boolean expressions,
  calls, assignments, attribute writes, dict/list/set literals, string
  literals, and print/log calls.
- Malformed Python records an `unsupported_syntax` source-fact limitation
  instead of silently returning empty facts.
- Existing owner/test extraction reuses the snapshot; no repair-card or
  recommendation surface changed in this slice.

### Work item: analysis/python-diff-owner-mapping

Status: complete

Blocked by:

- `analysis/python-source-facts`

#### Goal

Map changed Python lines to stable, language-qualified owners.

#### Acceptance

- Owner IDs cover functions, methods, classes, and module-level code.
- Mixed Rust/Python repos do not collide.
- Changed Python owner is visible in JSON output.
- Unrelated line movement avoids unnecessary ID churn where possible.

#### Delivered

- Python preview findings now populate stable, language-qualified
  `probe.owner` IDs using `python:<path>::<qualified_owner>`.
- Changed-line owner selection prefers the narrowest matching owner, so
  function/method changes do not collapse to class or module owners while
  class-body and module-level changes still receive durable owners.
- JSON and human output surface Python preview `probe.owner` values; the
  existing `owner_kind` vocabulary remains unchanged, with class owners
  represented by `probe.owner` only.
- Focused unit tests pin function, method, class, module, line-movement, JSON,
  and human rendering behavior; `python_owner_file_match` pins fixture output.

### Work item: analysis/python-pytest-oracles

Status: complete

Blocked by:

- `analysis/python-diff-owner-mapping`

#### Goal

Recognize common pytest tests and classify assertion strength.

#### Acceptance

- Detect `test_*.py`, `*_test.py`, `def test_*`, `class Test*`, plain
  `assert`, `pytest.raises`, `pytest.mark.parametrize`, fixture parameters,
  common `client` tests, `capsys`, `caplog`, and `monkeypatch`.
- Classify exact, boundary, exception, field, output, status-code, broad smoke,
  reach-only, and unknown helper oracles.
- Unknown helpers remain conservative.

#### Delivered

- Pytest test discovery now records fixture/parameter names and limits
  class-scoped pytest discovery to `class Test*` while preserving
  `unittest.TestCase` method discovery for the next slice.
- Python assertions now keep an internal pytest oracle shape for exact,
  boundary, exception, field, output, status-code, broad-smoke,
  reach-only, mock, and custom-helper evidence while preserving the shared
  `OracleKind` / `OracleStrength` output vocabulary.
- `pytest.raises` context managers, imported `raises(...)`, `caplog` /
  `capsys` output observers, `response.status_code` / `exit_code`,
  dict/attribute field assertions, parametrized tests, and
  `monkeypatch` fixture parameters are represented as preview evidence.
- `python_pytest_oracle_shapes` pins output/log assertion evidence; existing
  Python preview fixtures now record fixture parameters and non-exact oracle
  shapes in JSON evidence without emitting repair cards yet.

### Work item: analysis/python-unittest-oracles

Status: done

Blocked by:

- `analysis/python-pytest-oracles`

#### Goal

Support common `unittest` repos without a separate output model.

#### Acceptance

- Detect `unittest.TestCase`, `def test_*`, `assertEqual`, `assertTrue`,
  `assertFalse`, `assertRaises`, `assertIn`, `assertRegex`, and
  `assertDictEqual`.
- Verify commands can use pytest or `python -m unittest` when appropriate.
- Unittest facts enter the same oracle taxonomy as pytest.

#### Delivered

- Python test facts now preserve a class-qualified test name, so pytest class
  methods and `unittest.TestCase` methods can be addressed by stable static
  selectors.
- Preview evidence now records framework-shaped verify commands for related
  tests: `pytest path::node` for pytest and
  `python -m unittest module.Class.test_method` for unittest.
- Unittest assertion calls now use assertion arguments to preserve output,
  status-code, and dict/object field oracle shapes while keeping the shared
  `OracleKind` / `OracleStrength` vocabulary.
- `python_unittest_oracle_shapes` pins a unittest `self.assertIn(...)`
  output assertion and the generated `python -m unittest` verify command;
  existing Python preview fixtures record verify-command evidence without
  emitting repair cards yet.

### Work item: analysis/python-related-test-linking

Status: done

Blocked by:

- `analysis/python-unittest-oracles`

#### Goal

Connect changed owners to likely tests using conservative static signals.

#### Acceptance

- Use imports, direct calls, class references, obvious route/client references,
  filename similarity, test naming similarity, and fixture names.
- Distinguish related strong tests, related weak tests, and no related test.
- Weak related tests are preferred repair locations.
- Uncertain links are marked uncertain.

#### Delivered

- Related-test ranking now keeps direct syntactic calls and import-alias calls
  ahead of heuristic links so weak directly related tests remain preferred
  repair locations.
- Same-stem file proximity, test-name similarity, and fixture-name proximity
  are treated as heuristic-only links: they keep weak reachability, do not
  promote assertions to strong revealability, and emit
  `related_test_uncertain` evidence.
- `python_related_test_name_similarity` and
  `python_fixture_name_relation` pin the new uncertain relation outputs, while
  existing same-stem and module-level fixtures were refreshed to preserve the
  same uncertainty boundary.

### Work item: analysis/python-canonical-gap-identity

Status: done

Blocked by:

- `analysis/python-related-test-linking`

#### Goal

Create durable Python canonical gap IDs.

#### Acceptance

- Identity includes language, file, owner path, behavior kind, probe kind, and
  normalized expression, field, exception, or output.
- Duplicate raw signals collapse into one canonical finding.
- Line-number-only identity is avoided where possible.
- Same ID appears across CLI, JSON, SARIF, PR, LSP, and agent packet surfaces.

#### Delivered

- Python preview findings now carry an optional `canonical_gap_id` and typed
  `canonical_gap` identity made from language, file, owner path, behavior kind,
  probe kind, and normalized discriminator text.
- Canonical Python identities omit source line numbers, so line movement does
  not churn the ID when the changed owner and discriminator are stable.
- JSON output records `canonical_gap_id`, `canonical_gap_group_size`, and the
  structured identity parts; human, SARIF, GitHub annotation, LSP diagnostic,
  hover, and context-packet surfaces carry the same scalar ID.
- Static-limit Python findings keep `static_limit_kind` without a canonical
  repair-gap ID until typed non-actionable gap states land.
- Existing Python fixture goldens pin the identity on non-static-limit preview
  findings while static-limit fixtures stay unchanged.

### Work item: analysis/python-ripr-evidence-model

Status: done

Blocked by:

- `analysis/python-canonical-gap-identity`

#### Goal

Express Python evidence using RIPR reachability, infection, propagation, and
revealability concepts.

#### Acceptance

- Actionable gaps carry evidence for reachability, changed behavior,
  propagation, and revealability.
- Non-actionable cases carry stop reasons.
- Code changes alone do not produce recommendations.

#### Delivered

- Python preview findings now use family-specific RIPR infection and
  propagation evidence for predicates, return values, exception paths,
  field/object state, and call/output effects instead of placeholder
  `unknown` summaries.
- Static-limit Python findings fail closed as `static_unknown`, preserve the
  observed reach/oracle facts, carry typed stop reasons, and omit canonical
  repair-gap IDs and recommendations.
- Simple predicate-boundary findings can carry an activation-level missing
  discriminator such as `amount == threshold`; the value is visible in JSON,
  human evidence paths, and fixture goldens.
- Findings with no related Python test remain `no_static_path` evidence and do
  not emit repair recommendations until repair-card, verify-command, and
  receipt contracts exist.

### Work item: analysis/python-repair-classes-v1

Status: done

Blocked by:

- `analysis/python-ripr-evidence-model`

#### Goal

Ship the first high-confidence Python repair classes.

#### Acceptance

- Predicate boundary, return value, exception path, dict/object/dataclass
  field, and output/log behavior each have positive and negative fixtures.
- Every actionable gap includes a missing discriminator.
- Dynamic or ambiguous cases remain non-actionable.

#### Delivered

- Direct weak Python preview findings now emit family-specific missing
  discriminators for predicate boundaries, return values, exception paths,
  field/object values, and output/log/call effects.
- Weak direct findings get repair-class next-step wording that names the
  missing discriminator without claiming a full repair card, verify command, or
  receipt.
- Strong-oracle, no-path, heuristic-only, and static-limit cases suppress
  repair guidance instead of being treated as repair-ready work.
- Existing Python fixture goldens pin positive and negative examples across the
  first repair classes while preserving Python's preview/support-tier boundary.
- `python_dict_field_repair_gap` pins returned-dict field discriminator
  extraction without requiring runtime dataclass or serializer semantics.

### Work item: output/python-ranking-noise-control

Status: done

Blocked by:

- `analysis/python-repair-classes-v1`

#### Goal

Make ranking-facing output show a curated small set of Python findings. The
existing `ripr pilot` command still consumes Rust seam inventory; Python
first-use projection remains in `cli/python-first-use-path`.

#### Acceptance

- Rank higher for public owners, related weak tests, concrete
  discriminators, available verify commands, clear assertion shape, and core
  changed behavior classes.
- Rank lower or stop for dynamic imports, opaque helpers, monkeypatch-only
  behavior, generated code, metaprogramming, missing test locations, and
  unclear discriminators.
- "No actionable Python gaps" is an honest supported result.

#### Delivered

- The shared finding sorter now assigns Python preview findings an
  actionability rank without changing non-Python file/line ordering.
- Direct weak Python findings with canonical gaps, concrete missing
  discriminators, public owners, direct related-test evidence, verify-command
  evidence, and core repair families sort ahead of lower-value Python preview
  findings.
- Already-observed, no-path, heuristic-only, unknown, and static-limit Python
  findings remain visible but sort after repairable direct weak gaps.
- `python_ranking_noise_control` pins the report order: a direct repairable
  predicate-boundary gap appears before observed, no-related-test, and
  dynamic-dispatch static-limit findings even when those noisy files sort
  earlier by path.

### Work item: output/python-test-placement-verify

Status: done

Blocked by:

- `output/python-ranking-noise-control`

#### Goal

Recommend where and how to verify a Python repair.

#### Acceptance

- Actionable gaps include suggested test file, test name, node ID when
  possible, and pytest or unittest command.
- Command confidence is included.
- Commands do not assume dependencies that are not detected.

#### Delivered

- Direct weak Python findings with a concrete missing discriminator now emit
  placement metadata for the nearest direct pytest or unittest related test:
  suggested test file, suggested test name, pytest node ID when applicable,
  verify command, and verify-command confidence.
- Human output renders a compact `Repair placement` block before the next-step
  wording; JSON output carries the same data as an additive
  `repair_placement` object.
- The command builder only emits placement when a related test framework is
  detected statically, so heuristic-only, no-path, static-limit, and
  already-observed findings do not get invented commands.
- `python_test_placement_verify` pins pytest and unittest placement output,
  while existing direct weak Python goldens now show the same placement fields.

### Work item: output/python-repair-card-v1

Status: done

Blocked by:

- `output/python-test-placement-verify`

#### Goal

Produce copy-ready human Python repair cards.

#### Acceptance

- Cards include changed owner, changed behavior, current test evidence,
  missing discriminator, recommended test shape, suggested location, verify
  command, receipt status, and stop conditions.
- Cards are present in human CLI and JSON output.
- Cards do not edit files.
- Receipt commands remain deferred to `outcome/python-gap-ledger`; this v1 card
  exposes the receipt slot honestly as unavailable instead of inventing a
  before/after command before Python outcome records exist.

#### Delivered

- Direct weak Python findings that already have a canonical gap, concrete
  missing discriminator, related-test evidence, placement, and verify command
  now render a `Python repair card` in human output and an additive
  `python_repair_card` object in JSON.
- The v1 card names the changed owner and behavior, current weak test
  evidence, missing discriminator, recommended pytest/unittest-shaped test
  shape, suggested assertion, test location, verify command and confidence,
  preview/advisory authority boundary, deferred receipt status, stop
  conditions, and limits.
- Static-limit, heuristic-only, no-path, and already-observed Python findings
  still do not get repair cards.

### Work item: swarm/python-agent-packet-export

Status: done

Blocked by:

- `output/python-repair-card-v1`

#### Goal

Export deterministic, bounded Python repair packets for swarm use.

#### Acceptance

- Packet fields include canonical gap ID, language, allowed files, forbidden
  files, task, missing discriminator, test shape, verify command, receipt
  command, and stop conditions.
- Packets are suitable for parallel execution without overlapping edits where
  possible.

#### Delivered

- `ripr reports gap-ledger --check-output <check.json>` now derives PR-local
  Python `GapRecord` entries from actionable `python_repair_card` findings
  without rerunning analysis.
- The derived records preserve canonical Python gap IDs, preview language
  status, source anchors, suggested test files, suggested test names, verify
  commands, stop conditions, and preview/advisory authority boundaries.
- `ripr agent packet --gap-ledger <ledger> --gap-id <id> --json` can export the
  selected Python record through the existing agent packet envelope.
- Gap-ledger packets now carry explicit `allowed_files`, `forbidden_files`,
  `conflict_group`, `receipt_command`, and `receipt_status` fields so agents
  get bounded test-edit scope and same-file conflict grouping.
- Check-output-derived Python GapRecords now synthesize deterministic
  `ripr outcome` receipt commands from the supplied before check JSON,
  `target/ripr/reports/after-check.json`, and a gap-scoped receipt path, so
  the same records can render through `ripr agent packet --gap-ledger`.
- Python preview records remain advisory: gate and RIPR-zero projections stay
  ineligible until later policy and outcome-ledger work exists.

### Work item: cli/python-first-use-path

Status: in progress

Blocked by:

- `swarm/python-agent-packet-export`

#### Goal

Make first Python runs useful in CLI.

#### Acceptance

- `ripr pilot --root .`, `ripr first-pr --root . --base origin/main --head
  HEAD`, and `ripr check --root . --format json` show detected Python project,
  supported/unsupported features, top repairable gap, limitation count, repair
  card, verify command, and receipt command when evidence supports it.

#### Progress

- `ripr pilot --root .` now projects the existing diff-scoped Python preview
  repair card into the terminal summary and `pilot-summary.{json,md}` when a
  Python project/diff yields a repairable gap. The card keeps preview/advisory
  boundaries, limitation counts, verify command, and deferred receipt status.
- `ripr check --root . --format json` already emits the underlying
  `python_repair_card` object.
- `ripr first-pr --root . --base origin/main --head HEAD` can now accept
  Python-only project roots and select preview Python GapRecords from the
  existing gap decision ledger into `start-here.{json,md}` with
  `preview_limited` output state, missing discriminator, verify command, and
  receipt command.
- Python-only `first-pr` gap-ledger recovery now points at the existing bridge:
  `ripr check --json` followed by `ripr reports gap-ledger --check-output`,
  rather than the Rust repo-exposure path.
- Python before/after outcome receipts can now compare check-output JSON by
  canonical gap ID, and the check-output gap-ledger bridge now supplies the
  packet receipt command.
- A follow-up slice still needs, if useful, a direct `first-pr` producer flag
  for supplied check-output JSON and more contextual raw `ripr check` / `pilot`
  receipt guidance.

### Work item: output/python-surface-projection

Status: in progress

Blocked by:

- `cli/python-first-use-path`

#### Goal

Project Python repair cards consistently across output surfaces.

#### Acceptance

- JSON, Markdown, SARIF, PR comments, generated summaries, and
  output-contract tests share canonical IDs.
- Python findings are not Rust-shaped findings with Python labels.
- PR summary highlights top Python repair cards.

### Work item: ci/python-advisory-mode

Status: planned

Blocked by:

- `output/python-surface-projection`

#### Goal

Let teams run Python repair-routing in PRs safely.

#### Acceptance

- Advisory GitHub Actions support uploads report artifacts and normalized
  result checks.
- Fork/untrusted behavior is clear.
- No provider calls, mutation execution, default self-hosted runner use, or
  default CI blocking.

### Work item: lsp/python-repair-card-projection

Status: planned

Blocked by:

- `output/python-surface-projection`

#### Goal

Bring Python repair cards into editor surfaces.

#### Acceptance

- Diagnostics, hovers, and code actions match CLI reports.
- Code actions can copy repair card, pytest skeleton, agent packet, and open
  related test file.
- Stale state is obvious and no hidden code edits occur.

### Work item: analysis/python-http-api-pack-v1

Status: planned

Blocked by:

- `output/python-surface-projection`

#### Goal

Support simple FastAPI/Flask-shaped repair cards.

#### Acceptance

- Simple route decorators, returns, status codes, JSON fields, and obvious
  client tests can produce framework-shaped repair cards.
- Dynamic routing remains a named limitation.

### Work item: analysis/python-cli-output-pack-v1

Status: planned

Blocked by:

- `output/python-surface-projection`

#### Goal

Support Python CLI/output repair cards.

#### Acceptance

- Simple Click, Typer, argparse, `print`, stdout/stderr, and exit-code shapes
  can produce output assertion cards.
- Ambiguous command construction remains non-actionable.

### Work item: analysis/python-parametrized-boundaries

Status: planned

Blocked by:

- `analysis/python-repair-classes-v1`

#### Goal

Suggest native pytest parameterization for clear boundary predicates.

#### Acceptance

- Suggest parameterization only when candidate values are explainable.
- Simpler one-case test remains available.
- Expected values are not invented without uncertainty labeling.

### Work item: analysis/python-existing-test-strengthening

Status: planned

Blocked by:

- `analysis/python-repair-classes-v1`

#### Goal

Prefer strengthening weak related tests over adding redundant tests.

#### Acceptance

- Cards can distinguish "strengthen existing test" from "add new test".
- Agent packets can restrict edits to one existing test.
- Outcome receipt shows broad oracle becoming more exact.

### Work item: swarm/python-gap-work-queue

Status: planned

Blocked by:

- `swarm/python-agent-packet-export`

#### Goal

Make multiple Python repair cards shardable.

#### Acceptance

- Queue entries include canonical gap ID, priority, owner, allowed edit files,
  verify command, expected receipt, and conflict group.
- Same-file conflicts and stale entries are visible.

### Work item: swarm/python-agent-result-ingestion

Status: planned

Blocked by:

- `swarm/python-gap-work-queue`

#### Goal

Classify agent repair attempts without trusting them blindly.

#### Acceptance

- Ingested results classify closed, partially improved, verify failed, edited
  forbidden file, uncertain, and stale outcomes.
- Production-code edits are flagged.
- Verify result and before/after movement are attached.

### Work item: outcome/python-gap-ledger

Status: in progress

Blocked by:

- `swarm/python-agent-result-ingestion`

#### Goal

Make Python gap improvement durable.

#### Acceptance

- Receipts show closed, new, unchanged, weakened, and strengthened Python gaps.
- Canonical Python gaps can open and close across runs.
- PR summary can report scoped Python gap movement without claiming
  correctness beyond static evidence movement.

#### Progress

- `ripr outcome` can compare Python check-output JSON snapshots by canonical
  gap ID and report weak-to-strong evidence movement as closed.
- `ripr reports gap-ledger --check-output` now carries the corresponding
  receipt command into repairable Python GapRecords, which makes bounded
  packet delegation receipt-ready.

### Work item: fixtures/python-false-positive-corpus

Status: planned

Blocked by:

- `outcome/python-gap-ledger`

#### Goal

Prevent Python support from becoming noisy.

#### Acceptance

- Fixtures cover dynamic imports, monkeypatch-only behavior, generated files,
  metaclass/decorator magic, unresolved pytest fixtures, property-based tests
  with opaque discriminators, custom assertion helpers, async tests, broad
  smoke tests, reach-without-observe, and duplicate raw signals.
- Unsupported cases produce named limitations and do not enter the repair
  queue.

### Work item: dogfood/python-real-repo-evals

Status: planned

Blocked by:

- `fixtures/python-false-positive-corpus`

#### Goal

Prove usefulness outside fixtures.

#### Acceptance

- Dogfood runs cover a tiny controlled Python repo, normal pytest app repo,
  API repo, CLI/tooling repo, and mixed repo when relevant.
- Each run records command, runtime, top finding, repair card, verify command,
  usability, before/after receipt, false-positive notes, and limitation notes.
- At least one gap closes with receipt before promotion is considered.

### Work item: metrics/python-repair-routing-quality

Status: planned

Blocked by:

- `dogfood/python-real-repo-evals`

#### Goal

Measure Python quality by repair usefulness, not finding volume.

#### Acceptance

- Metrics include time to first useful finding, top-1/top-3 actionable
  precision, verify-command validity, concrete-discriminator rate,
  related-test-location rate, false-actionable rate, crash rate, unsupported
  limitation distribution, and receipt closure rate.
- Noisy changes fail quality gates.

### Work item: campaign/python-usable-alpha-promotion

Status: planned

Blocked by:

- `metrics/python-repair-routing-quality`

#### Goal

Promote Python only when the repair loop has receipt-backed evidence.

#### Acceptance

- Support docs, README claims, examples, capability matrix, traceability, and
  closeout evidence are updated by a dedicated promotion PR.
- Promotion target is at most `usable alpha` unless stronger evidence exists.
- Docs state that Python support provides static repair-routing for common
  pytest/unittest workflows and does not prove correctness, execute arbitrary
  code, or guarantee mutation adequacy.
- Source `ripr` remains the release/distribution authority.
