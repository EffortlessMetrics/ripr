# Python Repair Routing Current-State Inventory

Status: current snapshot

Owner: language-adapter / swarm

Created: 2026-05-29

Plan ID: RIPR-PLAN-0017

Linked plan:

- [Python repair routing implementation plan](implementation-plan.md)

Linked proposal:

- [RIPR-PROP-0017: Python Repair Routing Lane](../../docs/proposals/RIPR-PROP-0017-python-repair-routing-lane.md)

Linked specs and ADRs:

- [RIPR-SPEC-0026: Language adapter contract](../../docs/specs/RIPR-SPEC-0026-language-adapter-contract.md)
- [RIPR-SPEC-0028: Python preview static facts](../../docs/specs/RIPR-SPEC-0028-python-preview-static-facts.md)
- [ADR 0009: Python parser substrate](../../docs/adr/0009-python-parser-substrate.md)

## Scope

This is the inventory for work item
`docs/python-current-state-inventory`. It records the Python preview state
before any behavior change in the repair-routing lane.

This document does not promote Python support, change analyzer behavior,
define new output schema, add fixtures, run Python, or make Python findings
gate eligible. Python remains opt-in `preview` evidence.

## Summary

The repository already has a syntax-first Python preview adapter behind the
language adapter contract. It can parse `.py` files with
`rustpython-parser`, extract selected owners, tests, assertion/oracle facts,
probe shapes, related-test links, and static limits, then emit ordinary RIPR
findings with `language = "python"` and `language_status = "preview"` when
the repo enables Python in `ripr.toml`.

The current implementation is not yet the repair-routing loop from
RIPR-PROP-0017. The gaps are concentrated in project detection,
diff-to-owner mapping, language-neutral first-use CLI behavior, canonical
Python repair-gap projection, repair cards, verify commands, first-use routing,
and before/after receipts.

## Current Code Map

| Area | Current files | Current behavior |
| --- | --- | --- |
| Build feature | [`crates/ripr/Cargo.toml`](../../crates/ripr/Cargo.toml) | Default build enables `lang-python`; the feature pulls in optional `rustpython-parser`. |
| Config opt-in | [`crates/ripr/src/config.rs`](../../crates/ripr/src/config.rs) | Default `[languages]` is `["rust"]`; Python runs only when `python` is listed. |
| Router | [`crates/ripr/src/analysis/language/router.rs`](../../crates/ripr/src/analysis/language/router.rs) | `.py` paths route to `LanguageId::Python`; pipeline dispatch still depends on config. |
| Pipeline | [`crates/ripr/src/analysis/pipeline.rs`](../../crates/ripr/src/analysis/pipeline.rs) | Diff and repo pipelines can dispatch to `PythonAdapter` when the feature and config allow it. |
| Adapter | [`crates/ripr/src/analysis/language/python.rs`](../../crates/ripr/src/analysis/language/python.rs) | Extracts source-fact snapshots, preview owners, tests, oracles, related tests, probe shape, static limits, and `Finding` values. |
| Python helpers | [`crates/ripr/src/analysis/language/python/source_utils.rs`](../../crates/ripr/src/analysis/language/python/source_utils.rs) | Provides line/span/path helpers and Python test-file recognition. |
| Adapter tests | [`crates/ripr/src/analysis/language/python/python_tests.rs`](../../crates/ripr/src/analysis/language/python/python_tests.rs) | Pins assertion walking, probe classification, static limits, workspace exclusions, and diff analysis. |
| Human output | [`crates/ripr/src/output/human/sections.rs`](../../crates/ripr/src/output/human/sections.rs) | Renders preview language/status/owner metadata for non-Rust findings. |
| JSON output | [`crates/ripr/src/output/json/report.rs`](../../crates/ripr/src/output/json/report.rs) | Emits `language`, `language_status`, `owner_kind`, and `static_limit_kind` when present. |
| SARIF output | [`crates/ripr/src/output/sarif.rs`](../../crates/ripr/src/output/sarif.rs) | Renders diff-scoped findings by file/line and RIPR fields, but does not yet carry Python repair-card or language-specific metadata. |
| Repair packets | [`crates/ripr/src/output/gap_decision_ledger.rs`](../../crates/ripr/src/output/gap_decision_ledger.rs), [`crates/ripr/src/output/agent_seam_packets.rs`](../../crates/ripr/src/output/agent_seam_packets.rs) | `reports gap-ledger --check-output` can derive PR-local Python GapRecords from actionable `python_repair_card` findings, and `agent packet --gap-ledger` exports bounded packets with allowed test files, forbidden source files, conflict groups, verify commands, receipt status, and stop conditions. |

