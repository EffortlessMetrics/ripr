# RIPR-SPEC-0028: Python Preview Static Facts

Status: proposed

## Problem

The Python preview adapter must emit RIPR static facts from Python source
without depending on `mypy`, `pyright`, an import graph, or any runtime
tooling. Syntax-first facts are the contract; semantic enrichment is
explicitly deferred.

This spec defines the per-language behavior the adapter must produce. The
language-neutral boundary, the router, the output metadata, the opt-in
posture, and the cross-language non-goals live in
[RIPR-SPEC-0026: Language adapter contract](RIPR-SPEC-0026-language-adapter-contract.md).
The proposal context is
[RIPR-PROP-0001: Multi-Language Adapter Preview](../proposals/RIPR-PROP-0001-multi-language-adapter-preview.md).

## Behavior

The Python preview adapter is enabled by repo configuration:

```toml
[languages]
enabled = ["rust", "python"]
```

When enabled, it routes `*.py` files. It emits the same RIPR fact
families as the Rust adapter and tags each finding with
`language = "python"` and `language_status = "preview"`.

The adapter is syntax-first. It must not depend on `mypy`, `pyright`, a
runtime test runner, or an import graph. When syntax-first analysis
cannot classify, the adapter emits an explicit `static_limit_kind`
instead of silently coercing to `no_static_path`.

## Inputs

- Python source files routed to this adapter.
- Diff spans inside those files.
- Repo configuration including `[languages] enabled` and any future
  Python-specific options layered on top of this spec.

The adapter does not read `pyproject.toml` build metadata, requirements
files, virtualenv contents, generated stubs, or runtime test output.

## Owner Facts

Owners the adapter must recognise:

- top-level `def` functions and `async def` functions
- nested functions only when they participate in changed behavior
- `class` definitions and their methods (regular, `@staticmethod`,
  `@classmethod`)
- decorated functions and methods; the decoration is preserved as
  syntactic context, not resolved semantically
- module-scope expressions whose right-hand side participates in changed
  behavior (when a probe attaches to the expression)

Owner kinds emitted in output (per RIPR-SPEC-0026):

- `function`, `method`, `class_method`, `module_function`.

## Test and Assertion Facts

Test discovery:

- `pytest` test functions named `test_*` at module level
- `unittest.TestCase` subclasses and their `test_*` methods
- parametrized tests via `@pytest.mark.parametrize` (recognised
  syntactically)
- fixture files matched by configured patterns (default: `test_*.py`
  and `*_test.py`; the configured pattern is part of the repo config
  cross-spec contract)

Assertions / oracles the adapter must recognise:

- bare `assert expr` → smoke oracle
- `assert a == b` and `assert a != b` → exact-value oracle (for `==`) or
  smoke-style negative oracle (for `!=`, recorded as broad)
- `assert isinstance(value, SomeType)` → broad-type oracle
- `pytest.raises(...)` context manager → error-path oracle
- `self.assertEqual(a, b)` and `assertNotEqual` → exact-value oracle
- `self.assertRaises(...)` → error-path oracle
- `self.assertTrue(...)` / `assertFalse(...)` → smoke oracle
- `mock.assert_called*` family (`assert_called_once_with`,
  `assert_called_with`, `assert_called`, `assert_not_called`) →
  side-effect/call oracle
- `unittest.mock` patches recognised syntactically as call-context only

Related-test heuristics mirror the Rust and TypeScript adapters: changed
owner name match, import-reference match, file-path proximity, and
syntactic call proximity. Direct owner calls must be token-aware. Module
import aliases may match attribute calls such as `pricing.apply_discount(...)`;
arbitrary object method calls must not be treated as related to a top-level
function owner unless the changed owner is itself a method or class method.

## Probe Facts

Probes the adapter must generate (syntax-first):

- predicate probes for changed `if`/`elif`/conditional-expression
  boundary conditions
- return-value probes for changed `return` and final-expression shapes
- error-path probes for changed `raise` statements and `try`/`except`
  shapes
- field probes for changed attribute assignments
- call probes for changed function and method calls including argument
  changes
- mock-interaction probes for call surfaces resolved through a syntactic
  `mock.Mock()` / `MagicMock()` initializer

When the adapter cannot classify, it emits one of the `static_limit_kind`
values defined in RIPR-SPEC-0026:

- `dynamic_dispatch` (e.g., `getattr(obj, name)(...)` or an indexed call such as `dict[key]`)
- `metaprogramming` (e.g., metaclass usage, `__getattr__` indirection)
- `missing_import_graph` (the symbol is imported from a module the
  adapter cannot resolve syntactically)
