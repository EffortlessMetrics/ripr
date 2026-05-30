# Static Limits

Static limits explain what RIPR could not safely infer from syntax-first
evidence.

They are part of the evidence, not a separate verdict. A preview finding can
still point at a useful related test, assertion shape, or focused next action,
but the static limit must stay visible before anyone acts on that evidence.

## Where Static Limits Appear

Static limits can appear in:

- JSON findings as `static_limit_kind`;
- human and generated-CI summaries as static-limit text;
- VS Code diagnostics and hover/status text;
- agent packets and briefs when the selected finding carries the limit.

The stable `static_limit_kind` values are:

```text
dynamic_dispatch
metaprogramming
missing_import_graph
decorator_indirection
mocked_module
opaque_custom_assertion_helper
property_based_test
unresolved_pytest_fixture
unsupported_syntax
```

When `static_limit_kind` is absent but stable static-limit text is present,
render the text as evidence. Do not parse that prose to invent a different
action.

## How To Read Each Kind

| Kind | Plain-language meaning | What to do with it |
| --- | --- | --- |
| `dynamic_dispatch` | The call target or behavior may be selected dynamically, such as computed member calls (`obj[name]` followed by invocation) or `getattr(obj, name)(...)`. | Treat the finding as advisory. Prefer a focused test that observes the concrete runtime target or result. |
| `metaprogramming` | The code shape may change behavior through a metaprogramming mechanism, such as proxies, metaclasses, generated attributes, or similar indirection. | Keep the limit visible. Do not assume the static owner or call path is the full runtime boundary. |
| `missing_import_graph` | The preview adapter did not resolve a full project import graph. | Check whether the related test and owner are the intended files before copying a packet or opening a test. |
| `decorator_indirection` | A Python decorator may change the callable boundary before the body runs. | Treat owner/test evidence as syntax-first. Add a test around the decorated public behavior, not only the undecorated body. |
| `mocked_module` | A test replaces or mocks a module or symbol involved in the finding, such as `unittest.mock.patch(...)` or pytest `monkeypatch.setattr(...)`. | Read the mock as interaction evidence, not proof of the real dependency behavior. Keep repair routing blocked unless a separate non-mocked concrete oracle path exists. |
| `opaque_custom_assertion_helper` | A related Python test observes behavior through a custom assertion helper whose body is not inspected. | Keep the finding out of repair queues until a human or analyzer can confirm whether the helper already observes the changed discriminator. |
| `property_based_test` | A related Python test uses generated inputs, such as Hypothesis `@given(...)`, and syntax alone cannot prove which concrete examples run. | Do not assume the generated cases include the missing discriminator. Keep repair routing blocked unless that same related test also contains concrete strong oracle evidence. |
| `unresolved_pytest_fixture` | A related pytest test depends on fixture-sourced values that the preview adapter does not execute or resolve. | Do not assume the fixture supplies the missing discriminator or expected value. Keep repair routing blocked unless a separate concrete oracle path exists. |
| `unsupported_syntax` | The parser or preview adapter saw syntax outside the current preview contract. | Do not upgrade the finding into a stronger claim. Use the packet as a pointer for manual inspection. |

## What Static Limits Do Not Mean

A static limit does not mean:

- runtime mutation testing ran;
- coverage or runtime adequacy was established;
- the preview finding is policy-eligible by default;
- the editor can edit source or generate a test;
- the adapter is claiming Rust-level maturity;
- the action should change merely because a limit string appeared.

It means RIPR found enough syntax-first evidence to show a bounded preview
finding, while naming the part it could not model safely.

## Editor Read Order

For preview evidence, read editor output in this order:

```text
language
preview status
static limit
observed evidence
bounded next action
```

If the status is stale, wrong-root, malformed, disabled, or unavailable, that
state dominates. The editor should not project preview action text from stale or
untrusted artifacts.

## JSON And Integrations

Use `static_limit_kind` as a structured display and grouping field. It is safe
to group, count, and label by this value.

Do not branch code-action semantics on parsed human text. Text-only static
limits are display evidence until a structured kind exists.

For tool output contracts, see [Output schema](OUTPUT_SCHEMA.md). For the
editor projection rule, see
[RIPR-SPEC-0037: Editor preview static-limit projection](specs/RIPR-SPEC-0037-editor-preview-static-limit-projection.md).

## Related Docs

- [Language adapter preview workflow](LANGUAGE_ADAPTER_PREVIEW.md)
- [Editor extension](EDITOR_EXTENSION.md)
- [Support tiers](status/SUPPORT_TIERS.md)
- [TypeScript preview static facts](specs/RIPR-SPEC-0027-typescript-preview-static-facts.md)
- [Python preview static facts](specs/RIPR-SPEC-0028-python-preview-static-facts.md)