## Current Fact Coverage

| Fact family | Currently covered | Current limits |
| --- | --- | --- |
| Source snapshots | Stable file/span/language facts for modules, classes, functions, methods, decorators, parameters, returns, raises, predicates, comparisons, boolean expressions, calls, assignments, attribute writes, dict/list/set literals, string literals, and print/log calls. | Snapshot facts are still internal analysis substrate; they are not yet projected as canonical gap IDs or repair cards. |
| Owners | Top-level `def`, `async def`, methods, `@staticmethod`, `@classmethod` methods, class-body owners, module-level owners, and stable `python:<path>::<qualified_owner>` probe owner IDs. | Class owners intentionally omit `owner_kind` until the shared vocabulary adds a class value; canonical repair-gap IDs remain planned. |
| Tests | `test_*` functions, async `test_*`, `class Test*` pytest methods, `unittest.TestCase` `test_*` methods, fixture/parameter names, and test files by `test_*.py`, `*_test.py`, or `tests/` paths. | API client, CLI runner, and framework fixture semantics are recorded syntactically but not yet converted into repair locations or verify commands. |
| Pytest oracles | `assert a == b`, boundary comparisons, field assertions, output observers through `caplog` / `capsys`, status-code and exit-code assertions, bare `assert expr`, custom `assert_*` helpers, `isinstance(...)`, `pytest.raises(...)` / imported `raises(...)`, and `pytest.mark.parametrize` presence. | Boundary discriminator extraction is limited to simple syntax-derived predicate comparisons; no `match=` message observer, response JSON observer, or framework-shaped repair card exists yet. |
| Unittest oracles | `assertEqual`, `assertNotEqual`, `assertTrue`, `assertFalse`, `assertRaises`, `assertRaisesRegex`, `assertIn`, `assertRegex`, `assertDictEqual`, and unittest verify-command evidence. | Command confidence and repair-card placement remain planned. |
| Mock oracles | Common `mock.assert_called*` family is `mock_expectation` / medium. | Runtime mock substitution is not resolved; patched or monkeypatched modules surface as static limits. |
| Related tests | Direct owner calls, module import-alias calls, method attribute calls, same-stem file proximity, test-name similarity, and fixture-name proximity. Heuristic-only links are marked uncertain and keep weak reachability. | Route/client references and class references beyond simple calls are not yet repair-card inputs. |
| Probe shapes | Predicate/control, return value, error path, field assignment, returned dict fields, side-effect calls, await calls, and mock initializer shapes. | Canonical Python gap IDs now identify non-static-limit preview findings by language, file, owner, behavior kind, probe kind, and normalized discriminator; repair-card identity and closure receipts remain planned. |
| RIPR evidence | Non-static Python findings carry reach, infection, propagation, observation, discriminator evidence, and selected repair-class missing discriminators using Python behavior-family summaries. | Evidence remains syntax-first and preview; it does not execute imports, run tests, generate repairs, or claim mutation adequacy. |
| Static limits | `dynamic_dispatch`, `metaprogramming`, `decorator_indirection`, `mocked_module`, `missing_import_graph`, and `unsupported_syntax`. | Limits fail closed as `static_unknown` with typed stop reasons and no repair recommendation or canonical repair-gap ID. |

## Existing Fixture Corpus

The traceability and capability artifacts currently list these Python preview
fixture families:

- owner and routing fixtures:
  [`fixtures/python_boundary_gap`](../../fixtures/python_boundary_gap),
  [`fixtures/python_async_owner`](../../fixtures/python_async_owner),
  [`fixtures/python_method_owner`](../../fixtures/python_method_owner),
  [`fixtures/python_class_method_owner`](../../fixtures/python_class_method_owner),
  [`fixtures/python_owner_file_match`](../../fixtures/python_owner_file_match),
  [`fixtures/python_no_projectable_owner`](../../fixtures/python_no_projectable_owner),
  [`fixtures/python_mixed_language_no_cross_route`](../../fixtures/python_mixed_language_no_cross_route),
  and [`fixtures/python_disabled`](../../fixtures/python_disabled).
- assertion/oracle fixtures:
  [`fixtures/python_strong_oracle`](../../fixtures/python_strong_oracle),
  [`fixtures/python_broad_boolean_assertion`](../../fixtures/python_broad_boolean_assertion),
  [`fixtures/python_pytest_raises`](../../fixtures/python_pytest_raises),
  [`fixtures/python_pytest_oracle_shapes`](../../fixtures/python_pytest_oracle_shapes),
  [`fixtures/python_unittest_basic`](../../fixtures/python_unittest_basic),
  [`fixtures/python_unittest_assertions`](../../fixtures/python_unittest_assertions),
  [`fixtures/python_parametrize_basic`](../../fixtures/python_parametrize_basic),
  and [`fixtures/python_mock_assert_called`](../../fixtures/python_mock_assert_called).
- probe-shape fixtures:
  [`fixtures/python_return_value_shape`](../../fixtures/python_return_value_shape),
  [`fixtures/python_error_path_shape`](../../fixtures/python_error_path_shape),
  [`fixtures/python_field_assignment_shape`](../../fixtures/python_field_assignment_shape),
  [`fixtures/python_dict_field_repair_gap`](../../fixtures/python_dict_field_repair_gap),
  [`fixtures/python_ranking_noise_control`](../../fixtures/python_ranking_noise_control),
  [`fixtures/python_test_placement_verify`](../../fixtures/python_test_placement_verify),
  [`fixtures/python_call_argument_shape`](../../fixtures/python_call_argument_shape),
  and [`fixtures/python_mock_interaction_shape`](../../fixtures/python_mock_interaction_shape).
- related-test fixtures:
  [`fixtures/python_cross_file_import_reference`](../../fixtures/python_cross_file_import_reference),
  [`fixtures/python_same_stem_test`](../../fixtures/python_same_stem_test),
  [`fixtures/python_related_test_name_similarity`](../../fixtures/python_related_test_name_similarity),
  [`fixtures/python_fixture_name_relation`](../../fixtures/python_fixture_name_relation),
  and [`fixtures/python_unrelated_test_mention`](../../fixtures/python_unrelated_test_mention).
- static-limit fixtures:
  [`fixtures/python_dynamic_dispatch_limit`](../../fixtures/python_dynamic_dispatch_limit),
  [`fixtures/python_decorator_indirection_limit`](../../fixtures/python_decorator_indirection_limit),
  [`fixtures/python_mocked_module_limit`](../../fixtures/python_mocked_module_limit),
  [`fixtures/python_generated_file_excluded`](../../fixtures/python_generated_file_excluded),
  [`fixtures/python_missing_import_graph_limit`](../../fixtures/python_missing_import_graph_limit),
  [`fixtures/python_metaprogramming_limit`](../../fixtures/python_metaprogramming_limit),
  and [`fixtures/python_unsupported_syntax_limit`](../../fixtures/python_unsupported_syntax_limit).

The current fixtures prove preview facts, output metadata, placement guidance,
repair cards, and static verify commands for direct weak pytest/unittest
findings. They also prove agent packet safety through GapRecord projection for
direct weak repair cards. They do not yet prove canonical gap closure or
before/after receipt movement.

## First Fixture Matrix

