# Fixture Corpus: editor_actionable_gap_queue

Spec: RIPR-SPEC-0055

## Given

The editor consumes an existing `target/ripr/reports/actionable-gaps.json`
artifact and related receipt state.

## When

VS Code renders `ripr: Show Status`, queue actions, the current repair packet,
and the repo gap map from saved-workspace artifacts.

## Then

Each case pins one actionable-gap-queue state with:

- `vscode-status.json`;
- `lsp-code-actions.json`;
- `current-repair-packet.md`;
- `repo-gap-map.md`;
- `receipt-status.json`;
- explicit action-authority and non-claim boundaries.

| Required state | Fixture case | Required action authority |
| --- | --- | --- |
| Queue available | `setup_ok` | Repo gap map and refresh only. |
| Top actionable gap | `top_gap_ready` | Current repair packet, repo map, related test, and refresh. |
| Multiple actionable gaps | `multiple_gaps_bounded` | Top repair packet only; broader queue stays orientation. |
| No actionable gap | `no_actionable_gap` | Repo map and refresh only; repair packet suppressed. |
| Report-only/static-limit entries | `report_only_static_limit` | Repo map and refresh only; repair packet suppressed. |
| Stale actionable packet | `stale_actionable_packet` | Fail closed to refresh only. |
| Wrong-root packet | `wrong_root_packet` | Fail closed to refresh only. |
| Malformed packet | `malformed_packet` | Fail closed to refresh only. |
| Receipt improved | `receipt_improved` | Current repair packet remains bounded; movement is static/advisory. |
| Receipt unchanged | `receipt_unchanged` | Current repair packet remains bounded; unchanged movement is visible. |

## Must Not

Fixtures must not imply source edits, generated tests, provider calls, mutation
execution, runtime adequacy, policy eligibility, gate authority, PR comment
publishing, generated CI summaries, automatic repair, or merge readiness.
