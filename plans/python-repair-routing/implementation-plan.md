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
- `python_model_field_repair_gap` pins syntax-only returned constructor
  keyword field routing for model-like objects, recommends an object-field
  assertion such as `assert result.active == True`, and adds a stop condition
  for cases where the returned object does not expose the keyword as a public
  field or attribute.

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
- Field/object repair cards now specialize common assertion shapes for returned
  mapping fields, response JSON fields, and response status-code fields so the
  suggested assertion is directly copyable while staying static/advisory.
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
- `ripr first-pr --check-output <check.json>` now accepts saved Python check
  JSON directly, materializes the check-output-derived
  `gap-decision-ledger.{json,md}`, and then selects the preview Python
  start-here repair through the normal GapRecord path.
- Raw `ripr check` and `pilot` repair cards now include receipt guidance that
  tells users to save check JSON and run `ripr first-pr --check-output` or
  `ripr reports gap-ledger --check-output` to materialize a gap ledger with a
  concrete receipt command.

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

#### Progress

- Eligible `python_repair_card` findings now project into diff-scoped SARIF
  properties with the same advisory card fields as check JSON.
- GitHub annotation output now includes a concise Python repair-card sentence
  with the missing discriminator, suggested test target, verify command, and
  preview/advisory boundary.
- `cargo xtask pr-summary` now highlights the top Python preview repair card
  from `actionable-gaps.json` with the canonical gap, changed owner, missing
  discriminator, suggested test target, verify command, receipt command, and
  stop conditions while preserving the static/advisory boundary.

### Work item: ci/python-advisory-mode

Status: done

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

Status: in progress

Blocked by:

- `output/python-surface-projection`

#### Goal

Bring Python repair cards into editor surfaces.

#### Acceptance

- Diagnostics, hovers, and code actions match CLI reports.
- Code actions can copy repair card, pytest skeleton, agent packet, and open
  related test file.
- Stale state is obvious and no hidden code edits occur.

#### Progress

- LSP GapRecord command validation now accepts bounded Python verify commands
  for `pytest ...` and `python -m unittest ...` while preserving shell
  metacharacter and parent-directory rejection.
- Python preview GapRecord diagnostics with a safe pytest verify command now
  expose a `Write Python test: copy pytest skeleton` code action. The copied
  skeleton includes the canonical gap, suggested file, missing discriminator,
  changed behavior, verify command, stop conditions, and a fail-fast
  `NotImplementedError` placeholder rather than silently generating a passing
  test.
- Python preview GapRecord diagnostics with a safe verify command now expose a
  `Copy Python repair card` code action. The copied card is marked as current
  validated GapRecord evidence and includes the changed owner, changed
  behavior, current weak test evidence, missing discriminator, suggested
  assertion/location, verify command, receipt command when available, stop
  conditions, and preview/advisory limits.
- Current actionable and repairable Python GapRecord diagnostics now expose an
  `Agent handoff: copy Python packet` code action. The action reuses the
  existing GapRecord-backed `ripr.collectContext` packet path and fails closed
  without safe gap-ledger paths, repair-route paths, verify commands, and
  receipt commands.
- GapRecord code actions now fall back from a bare Python test name to
  `repair_route.target_file` when opening the related test file, matching the
  check-output-derived Python repair-card shape.

### Work item: analysis/python-http-api-pack-v1

Status: done

Blocked by:

- `output/python-surface-projection`

#### Goal

Support simple FastAPI/Flask-shaped repair cards.

#### Acceptance

- Simple route decorators, returns, status codes, JSON fields, and obvious
  client tests can produce framework-shaped repair cards.
- Dynamic routing remains a named limitation.

#### Progress

- `fixtures/python_api_route_decorator_repair_gap` now proves a simple
  FastAPI/Flask-shaped `@api.post(...)` route decorator can remain
  syntax-first route metadata instead of a decorator-indirection limit when a
  changed `response.status_code` assignment has weak pytest evidence.
- `fixtures/python_api_json_field_repair_gap` now proves a literal client route
  call such as `client.post("/checkout")` can link to the route owner and
  produce a framework-shaped `response.json()["detail"]` repair card.