| Matrix case | Current fixture home | Current preview coverage | Repair-routing work still needed |
| --- | --- | --- | --- |
| `basic_function` | `python_owner_file_match`, `python_boundary_gap` | Top-level function owners, direct calls, exact and weak oracle examples. | Add Python-only project detection and a repair-card fixture that does not depend on Cargo workspace assumptions. |
| `predicate_boundary` | `python_boundary_gap`, `python_strong_oracle`, `python_parametrize_basic`, `python_test_placement_verify`, `first_successful_pr/python-preview-gap` | Predicate probe, weak/strong related-test examples, canonical gap identity, simple equality-boundary missing discriminators such as `amount == threshold`, pytest placement/verify guidance, repair-card projection, and before/after `ripr outcome` fixtures where the canonical Python gap closes, stays unchanged, or opens when the equality discriminator is removed or reappears. | Add broader strengthened/weakened/regressed Python receipt fixtures before dogfood promotion. |
| `changed_return_value` | `python_return_value_shape`, `python_broad_boolean_assertion`, `python_unittest_oracle_shapes` | Return-value probes now distinguish exact observed examples from weak direct examples carrying returned-value missing discriminators such as `return value == amount >= 100`, plus static placement/verify guidance and repair-card projection for direct weak tests. | Add receipt command support and richer expected-value guidance. |
| `changed_exception` | `python_error_path_shape`, `python_pytest_raises`, `python_unittest_assertions`, `python_test_placement_verify` | Error-path probes plus pytest/unittest broad-error observers now carry exception missing discriminators such as `raises ValueError matching "positive required"` when direct weak evidence exists, including unittest placement/verify guidance and repair-card assertions. | Add closure receipt support once Python outcome records exist. |
| `dict_field_change` | `python_field_assignment_shape`, `python_dict_field_repair_gap` | Attribute assignment probes and returned dict fields can carry field/object missing discriminators such as `self.status == "paid"` and `status == "paid"` while exact related assertions remain observed; direct weak findings carry placement/verify guidance. | Add richer dict/object/dataclass repair cards. |
| `pytest_exact_assert` | `python_strong_oracle`, `python_owner_file_match`, `python_return_value_shape`, `first_successful_pr/python-preview-gap` | `assert ... == ...` becomes `exact_value` / strong and can classify as `exposed`; the first-PR Python preview fixture now records that weak-to-exact movement as a closed canonical gap in `ripr outcome`. | Add more receipt cases for non-boundary repair classes. |
| `pytest_smoke_assert` | `python_boundary_gap`, `python_broad_boolean_assertion` | Unknown, reach-only, or smoke oracle keeps finding `weakly_exposed`, and JSON evidence records the non-exact oracle shape. | Prefer strengthening the existing weak test when safe instead of always adding a new test. |
| `unittest_assert_equal` | `python_unittest_assertions`, `python_unittest_oracle_shapes`, `python_test_placement_verify` | `self.assertEqual(...)` becomes `exact_value` / strong; unittest related tests now carry `python -m unittest module.Class.test_method` verify-command evidence, and `assertIn` / `assertRegex` / `assertDictEqual` feed output, status-code, and field oracle shapes. Direct weak unittest findings also get suggested test methods and verify commands. | Tie unittest placement to repair cards and agent packets. |
| `fastapi_route_optional` | Missing | FastAPI/Flask decorators currently look like decorator or call syntax, not framework facts. | Add HTTP/API fixture pack with route owner, status-code, and JSON-field repair cards; keep dynamic routing limited. |
| `cli_output_optional` | Partial: `python_pytest_oracle_shapes`, `python_call_argument_shape` | Generic call, log/output, and side-effect shapes can carry missing discriminators such as `log contains "coupon expired"` or `call includes "receipt.sent"` when direct weak evidence exists, with generic placement/verify guidance. | Add Click/Typer/argparse output fixtures, output assertion cards, and exit-code-specific verify guidance. |
| `dynamic_unsupported` | Static-limit and generated-file fixture family | Dynamic dispatch, decorator, mocked module, missing import graph, metaprogramming, and unsupported syntax limits are visible and fail closed with typed stop reasons; detectable generated Python diffs are excluded before repair routing. | Keep those limitations and generated-file changes out of first-use and queue projections while repairable Python cards move through agent packets. |

## Current Rust/Cargo Assumptions To Remove Or Contain

