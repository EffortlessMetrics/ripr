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
Python gap identity, missing-discriminator extraction, repair cards, verify
commands, agent packets, and before/after receipts.

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
| Repair packets | [`crates/ripr/src/output/agent_seam_packets.rs`](../../crates/ripr/src/output/agent_seam_packets.rs) | Can render language/status from gap records, but Python adapter findings are not yet converted into repair-ready gap records. |

## Current Fact Coverage

| Fact family | Currently covered | Current limits |
| --- | --- | --- |
| Source snapshots | Stable file/span/language facts for modules, classes, functions, methods, decorators, parameters, returns, raises, predicates, comparisons, boolean expressions, calls, assignments, attribute writes, dict/list/set literals, string literals, and print/log calls. | Snapshot facts are still internal analysis substrate; they are not yet projected as canonical gap IDs or repair cards. |
| Owners | Top-level `def`, `async def`, methods, `@staticmethod`, `@classmethod` methods, class-body owners, module-level owners, and stable `python:<path>::<qualified_owner>` probe owner IDs. | Class owners intentionally omit `owner_kind` until the shared vocabulary adds a class value; canonical repair-gap IDs remain planned. |
| Tests | `test_*` functions, async `test_*`, tests inside classes, `unittest.TestCase` `test_*` methods, and test files by `test_*.py`, `*_test.py`, or `tests/` paths. | Fixture parameters, custom helpers, API clients, CLI runners, and framework fixtures are not modeled as first-class test facts. |
| Pytest oracles | `assert a == b`, non-equality comparisons, bare `assert expr`, `isinstance(...)`, `pytest.raises(...)`, and `pytest.mark.parametrize` presence. | No exact boundary discriminator extraction, no `match=` message observer, no `capsys`, `caplog`, status-code, response JSON, or custom helper classification. |
| Unittest oracles | `assertEqual`, `assertNotEqual`, `assertTrue`, `assertFalse`, `assertRaises`, and `assertRaisesRegex`. | `assertIn`, `assertRegex`, `assertDictEqual`, verify-command selection, and command confidence remain planned. |
| Mock oracles | Common `mock.assert_called*` family is `mock_expectation` / medium. | Runtime mock substitution is not resolved; patched or monkeypatched modules surface as static limits. |
| Related tests | Direct owner calls, module import-alias calls, method attribute calls, and same-stem file proximity. | Route/client references, fixture names, class references beyond simple calls, and uncertain relation reasons are not yet repair-card inputs. |
| Probe shapes | Predicate/control, return value, error path, field assignment, side-effect calls, await calls, and mock initializer shapes. | No behavior-kind-specific canonical gap ID, normalized expression, field/output discriminator, or stable repair identity yet. |
| Static limits | `dynamic_dispatch`, `metaprogramming`, `decorator_indirection`, `mocked_module`, `missing_import_graph`, and `unsupported_syntax`. | Limits are emitted as finding context; they are not yet stop reasons for a Python repair queue. |

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
  [`fixtures/python_unittest_basic`](../../fixtures/python_unittest_basic),
  [`fixtures/python_unittest_assertions`](../../fixtures/python_unittest_assertions),
  [`fixtures/python_parametrize_basic`](../../fixtures/python_parametrize_basic),
  and [`fixtures/python_mock_assert_called`](../../fixtures/python_mock_assert_called).
- probe-shape fixtures:
  [`fixtures/python_return_value_shape`](../../fixtures/python_return_value_shape),
  [`fixtures/python_error_path_shape`](../../fixtures/python_error_path_shape),
  [`fixtures/python_field_assignment_shape`](../../fixtures/python_field_assignment_shape),
  [`fixtures/python_call_argument_shape`](../../fixtures/python_call_argument_shape),
  and [`fixtures/python_mock_interaction_shape`](../../fixtures/python_mock_interaction_shape).
