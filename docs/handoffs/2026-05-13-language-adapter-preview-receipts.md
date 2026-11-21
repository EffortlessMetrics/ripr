# Language Adapter Preview Dogfood Receipts

Date: 2026-05-13

Campaign: 27, Language Adapter Preview

Work item: `dogfood/language-adapter-preview-receipts`

## Scope

This receipt records repo-local TypeScript and Python preview adapter cases
checked by `cargo xtask dogfood`. The checks run selected preview fixtures
through `ripr check --mode fast` and verify preview labels, structured static
limits, disabled-language behavior, and language-safe related-test routing.

The checks do not change analyzer behavior, editor routing, generated CI,
gate policy, branch protection, provider behavior, source files, generated
tests, or mutation execution.

## Checked Preview Cases

| Case | Language | Purpose |
| --- | --- | --- |
| `typescript_mocked_module_limit` | TypeScript | TypeScript preview finding keeps `language_status = "preview"` and the `mocked_module` static limit. |
| `python_missing_import_graph_limit` | Python | Python preview finding keeps `language_status = "preview"` and the `missing_import_graph` static limit. |
| `python_mixed_language_no_cross_route` | Python | A TypeScript test mention is not used as Python related-test evidence. |
| `python_disabled` | Python | Rust-default configuration does not emit disabled Python preview findings. |

## Validation

```bash
cargo xtask dogfood
```

Result: pass.

The dogfood report writes advisory receipts to:

```text
target/ripr/reports/dogfood.json
target/ripr/reports/dogfood.md
target/ripr/dogfood/language-preview/
```

## Limits

- Preview evidence remains opt-in through `[languages]`.
- Preview evidence remains advisory by default.
- Rust-default behavior is unchanged.
- Static evidence only.
- No runtime typechecker or test execution.
- No hidden analysis rerun beyond the explicit dogfood fixture commands.
- No source edits or generated tests.
- No provider calls.
- No mutation execution.
- No policy or gate semantic changes.
- No default CI blocking.