| Assumption | Current owner | Why it blocks the lane |
| --- | --- | --- |
| Missing `ripr.toml` used to be Rust-only for every repo shape. | `config` / `analysis` | Contained by `analysis/python-project-detection`: Python project markers now select Python preview when config is absent, and explicit `ripr.toml` still wins. |
| `ripr pilot` builds the first-use packet from repo seam inventory. | `cli` / `analysis::seam_inventory` / `output::pilot` | Partly contained: Python project/diff runs now project the top `python_repair_card` into the pilot summary, and `ripr first-pr` can select preview Python GapRecords from an existing gap ledger. Python-only first-pr recovery can now generate that ledger through `ripr check --json` and `reports gap-ledger --check-output`; Python repo-mode facts remain a follow-up. |
| Python repo-mode analysis returns no findings. | `PythonAdapter::analyze_repo` | `ripr pilot` and repo-baseline loops cannot rely on Python repo facts until repo-mode or a Python-specific first-use bridge exists. |
| The summary JSON field is named `changed_rust_files`. | `domain::Summary` / `output::json` | Python and mixed-language reports currently carry a Rust-shaped summary field even when the counted changed file is `.py`. |
| Workspace exclusions must stay aligned with the Python-lane contract. | `PythonAdapter::visit_workspace` | Contained by `analysis/python-project-detection` for `.tox`, `.nox`, `site-packages`, `.pytest_cache`, `dist`, `build`, and detectable generated Python files. |
| `first-pr`, `agent packet`, and receipt flows consume existing gap/seam artifacts. | `output::first_pr`, `output::agent_seam_packets`, `output::outcome` | Actionable Python repair cards can now become PR-local GapRecords and agent packets with allowed files, forbidden files, verify commands, and deferred receipt status; `ripr first-pr` can select those Python preview GapRecords into a preview-limited start-here packet. Before/after Python receipts remain planned. |

## Output Surface Inventory

| Surface | Current Python behavior | Work remaining for repair routing |
| --- | --- | --- |
| `ripr check --format human` | Renders Python preview findings and direct weak repair cards with changed owner, missing discriminator, test shape, location, verify command, preview/advisory authority, deferred receipt status, stop conditions, and limits when config enables Python. | Raw check cards still need caller-specific receipt paths; the check-output gap-ledger and first-PR bridges synthesize concrete receipt commands. |
| `ripr check --json` | Emits Python metadata fields, canonical gap IDs, additive `repair_placement` objects, and additive `python_repair_card` objects on direct weak actionable findings. | Add full receipt payloads and direct first-use selection. |
| `ripr pilot` | Produces the existing Rust seam-oriented pilot packet and, when Python preview diff evidence yields a repair card, shows the top Python repairable gap with supported/deferred features, limitation count, verify command, and deferred receipt guidance. | Add Python repo-mode facts and direct receipt projection once outcome/ledger support exists. |
| `ripr first-pr` / start-here | Can select repairable Python preview GapRecords from an existing gap ledger, accept Python-only project roots with markers, write preview-limited `start-here.{json,md}` with missing discriminator, verify command, and receipt command, and recover a missing Python-only gap ledger through `ripr check --json` plus `reports gap-ledger --check-output` instead of Rust repo-exposure. The first-PR Python preview fixture now includes before/after check-output inputs and expected `ripr outcome` receipts for closed, unchanged, and opened gap movement. | Add dogfood receipts from real Python repos. |
| SARIF | Renders generic diff finding locations and RIPR properties. | Preserve Python language/status/static-limit metadata and repair-card context. |
| Generated CI summary | Can group preview evidence when configured. | Add safe Python advisory mode with repair-card artifacts and fork-safe posture. |
| PR summary/front panel | Can consume existing report/gap artifacts and now highlights the top Python preview repair card from `actionable-gaps.json` with the canonical gap, changed owner, missing discriminator, suggested test target, verify command, receipt command, stop conditions, and advisory boundary. | Add no-action states from canonical Python gaps. |
| LSP/editor | Preview routing and metadata projection exist for Python. Python preview GapRecord diagnostics with safe `pytest ...` or `python -m unittest ...` verify commands can now expose verify/receipt actions, copy a full repair card for safe target-file routes with a current validated GapRecord freshness cue, copy a pytest skeleton, and open the target test file even when the repair route carries a bare test name. | Add copyable agent packets and richer stale-state warnings for Python-specific cards. |
| Agent packet | Actionable Python repair cards can be projected to GapRecords through `ripr reports gap-ledger --check-output`, exported through `ripr agent packet --gap-ledger ... --gap-id ... --json` with allowed test files, forbidden source files, conflict groups, stop conditions, verify commands, and receipt status, queued through `ripr swarm queue --language python`, and classified after an external attempt through `ripr swarm ingest --result ...`. | Add richer stale-queue detection and outcome-ledger joins once dogfood attempts produce more Python receipts. |
| Outcome/ledger | `ripr outcome` can compare check-output findings by Python canonical gap ID, and `first_successful_pr/python-preview-gap` pins closed, unchanged, and opened Python preview gap receipts in JSON and Markdown. | Expand Python receipt fixtures for strengthened, weakened, and regressed movement before support-tier promotion. |

