# RIPR-PROP-0013: Editor Actionable Gap Queue

Status: accepted

Owner: Lane 3 - Editor / LSP UX

Created: 2026-05-19

Target campaign: Editor Actionable Gap Queue

Linked specs:

- `RIPR-SPEC-0055`: Editor actionable gap queue

Linked ADRs:

- `ADR-0017`: Editor gap queue is read-only

Linked work items:

- #1298 `docs(lane3): open editor actionable gap queue stack`
- #1299 `test(lsp): pin post-adoption editor contract`
- #1300 `lsp(queue): validate actionable gap packet artifacts`
- #1301 `lsp(queue): project repair queue in Show Status`
- #1302 `lsp(queue): add Copy Current Repair Packet`
- #1303 `lsp(queue): add Copy Repo Gap Map`
- #1304 `fixtures(editor): add actionable gap queue corpus`
- #1305 `test(vscode): smoke actionable gap queue`
- #1306 `docs(editor): document actionable gap queue`
- #1307 `dogfood(lane3): record actionable gap queue receipts`
- #1308 `campaign(lane3): close editor actionable gap queue`

## Problem

Lane 3 has closed the saved-workspace editor cockpit, preview-language routing,
editor gap cockpit, first-run repair usability, first-pr bridge, and adoption
assurance. The editor can explain setup, one diagnostic, receipt state, and
first-pr packet state.

The next user pain is work selection. Lane 1 now emits typed
`actionable-gaps.{json,md}` reports that describe the current safe repair
queue, but the editor does not yet project that queue. A user can still have to
traverse reports to answer:

```text
What should I work on now?
Which gap is actionable?
Which gaps are report-only or blocked?
Which packet can I hand to a human or coding agent?
What command verifies movement?
```

This campaign makes the existing editor cockpit project that queue as a
read-only, bounded, typed artifact consumer. It does not make Lane 3 an
analyzer, action-packet producer, PR/CI producer, policy surface, source editor,
test generator, provider caller, mutation runner, or gate authority.

## Users and surfaces

Users:

- human reviewers deciding which local repair to inspect first;
- coding agent operators copying a narrow work packet;
- maintainers doing a first-use repair in an unfamiliar repo;
- platform owners checking whether the editor adoption path stays safe.

Surfaces:

- `ripr: Show Status`;
- `ripr: Copy Current Repair Packet`;
- `ripr: Copy Repo Gap Map`;
- diagnostic hover and existing bounded code actions when a validated gap is
  selected;
- receipt and first-pr packet state already projected by closed Lane 3 slices;
- editor fixtures, VS Code smoke tests, docs, dogfood receipts, and
  `cargo xtask lsp-cockpit-report`.

## Success criteria

- Show Status names the top actionable gap or a clear no-action state from a
  validated `actionable-gaps` artifact.
- Show Status summarizes actionable, report-only, static-limit-only, and
  receipt states without requiring users to open the report graph first.
- `ripr: Copy Current Repair Packet` is available only when a validated
  actionable gap has a canonical identity, repair route, safe verify command,
  workspace-local paths, and fresh matching artifact state.
- `ripr: Copy Repo Gap Map` gives read-only orientation over actionable,
  report-only, static-limit, receipt, preview, and regeneration state.
- Stale, wrong-root, malformed, unsupported, unsafe, disabled, unavailable,
  receipt-mismatched, first-pr-mismatched, and actionable-packet-mismatched
  states fail closed.
- The editor does not independently re-rank upstream queue order unless a
  later spec defines local filtering and disclosure.
- No editor authority creep lands: no analyzer truth, output schema producer
  changes, PR/CI producer changes, policy decisions, gate authority, source
  edits, generated tests, provider calls, mutation execution, or new editor UI
  furniture.

## Proposed shape

Open a Lane 3 campaign named Editor Actionable Gap Queue. It consumes the
existing Lane 1 `actionable-gaps` artifacts and adds editor-side validation and
projection only:

1. define this source-of-truth stack and issue burn-down;
2. pin the closed adoption-assurance editor contract;
3. validate `target/ripr/reports/actionable-gaps.json` as a safe input seam;
4. project a small queue summary in Show Status;
5. add `ripr: Copy Current Repair Packet`;
6. add `ripr: Copy Repo Gap Map`;
7. fixture-pin success and fail-closed states;
8. smoke the real VS Code path;
9. document the workflow;
10. record dogfood receipts;
11. close with proof and remaining limits.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Keep the editor diagnostic-only. | Diagnostics are file-local, while repair selection is often repo-local. Users still have to inspect reports to find the safest next repair. |
| Recompute or re-rank actionable gaps inside Lane 3. | The queue producer belongs upstream. Lane 3 should project typed artifacts, not create analyzer truth or competing rankings. |
| Parse `actionable-gaps.md` for command and repair semantics. | Human prose is not an action contract. Typed fields decide; Markdown explains. |
| Add a dashboard-style editor surface. | The bottleneck is a bounded next action, not more UI chrome. |
| Let report-only or static-limit-only rows produce repair packets. | Interrupting users requires a repair route and verify command. Report-only state can orient, but it must not pretend to be actionable. |

