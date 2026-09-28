# Editor Gap Cockpit Dogfood Receipts

Date: 2026-05-14

Lane: 3, Editor / LSP UX

Work item: `dogfood/lane3-editor-gap-cockpit-receipts`

## Scope

This receipt records repo-local editor gap cockpit cases checked by
`cargo xtask dogfood`. The checks read the fixture-backed editor projections in
`fixtures/editor_gap_cockpit`, verify the compact `gap-projection.json`
contracts, and confirm the matching diagnostics, hover, code-action, and VS Code
status artifacts stay aligned.

The checks do not change analyzer behavior, editor routing, generated CI, gate
policy, branch protection, provider behavior, source files, generated tests, PR
comments, or mutation execution.

## Checked Editor Cases

| Case | Purpose |
| --- | --- |
| `rust_actionable` | Stable Rust gap projects a diagnostic, related-test action, repair packet, verify command, receipt command, and refresh. |
| `typescript_preview_static_limit` | TypeScript preview gap keeps `language_status = "preview"` and the `mocked_module` static limit before action language. |
| `python_preview_static_limit` | Python preview gap keeps `language_status = "preview"` and the `missing_import_graph` static limit before action language. |
| `disabled_language` | Disabled preview language publishes no diagnostics and leaves refresh as the safe action. |
| `wrong_root` | Wrong-root artifacts fail closed and leave refresh as the safe action. |
| `stale_artifact` | Stale artifacts fail closed and suppress repair actions until refreshed. |
| `no_actionable_gap` | No-action state publishes no diagnostics and leaves refresh/status inspection as the safe action. |

## Validation

```bash
cargo test -p xtask dogfood_editor_gap
cargo xtask dogfood
```

Result: pass.

The dogfood report writes advisory receipts to:

```text
target/ripr/reports/dogfood.json
target/ripr/reports/dogfood.md
```

The editor gap cockpit section records:

```text
target/ripr/reports/dogfood.json -> editor_gap_cockpit
target/ripr/reports/dogfood.md -> Editor Gap Cockpit Receipts
```

## Limits

- Editor behavior remains saved-workspace and projection-only.
- Rust default behavior is unchanged.
- Preview evidence remains opt-in and advisory.
- Static limits remain part of the evidence boundary.
- Wrong-root, stale, disabled, unavailable, malformed, and no-action states
  fail closed.
- No runtime adequacy claim.
- No source edits or generated tests.
- No provider calls.
- No mutation execution.
- No policy, gate, PR comment, or default CI blocking changes.