## Next Work Item Readiness

The next `cli/python-first-use-path` slice can start from this boundary:

- Python project detection keeps no-config Python repos analyzable without
  weakening explicit `ripr.toml` authority.
- `ripr pilot` can now bridge diff-scoped Python repair cards into first-screen
  CLI output without requiring Cargo.
- `ripr first-pr` can now bridge an existing Python preview gap ledger into a
  preview-limited start-here packet for Python-only project roots.
- Missing Python-only `first-pr` gap ledgers now recover through the existing
  check-output bridge (`ripr check --json` then `reports gap-ledger
  --check-output`) instead of Rust repo-exposure.
- Python analysis now reuses a source-fact snapshot instead of separate
  parser passes for owner and test extraction.
- Malformed Python records an internal `unsupported_syntax` source-fact
  limitation.
- Source-fact tests cover the syntax vocabulary needed before canonical gaps can
  be added.
- Python diff findings now carry stable, language-qualified `probe.owner` IDs
  for functions, methods, classes, and module-level changes, and output tests
  prove the owner is visible in JSON and human reports.
- Pytest preview evidence now records fixture parameters, `class Test*`
  discovery, output/status/field/boundary/smoke/custom-helper oracle shapes,
  and conservative reach-only evidence without changing support tier or
  emitting repair cards.
- Unittest preview evidence now records class-qualified selectors,
  framework-shaped `python -m unittest` verify commands, and
  output/status/field oracle shapes from common assertion-call arguments.
- Related-test evidence now orders direct calls and import-alias calls ahead of
  heuristic links, adds conservative test-name and fixture-name proximity, and
  marks same-stem/name/fixture links as uncertain with weak reachability.
- Non-static-limit Python findings now carry stable `canonical_gap_id` values
  across JSON, human, SARIF, GitHub annotation, LSP diagnostic/hover, and
  context-packet surfaces.
- Python RIPR stage evidence now distinguishes reachability, changed-behavior
  infection, propagation, observation, and revealability for non-static
  findings.
- Static-limit findings intentionally fail closed as `static_unknown` with
  typed stop reasons, no canonical repair-gap ID, and no repair recommendation.
- Simple predicate-boundary findings can carry activation-level missing
  discriminator facts such as `amount == threshold`.
- Direct weak Python findings can carry first repair-class discriminators for
  predicate boundary, return value, exception path, field/object value, and
  output/log/call effects.
- Strong-oracle, no-path, heuristic-only, and static-limit Python findings
  suppress repair guidance rather than becoming repair-ready work.
- Ranking-facing output now puts direct repairable weak Python gaps ahead of
  observed, no-path, heuristic-only, and static-limit preview findings while
  keeping non-actionable findings visible.
- Direct weak pytest and unittest findings now carry suggested test file,
  suggested test name, framework-shaped verify command, command confidence, and
  pytest node IDs when applicable.
- Direct weak Python findings that already have canonical gap, concrete missing
  discriminator, related-test evidence, placement, and verify-command evidence
  now carry `python_repair_card` output in JSON and human reports.
- `reports gap-ledger --check-output` can turn those cards into PR-local
  Python GapRecords, and `agent packet --gap-ledger` can export bounded agent
  packets with allowed files, forbidden files, conflict groups, verify commands,
  deferred receipt status, and stop conditions.

Acceptance for the next behavior PR should expand Python before/after receipt
coverage beyond the closed predicate-boundary case or add direct dogfood
receipts from a real Python repository, without generated tests or stronger
runtime/correctness claims.