- `fixtures/python_dynamic_route_registration_limit` now proves dynamic route
  registration fails closed as a named `dynamic_route_registration` limitation
  without producing a repair card or agent-packet-eligible canonical gap.
- Arbitrary decorators remain fail-closed through
  `python_decorator_indirection_limit`.

### Work item: analysis/python-cli-output-pack-v1

Status: done

Blocked by:

- `output/python-surface-projection`

#### Goal

Support Python CLI/output repair cards.

#### Acceptance

- Simple Click, Typer, argparse, `print`, stdout/stderr, and exit-code shapes
  can produce output assertion cards.
- Ambiguous command construction remains non-actionable.

#### Progress

- `fixtures/python_cli_output_repair_gap` now proves a simple Click
  `@click.command()` owner with changed `click.echo(...)` output routes to a
  bounded pytest repair card with `output contains ...`, a suggested existing
  test, and a focused verify command.
- `fixtures/python_argparse_output_repair_gap` now proves an argparse-shaped
  command can route a changed static `print(...)` output to a bounded pytest
  CLI output repair card without importing argparse or executing tests.
- Unit coverage pins Typer `app.command` transparency only when a `typer`
  import is present, keeps custom `app.command` decorators fail-closed, and
  recognizes `click.echo`, `typer.echo`, `sys.stdout.write`,
  `sys.stderr.write`, and simple literal `sys.exit` / `SystemExit` exit-code
  discriminators.
- Python repair-card copy now specializes output/call-effect side effects into
  CLI output or CLI exit-code assertion guidance when the missing discriminator
  is output text or a literal exit code.

### Work item: analysis/python-parametrized-boundaries

Status: done

Blocked by:

- `analysis/python-repair-classes-v1`

#### Goal

Suggest native pytest parameterization for clear boundary predicates.

#### Acceptance

- Suggest parameterization only when candidate values are explainable.
- Simpler one-case test remains available.
- Expected values are not invented without uncertainty labeling.

#### Progress

- Predicate-boundary repair cards now keep the equality discriminator as the
  minimum repair and offer optional pytest below/equal/above parameterized rows
  only for simple identifier or integer boundaries.
- Suggested assertion copy labels parameterized expected values as
  domain-specific placeholders instead of inventing outputs.
- Boundary cards add a stop condition telling agents and humans to keep only
  the equality assertion when below/above expected values are unclear.
- `fixtures/python_parametrized_boundary_repair_gap` pins the human and JSON
  output contract for this guidance.

### Work item: analysis/python-existing-test-strengthening

Status: done

Blocked by:

- `analysis/python-repair-classes-v1`

#### Goal

Prefer strengthening weak related tests over adding redundant tests.

#### Acceptance

- Cards can distinguish "strengthen existing test" from "add new test".
- Agent packets can restrict edits to one existing test.
- Outcome receipt shows broad oracle becoming more exact.

#### Delivered

- Direct weak pytest and unittest placements now emit
  `suggested_repair_action: strengthen_existing_test`, target the existing weak
  related test name/node, and verify that test instead of proposing a redundant
  new test.
- Python repair cards expose `repair_action`, render "strengthen existing"
  guidance in human, JSON, pilot, SARIF, and GitHub-projected card payloads,
  and keep the preview/advisory receipt boundary.
- Check-output-derived Python GapRecords map strengthening cards to
  `StrengthenExistingTest`, so `ripr agent packet --gap-ledger ...` emits
  `task = "strengthen_targeted_test"` with the existing test file as the
  allowed edit surface and production Python files forbidden.
- Python fixture goldens now pin the stronger routing across predicate,
  return, exception, field/object, output/log/call-effect, pytest, and unittest
  examples.

### Work item: swarm/python-gap-work-queue

Status: complete

Blocked by:

- `swarm/python-agent-packet-export`

#### Goal

Make multiple Python repair cards shardable.

#### Acceptance

- Queue entries include canonical gap ID, priority, owner, allowed edit files,
  verify command, expected receipt, and conflict group.
- Same-file conflicts and stale entries are visible.

#### Progress

- `ripr swarm queue --language python` ranks packetable Python GapRecords into
  conflict-grouped advisory work and excludes no-action, static-limit, and
  non-packetable records.