- `decorator_indirection` (the decorator changes the call semantics in a
  way the syntax-first adapter cannot follow)
- `mocked_module` (e.g., `@patch(...)` or `monkeypatch.setattr(...)`
  observed at the related-test call site)
- `unsupported_syntax`

## Required Evidence

The Python preview contract is supported only when the implementation
can show:

- a fixture corpus pinning at least one example per owner kind above
- a fixture corpus pinning at least one example per oracle kind above
- a fixture corpus pinning at least one example per probe kind above
- a fixture corpus pinning at least one example per `static_limit_kind`
- fixtures cover plain `def`, `async def`, classes, methods, decorated
  methods, and module-level fixtures
- a fixture proving `pytest.raises` and `self.assertRaises` are
  recognised as error-path oracles
- a fixture proving `mock.assert_called*` is recognised as a
  side-effect oracle
- a fixture covering parametrized `pytest` cases
- generated CI fixtures cover Python preview output visible only when
  `[languages]` declares `python`
- LSP protocol smoke covers a Python seam diagnostic, hover, code
  action, and evidence packet
- VS Code e2e smoke covers opening a Python file when the adapter is
  enabled
- `cargo xtask dogfood` records a checked Python preview receipt
- the capability matrix gains `Python preview static facts` at `alpha`,
  marked preview, with metrics from RIPR-SPEC-0026 plus Python-specific
  counts where the language adds vocabulary

## Non-Goals

- No type checking (`mypy`, `pyright`, `pytype`).
- No `pyproject.toml`, `setup.py`, or `requirements.txt` parsing.
- No virtualenv resolution or installed-package introspection.
- No runtime test runner integration beyond syntax pattern recognition.
- No `@dataclass` semantic expansion beyond detecting the syntactic
  shape.
- No mocking framework introspection beyond syntactic recognition.
- No automatic source edits, generated tests, or provider calls.
- No claims of parity with Rust evidence.

## Acceptance Examples

Function with boundary gap:

```python
def apply_discount(amount: float, threshold: float) -> float:
    if amount >= threshold:
        return amount * 0.9
    return amount
```

Existing tests:

```python
def test_discount_applies_above_threshold():
    assert apply_discount(100, 50) == 90

def test_no_discount_below_threshold():
    assert apply_discount(10, 50) == 10
```

Expected static evidence:

- owner: `apply_discount` (`function`)
- predicate probe: `amount >= threshold`
- oracle: `exact-value` via `assert ... == 90` and `assert ... == 10`
- missing discriminator: `amount == threshold`
- finding emits `language = "python"`,
  `language_status = "preview"`, `owner_kind = "function"`

Error path with `pytest.raises`:

```python
def test_rejects_negative_amount():
    with pytest.raises(ValueError):
        apply_discount(-1, 50)
```

Expected static evidence:

- oracle: `error-path`
- the `pytest.raises` context manager is recorded as the discriminator
  surface

Decorator indirection limit:

```python
@retry(times=3)
def fetch_total(client):
    return client.get_total()
```

Expected static evidence:

- probe emits `static_limit_kind = "decorator_indirection"`; finding
  stays conservative.

## Test Mapping

Follow-up fixtures and tests cover the owner, test, assertion, related
test, probe, and static-limit cases listed under Required Evidence, plus
generated CI behavior and LSP smoke coverage.

## Implementation Mapping

Follow-up implementation belongs to Campaign 27 work item
`analysis/python-preview-adapter`. The boundary, router, repo config,
and additive output metadata land first under RIPR-SPEC-0026 work items.
This spec PR records the per-language contract; no analyzer behavior
changes in the spec PR.

## Metrics

In addition to the cross-language metrics in RIPR-SPEC-0026, the Python
adapter contributes:

- `language_adapter_python_findings_preview`
- `language_adapter_python_owner_function`
- `language_adapter_python_owner_method`
- `language_adapter_python_owner_class_method`
- `language_adapter_python_owner_module_function`
- `language_adapter_python_oracle_exact_value`
- `language_adapter_python_oracle_error_path`
- `language_adapter_python_oracle_side_effect`
- `language_adapter_python_oracle_smoke`
- `language_adapter_python_oracle_broad_type`
- `language_adapter_python_static_limit_dynamic_dispatch`
- `language_adapter_python_static_limit_decorator_indirection`
- `language_adapter_python_static_limit_missing_import_graph`
- `language_adapter_python_static_limit_metaprogramming`
- `language_adapter_python_static_limit_mocked_module`
- `language_adapter_python_static_limit_unsupported_syntax`