- related-test fixtures:
  [`fixtures/python_cross_file_import_reference`](../../fixtures/python_cross_file_import_reference),
  [`fixtures/python_same_stem_test`](../../fixtures/python_same_stem_test),
  and [`fixtures/python_unrelated_test_mention`](../../fixtures/python_unrelated_test_mention).
- static-limit fixtures:
  [`fixtures/python_dynamic_dispatch_limit`](../../fixtures/python_dynamic_dispatch_limit),
  [`fixtures/python_decorator_indirection_limit`](../../fixtures/python_decorator_indirection_limit),
  [`fixtures/python_mocked_module_limit`](../../fixtures/python_mocked_module_limit),
  [`fixtures/python_missing_import_graph_limit`](../../fixtures/python_missing_import_graph_limit),
  [`fixtures/python_metaprogramming_limit`](../../fixtures/python_metaprogramming_limit),
  and [`fixtures/python_unsupported_syntax_limit`](../../fixtures/python_unsupported_syntax_limit).

The current fixtures prove preview facts and output metadata. They do not yet
prove Python repair cards, canonical gap closure, verify commands, agent
packet safety, or before/after receipt movement.

## First Fixture Matrix

| Matrix case | Current fixture home | Current preview coverage | Repair-routing work still needed |
| --- | --- | --- | --- |
| `basic_function` | `python_owner_file_match`, `python_boundary_gap` | Top-level function owners, direct calls, exact and weak oracle examples. | Add Python-only project detection and a repair-card fixture that does not depend on Cargo workspace assumptions. |
| `predicate_boundary` | `python_boundary_gap`, `python_strong_oracle`, `python_parametrize_basic` | Predicate probe and weak/strong related-test examples. | Extract a canonical missing discriminator such as `amount == threshold`; avoid line-number-only gap identity. |
| `changed_return_value` | `python_return_value_shape` | Return-value probe with related exact pytest assertion. | Produce a return-value repair card, suggested assertion, and verify command. |
| `changed_exception` | `python_error_path_shape`, `python_pytest_raises`, `python_unittest_assertions` | Error-path probe plus pytest/unittest broad-error observers. | Distinguish exception type versus message discriminator, including `pytest.raises(..., match=...)` guidance. |
| `dict_field_change` | Partial: `python_field_assignment_shape` | Attribute assignment probe with exact related assertion. | Add dict/object/dataclass return-field fixtures and field-specific repair cards. |
| `pytest_exact_assert` | `python_strong_oracle`, `python_owner_file_match`, `python_return_value_shape` | `assert ... == ...` becomes `exact_value` / strong and can classify as `exposed`. | Tie exact assertions to a canonical gap closing receipt, not just a preview finding class. |
| `pytest_smoke_assert` | `python_boundary_gap`, `python_broad_boolean_assertion` | Unknown or smoke oracle keeps finding `weakly_exposed`. | Prefer strengthening the existing weak test when safe instead of always adding a new test. |
| `unittest_assert_equal` | `python_unittest_assertions` | `self.assertEqual(...)` becomes `exact_value` / strong. | Build unittest verify-command selection and add remaining unittest assertion shapes. |
| `fastapi_route_optional` | Missing | FastAPI/Flask decorators currently look like decorator or call syntax, not framework facts. | Add HTTP/API fixture pack with route owner, status-code, and JSON-field repair cards; keep dynamic routing limited. |
| `cli_output_optional` | Missing | Generic call and side-effect shapes exist, but CLI runners and stdout/stderr assertions are not modeled. | Add Click/Typer/argparse output fixtures, output assertion cards, and exit-code verify guidance. |
| `dynamic_unsupported` | Static-limit fixture family | Dynamic dispatch, decorator, mocked module, missing import graph, metaprogramming, and unsupported syntax limits are visible. | Convert applicable limits into non-actionable stop reasons so repair queues do not assign unsafe work. |

## Current Rust/Cargo Assumptions To Remove Or Contain