- Queue rendering now fails closed when a gap ledger omits root provenance or
  declares a different root from the selected `--root`, returning a blocked
  queue with no packets instead of assigning rootless, stale, or
  wrong-workspace repair work.
- Queue packets now surface explicit stale receipt movement from GapRecords as
  `queue_state = "blocked_stale"`, `staleness_status = "stale"`, and
  `summary.stale_total`, so a closed or stale Python repair packet is visible
  but not silently assignable.

### Work item: swarm/python-agent-result-ingestion

Status: done

Blocked by:

- `swarm/python-gap-work-queue`

#### Goal

Classify agent repair attempts without trusting them blindly.

#### Acceptance

- Ingested results classify closed, partially improved, verify failed, edited
  forbidden file, uncertain, and stale outcomes.
- Production-code edits are flagged.
- Verify result and before/after movement are attached.

#### Delivered

- `ripr swarm ingest --result <agent-result.json>` reads one external agent
  result artifact, validates that the result path stays under the selected
  root, and emits an advisory `swarm-ingest` JSON envelope without rerunning
  tests, writing receipts, calling providers, generating tests, or editing
  files.
- Ingest classification now distinguishes `closed`, `partially_improved`,
  `verify_failed`, `edited_forbidden_file`, `stopped_by_agent`,
  `stale_packet`, and `uncertain`; missing verify evidence stays uncertain,
  and forbidden production-code edits are flagged before any reported success
  claim.
- The Python preview first-PR fixture now includes an agent-result input and
  expected ingest output proving that a test-only edit with passing verify
  evidence and resolved receipt movement becomes `closed` /
  `attempt_outcome = "resolved"` while keeping `trusted_success = false` for
  operator review.

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
- `fixtures/first_successful_pr/python-preview-gap` now pins the same
  before/after check-output path with expected `ripr outcome` JSON and Markdown
  receipts for closed, unchanged, opened, strengthened, and weakened movement,
  proving the first-PR Python preview gap can close, remain weak, partially
  improve, weaken, or reopen without a Python-only receipt command.
- Strengthened-but-still-weak rows now stay visible in receipt
  `remaining_weak_or_unknown` output instead of being mistaken for closure.
- `fixtures/first_successful_pr/python-return-gap` pins a non-boundary
  return-value receipt where broad pytest evidence strengthens to an exact
  return assertion and closes the canonical Python gap.
- `fixtures/first_successful_pr/python-exception-gap` pins exception-path
  receipt movement where broad exception evidence strengthens to exact
  `pytest.raises(..., match=...)` message evidence and closes the canonical
  Python gap.
- `fixtures/first_successful_pr/python-field-gap` pins field/object receipt
  movement where broad object truthiness strengthens to exact returned-field
  evidence and closes the canonical Python gap.
- `fixtures/first_successful_pr/python-output-gap` pins output/log receipt
  movement where broad output smoke strengthens to exact output text evidence
  and closes the canonical Python gap.
- `ripr reports gap-ledger --check-output` now carries the corresponding
  receipt command into repairable Python GapRecords, which makes bounded
  packet delegation receipt-ready.
- Outcome, review-receipt, and agent-verify JSON now include
  `summary.gap_movement` counts for closed, opened, strengthened, weakened,
  unchanged, new, removed, and changed canonical gaps, so Python repair-loop
  receipts expose closure without requiring row-by-row inspection.

### Work item: fixtures/python-false-positive-corpus

Status: done

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

#### Delivered

- Check-output-derived Python static-limit findings now become report-only
  `StaticLimitation` GapRecords with `repairability = "analyzer_limitation"`.
  The swarm queue excludes those records instead of turning preview limitations
  into agent repair packets.
- `python_decorator_indirection_limit` pins decorated owners as
  `decorator_indirection` static limitations, so decorator-modified call
  semantics are named instead of treated as hidden analyzer truth.
- `python_opaque_custom_helper_limit` pins custom assertion helpers as
  `opaque_custom_assertion_helper` static limitations so the adapter does not
  route a repair packet when the helper body might already observe the changed
  discriminator.
- `python_property_based_limit` pins Hypothesis-style property-based tests as
  `property_based_test` static limitations so the adapter does not infer that
  generated inputs cover a concrete missing discriminator.