## Behavior specs to create or update

- Add `RIPR-SPEC-0055`: Editor actionable gap queue.

## Architecture decisions needed

- Add `ADR-0017` to record that the editor gap queue remains read-only and
  projection-only.

## Implementation campaign shape

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md).

1. `docs/lane3-editor-actionable-gap-queue-stack`
2. `test/lsp-post-adoption-editor-contract`
3. `lsp/actionable-gap-packet-validation`
4. `lsp/show-status-repair-queue`
5. `lsp/copy-current-repair-packet`
6. `lsp/copy-repo-gap-map`
7. `fixtures/editor-actionable-gap-queue`
8. `test/vscode-actionable-gap-queue`
9. `docs/editor-actionable-gap-queue`
10. `dogfood/lane3-actionable-gap-queue-receipts`
11. `campaign/lane3-actionable-gap-queue-closeout`

## Evidence plan

- Source-of-truth docs identify the proposal, spec, ADR, plan, lane tracker,
  issue burn-down, and traceability entry.
- LSP tests pin the current post-adoption contract before queue projection
  behavior lands.
- Validation tests cover missing, malformed, unsupported-schema, wrong-root,
  stale, identity-missing, repair-route-missing, verify-command-missing,
  unsafe-command, unsafe-path, disabled-language, unavailable-adapter, receipt
  mismatch, first-pr mismatch, and actionable-packet mismatch states.
- Fixtures pin top-gap ready, multiple-gap bounded, no-action, report-only
  static limit, stale packet, wrong-root packet, malformed packet, improved
  receipt, and unchanged receipt cases.
- VS Code smoke proves Show Status, Copy Current Repair Packet, Copy Repo Gap
  Map, receipt state, suppression in unsafe states, Rust default preservation,
  and preview boundary visibility.
- Docs explain the local repair queue workflow and non-claims.
- Dogfood receipts prove actionable, no-action, static-limit-only, wrong-root,
  stale, receipt-improved, receipt-unchanged, and preview-advisory states.

## Risks

- Queue projection could become a second ranking engine. Mitigation: upstream
  queue order is authoritative unless a future spec explicitly defines and
  discloses local filtering.
- Missing typed fields could tempt prose parsing. Mitigation: Lane 3 fails
  closed until the upstream artifact supplies required typed fields.
- A repo map could look like a gate or merge signal. Mitigation: ADR-0017 and
  `RIPR-SPEC-0055` forbid gate, runtime, mutation, policy, and merge-readiness
  claims.
- Preview-language queue rows could look Rust-level. Mitigation:
  `language_status`, preview labels, and static-limit state remain visible
  before action language.
- A repair packet could point outside the workspace. Mitigation:
  workspace-root and path-safety validation gate all repair actions.

## Non-goals

- No analyzer changes.
- No `actionable-gaps` producer or output schema changes.
- No PR/CI producer changes.
- No policy, gate, badge, baseline, waiver, suppression, or default-blocking
  changes.
- No source edits, inline patches, or automatic repair application.
- No generated tests.
- No provider or model calls.
- No runtime mutation execution.
- No runtime adequacy, mutation proof, policy eligibility, gate status, or
  merge-readiness claim.
- No CodeLens, inlay hints, semantic tokens, inline patches, or
  unsaved-buffer overlays.
- No preview-language promotion to Rust-level confidence.

## Exit criteria

This proposal can move to `accepted` when:

- the post-adoption Lane 3 editor contract remains pinned;
- actionable-gap packet validation exists;
- Show Status projects queue state from validated artifacts;
- Copy Current Repair Packet exists and is bounded by typed safety fields;
- Copy Repo Gap Map exists and remains read-only orientation;
- fixtures cover success and fail-closed states;
- VS Code e2e proves the packaged extension path;
- docs explain the queue workflow;
- dogfood receipts prove actionable, no-action, static-limit-only, wrong-root,
  stale, receipt, and preview-advisory states;
- no analyzer, schema-producer, policy, PR/CI, release, source-edit,
  generated-test, provider, mutation, gate, or UI-sprawl scope landed.