| Assumption | Current owner | Why it blocks the lane |
| --- | --- | --- |
| Missing `ripr.toml` used to be Rust-only for every repo shape. | `config` / `analysis` | Contained by `analysis/python-project-detection`: Python project markers now select Python preview when config is absent, and explicit `ripr.toml` still wins. |
| `ripr pilot` builds the first-use packet from repo seam inventory. | `cli` / `analysis::seam_inventory` / `output::pilot` | Python findings can appear in `ripr check`, but the first-use path in RIPR-PROP-0017 needs Python project detection and top repair-card selection. |
| Python repo-mode analysis returns no findings. | `PythonAdapter::analyze_repo` | `ripr pilot` and repo-baseline loops cannot rely on Python repo facts until repo-mode or a Python-specific first-use bridge exists. |
| The summary JSON field is named `changed_rust_files`. | `domain::Summary` / `output::json` | Python and mixed-language reports currently carry a Rust-shaped summary field even when the counted changed file is `.py`. |
| Workspace exclusions must stay aligned with the Python-lane contract. | `PythonAdapter::visit_workspace` | Contained by `analysis/python-project-detection` for `.tox`, `.nox`, `site-packages`, `.pytest_cache`, `dist`, `build`, and detectable generated Python files. |
| `first-pr`, `agent packet`, and receipt flows consume existing gap/seam artifacts. | `output::first_pr`, `output::agent_seam_packets`, `output::outcome` | Python adapter findings are not yet projected into canonical repair-gap records with allowed files, forbidden files, verify commands, and receipt commands. |

## Output Surface Inventory

| Surface | Current Python behavior | Work remaining for repair routing |
| --- | --- | --- |
| `ripr check --format human` | Renders Python preview findings when config enables Python. | Add compact repair-card sections with changed owner, missing discriminator, test shape, location, verify, receipt, and stop conditions. |
| `ripr check --json` | Emits Python metadata fields on findings. | Add stable canonical gap fields and repair-card/verify/receipt payloads. |
| `ripr pilot` | Produces the existing Rust seam-oriented pilot packet. | Detect Python repos and select the top Python repairable gap without Cargo assumptions. |
| `ripr first-pr` / start-here | Can display preview language/static-limit warnings when supplied by existing artifacts. | Generate Python start-here content directly from Python canonical gaps. |
| SARIF | Renders generic diff finding locations and RIPR properties. | Preserve Python language/status/static-limit metadata and repair-card context. |
| Generated CI summary | Can group preview evidence when configured. | Add safe Python advisory mode with repair-card artifacts and fork-safe posture. |
| PR summary/front panel | Can consume existing report/gap artifacts. | Highlight top Python repair cards and no-action states from canonical Python gaps. |
| LSP/editor | Preview routing and metadata projection exist for Python. | Project Python repair cards, skeleton copy actions, related test paths, and stale-state warnings. |
| Agent packet | Gap-record packet rendering can include language/status. | Export deterministic Python packets with allowed test files, forbidden source files, stop conditions, verify, and receipt. |
| Outcome/ledger | Existing static outcome and ledger flows are Rust/gap-record oriented. | Track Python canonical gaps opened, closed, unchanged, strengthened, weakened, and newly introduced. |

## Next Work Item Readiness

The next work item, `analysis/python-pytest-oracles`, can start from this
boundary:

- Python project detection keeps no-config Python repos analyzable without
  weakening explicit `ripr.toml` authority.
- Python analysis now reuses a source-fact snapshot instead of separate
  parser passes for owner and test extraction.
- Malformed Python records an internal `unsupported_syntax` source-fact
  limitation.
- Source-fact tests cover the syntax vocabulary needed before canonical gaps can
  be added.
- Python diff findings now carry stable, language-qualified `probe.owner` IDs
  for functions, methods, classes, and module-level changes, and output tests
  prove the owner is visible in JSON and human reports.

Acceptance for the next behavior PR should deepen pytest oracle extraction
without promoting Python beyond preview or emitting repair cards before missing
discriminators and verify commands exist.