- `python_unresolved_fixture_limit` pins pytest fixture-sourced inputs and
  expected values as `unresolved_pytest_fixture` static limitations so the
  adapter does not turn opaque fixture data into a repair packet or a
  discriminator claim.
- Static-limit findings now keep revealability/discriminator evidence
  `unknown`, even when a related test has an exact-looking oracle, because the
  named limitation prevents a safe discriminator claim.
- `python_monkeypatch_module_limit` pins pytest `monkeypatch.setattr(...)`
  substitution as a `mocked_module` static limitation so monkeypatch-only
  related tests stay visible but do not become repair cards, canonical gaps, or
  swarm packets.
- `python_generated_file_excluded` pins detectable generated Python file diffs
  such as `*_pb2.py` as excluded from preview diff analysis, so generated-code
  edits do not produce repair cards, canonical repair gaps, or swarm packets.
- `python_dynamic_import_limit` pins runtime import calls such as
  `importlib.import_module(...)` as `missing_import_graph` static limitations,
  so exact-looking related tests stay visible but do not become repair cards,
  canonical gaps, or swarm packets.
- `python_metaclass_limit` pins `class ...(metaclass=...)` declarations as
  `metaprogramming` static limitations, so class-level magic is named and kept
  out of repair cards, canonical gaps, and swarm packets.
- `python_async_owner` pins async owner and async pytest-style test discovery
  without executing an event loop or treating async syntax as runtime proof.
- `python_broad_boolean_assertion` and `python_boundary_gap` pin broad-smoke
  and reach-only evidence as weak repair-routing inputs: they can become
  strengthen-existing-test cards only when a concrete missing discriminator,
  suggested test target, verify command, stop conditions, and advisory limits
  are available.
- `python_same_line_duplicate_collapse` pins a returned dict line containing
  return, field, and string literal signals as one user-facing canonical repair
  gap with `canonical_gap_group_size = 1`, preventing same-line raw-signal
  noise from inflating Python repair work.
- Unsupported cases produce named limitations and are excluded from swarm
  queues, while supported weak direct evidence remains repairable only when it
  can carry a bounded repair card and verify command.

### Work item: dogfood/python-real-repo-evals

Status: done

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

#### Progress

- `fixtures/real-repair-attempts/corpus.json` now includes a checked
  repo-local Python preview receipt where a bounded packet edits only
  `tests/test_pricing.py`, keeps `app/pricing.py` forbidden, verifies with a
  focused pytest command, and closes the canonical
  `amount >= threshold` predicate-boundary gap through `ripr outcome`.
- `cargo xtask dogfood` requires that Python receipt row as part of the
  durable repair-attempt corpus, so the first closed Python packet/receipt loop
  is visible in the same advisory dogfood report as other swarm repair
  attempts.
- `fixtures/python-real-repo-evals/corpus.json` now records a tiny controlled
  pytest scratch-repo eval where RIPR emits a predicate-boundary repair card,
  a human-run focused pytest command passes, and `ripr outcome` closes the
  canonical Python gap while preserving the preview/advisory claim boundary.
- The same corpus now records a normal pyproject-based pytest app eval where
  a `free_shipping_offer` threshold-boundary change routes to a
  strengthen-existing-test repair card, the focused pytest verify command
  passes, and `ripr outcome` closes the canonical Python gap.
- The same corpus now records a native pytest parametrized-boundary eval where
  a changed `amount >= threshold` predicate routes to a
  strengthen-existing-test card with optional below/equal/above row guidance,
  the bounded packet edits only `tests/test_tax.py`, the focused pytest verify
  command passes, and `ripr outcome` closes the canonical Python gap.
- The same corpus now records a CLI/output-style pytest eval where a changed
  `print(...)` side effect routes to a strengthen-existing-test repair card,
  the focused `capsys` pytest verify command passes, and `ripr outcome` closes
  the canonical Python output/call-effect gap.
- The same corpus now records a lightweight API-handler pytest eval where a
  changed `response.status_code` assignment routes to a field/object repair
  card, the focused status-code pytest verify command passes, and
  `ripr outcome` closes the canonical Python API status gap.
- The same corpus now records a mixed Rust/Python pytest eval where a Python
  `amount >= threshold` predicate-boundary change routes to a repair card
  despite Cargo metadata, the focused pytest verify command passes, and
  `ripr outcome` closes the canonical Python gap.
- The same corpus now records a decorated route pytest eval where a simple
  `@api.post(...)` route handler changes `response.status_code`, RIPR emits a
  field/object repair card with missing discriminator
  `response.status_code == 422`, the focused pytest verify command passes, and
  `ripr outcome` closes the canonical Python gap.
- The same corpus now records a dataclass/model-field pytest eval where a
  changed returned constructor field routes to a strengthen-existing-test card
  with missing discriminator `result.active == True`, the bounded packet edits
  only `tests/test_users.py`, the focused pytest verify command passes, and
  `ripr outcome` closes the canonical Python gap.
- `cargo xtask dogfood` projects the Python real-repo eval corpus into the
  dogfood report as receipt-backed eval evidence separate from analyzer
  fixture goldens.
- The corpus now supplies the receipt-backed dogfood evidence consumed by the
  route-quality metrics and scoped support-tier review.

### Work item: metrics/python-repair-routing-quality

Status: done

Blocked by:

- none; dogfood real-repo eval receipts are now fixture-backed.

#### Goal

Measure Python quality by repair usefulness, not finding volume.

#### Acceptance

- Metrics include time to first useful finding, top-1/top-3 actionable
  precision, verify-command validity, agent-packet boundary validity,
  concrete-discriminator rate, related-test-location rate, false-actionable
  rate, crash rate, unsupported limitation distribution, and receipt closure
  rate.
- Noisy changes fail quality gates.

#### Progress

- `cargo xtask dogfood` now derives Python repair-routing quality metrics from
  `fixtures/python-real-repo-evals/corpus.json`: top-1 actionable usefulness,
  top-3 actionable precision over captured ranked repair-card findings,
  verify-command validity, agent-packet boundary validity,
  concrete-discriminator coverage, suggested test-location coverage,
  false-actionable rate, crash rate, receipt closure rate, and unsupported
  limitation distribution.
- The Python eval corpus now records structured unsupported limitation kinds,
  and the decorated-route eval contributes `dynamic_route_registration` to the
  limitation distribution while keeping the support-tier boundary explicit.
- The Python eval corpus now records ranked top-3 repair-card findings for each
  dogfood case. Cases with fewer than three ranked repair cards must explain the
  capture limit so top-3 precision is measured without hiding sparse output.
- Corpus validation fails if the checked top Python repair cards become noisy:
  unusable top-1 card, invalid verify command, missing concrete discriminator,
  missing suggested test location, false actionability, crash/contract error, or
  no closed receipt. Validation also fails when ranked top-3 finding capture is
  missing, malformed, or not usable, concrete, placed, verifiable, and
  false-positive clean.
- This metric slice landed before support-tier review, so promotion evidence is
  based on top-finding usefulness and closure movement rather than raw finding
  volume.

### Work item: campaign/python-usable-alpha-promotion

Status: done

Blocked by:

- none; metrics and dogfood receipt evidence are checked.

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

#### Delivered

- `docs/status/SUPPORT_TIERS.md` now promotes only the scoped Python
  repair-routing loop to `usable alpha`: selected pytest/unittest repair cards,
  verify commands, bounded agent packets, queue/ingest handling, and
  before/after receipts.
- Root README, Quickstart, the language-adapter workflow, the capability matrix,
  and traceability now keep the same claim boundary: broader Python static facts
  and static limits remain preview/advisory.
- `docs/handoffs/2026-05-31-python-repair-routing-usable-alpha-closeout.md`
  records the proof commands, usable-alpha scope, remaining limits, policy
  non-claims, and next work.

### Work item: dogfood/python-stability-evals-v1

Status: in progress

Blocked by:

- `campaign/python-usable-alpha-promotion`

#### Goal

Extend Python repair-routing evidence after usable alpha before any broader
support-tier consideration.

#### Acceptance

- Add or refresh real or external-repo-style Python repair-routing evals beyond
  the promotion corpus.
- Each eval records command, runtime, top finding, repair card, agent packet,
  verify command, receipt or no-receipt reason, false-positive notes, and
  limitation notes.
- Route-quality metrics continue to emphasize top-1 usefulness, top-3
  precision, verify-command validity, agent-packet boundary validity, concrete
  discriminators, suggested test location, false-actionable rate, crash rate,
  receipt closure, and limitation distribution.
- No support-tier promotion, gate eligibility, badge authority, baseline/RIPR
  Zero inclusion, provider calls, generated tests, arbitrary imports, mutation
  execution, or production-code edit authority changes.

#### Proof commands

```bash
cargo xtask dogfood
cargo xtask metrics
cargo xtask check-capabilities
cargo xtask check-traceability
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

#### Rollback

- Revert the added eval entries, generated dogfood/metric expectations, and
  any docs or capability links. The scoped usable-alpha support claim remains
  unchanged.

#### Progress

- `fixtures/python-real-repo-evals/corpus.json` now records bounded agent
  packet fields for every checked Python dogfood eval: packet command, allowed
  test files, forbidden production files, and stop conditions.
- The corpus now adds `unittest_return_value_receipt` as a post-promotion
  stability eval where a unittest return-value repair routes to one existing
  test method, verifies with `python -m unittest`, exports a bounded test-only
  packet, and closes the canonical Python gap through `ripr outcome`.
- The corpus now adds `api_json_detail_pytest_receipt` as a post-promotion
  stability eval where an API response JSON detail repair routes to one
  existing pytest method, verifies with a focused `pytest` command, exports a
  bounded test-only packet, and closes the canonical Python gap through
  `ripr outcome`.
- The corpus now adds `exception_path_pytest_receipt` as a post-promotion
  stability eval where a broad `pytest.raises(ValueError)` observer
  strengthens to exact `pytest.raises(..., match=...)` evidence, verifies with
  a focused `pytest` command, exports a bounded test-only packet, and closes
  the canonical Python exception gap through `ripr outcome`.
- The corpus now adds `unittest_exception_path_receipt` as a post-promotion
  stability eval where a broad `self.assertRaises(ValueError)` observer
  strengthens to exact `self.assertRaisesRegex(...)` evidence, verifies with
  `python -m unittest`, exports a bounded test-only packet, and closes the
  canonical Python exception gap through `ripr outcome`.
- Route response-constructor assignments such as `response =
  Response(status_code=422, detail="coupon expired")` now route through the
  field/object repair path for statically recognized Python route owners. The
  corpus records `api_exception_response_pytest_receipt`, where a route catches
  an application exception, constructs a response object, recommends
  `assert response.status_code == 422`, verifies with focused `pytest`, exports
  a bounded test-only packet, and closes the canonical Python gap through
  `ripr outcome`.
- The corpus now records `decorator_indirection_no_packet_eval` as a
  post-promotion fail-closed stability eval where RIPR sees a related pytest
  exact oracle but refuses to emit a repair card, agent packet, verify success,
  or receipt movement because the changed owner is wrapped by a runtime
  decorator.
- The corpus now records `missing_import_graph_no_packet_eval` as a
  post-promotion fail-closed stability eval where RIPR sees a related pytest
  exact oracle but refuses to emit a repair card, agent packet, verify success,
  or receipt movement because the changed behavior depends on an imported
  implementation outside the static preview import graph.
- The corpus now records `mocked_module_no_packet_eval` as a post-promotion
  fail-closed stability eval where RIPR sees a related pytest exact oracle but
  refuses to emit a repair card, agent packet, verify success, or receipt
  movement because the related test depends on `unittest.mock.patch` runtime
  substitution.
- The corpus now records `opaque_custom_helper_no_packet_eval` as a
  post-promotion fail-closed stability eval where RIPR sees a related pytest
  method and custom `assert_*` helper oracle but refuses to emit a repair card,
  agent packet, verify success, or receipt movement because the helper body is
  opaque to the preview adapter.
- The corpus now records `property_based_no_packet_eval` as a post-promotion
  fail-closed stability eval where RIPR sees a related Hypothesis-style pytest
  method and weak relational oracle but refuses to emit a repair card, agent
  packet, verify success, or receipt movement because generated inputs do not
  prove that the changed discriminator is covered.
- Dogfood quality metrics now include agent-packet boundary validity so a
  future eval that lacks packet scope, stop conditions, or forbidden-file
  protection fails the checked quality gate instead of counting as usable.
