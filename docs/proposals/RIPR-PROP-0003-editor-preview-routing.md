# RIPR-PROP-0003: Editor Preview Routing

Status: proposed

Owner: Lane 3 - Editor / LSP UX

Created: 2026-05-12

Target campaign: Campaign 27, Language Adapter Preview

Linked specs:

- [RIPR-SPEC-0036: Editor preview routing](../specs/RIPR-SPEC-0036-editor-preview-routing.md).
- [RIPR-SPEC-0037: Editor preview static-limit projection](../specs/RIPR-SPEC-0037-editor-preview-static-limit-projection.md).

Linked ADRs:

- [ADR-0011: Editor preview routing is projection-only](../adr/0011-editor-preview-routing-is-projection-only.md).

Linked issues and work items:

- `analysis/python-preview-adapter` (#771).
- `lsp/editor-language-routing` (#772).
- `static_limit_kind` follow-up (#807).
- Policy-readiness static-limit consumer watchpoint (#814).
- [Campaign 27 Lane 3 editor preview routing plan](../../plans/campaign-27/lane3-editor-preview-routing.md).

## Problem

The Rust saved-workspace editor cockpit is already useful: diagnostics,
hover evidence, code actions, context packets, status, related-test opening,
and copyable packet, brief, after-snapshot, verify, and receipt commands
turn static RIPR evidence into one focused local test-intent loop.

Campaign 27 adds preview TypeScript, JavaScript, and Python evidence. That
evidence is valuable only if the editor can project it without making
syntax-first preview findings look like mature Rust evidence. A preview
finding that hides its static limits can cause a human or coding agent to
over-trust a weak seam. A preview finding that uses a separate editor flow
can drift away from the cockpit users already understand.

Lane 3 needs a narrow editor plan: keep Rust behavior unchanged by default,
route only explicitly enabled preview languages, and make preview status and
static limits visible before any suggested action.

## Users and surfaces

- Rust users who must see no default editor behavior change.
- TypeScript, JavaScript, and Python users who opt in to advisory preview
  evidence.
- Coding agents that need bounded packet, brief, verify, receipt, and refresh
  commands rather than speculative editor actions.
- Reviewers and operators who need proof that editor output is projection-only
  and fail-closed on stale, malformed, or wrong-workspace artifacts.
- Lane 3 maintainers who need a clear boundary between editor projection and
  analyzer, policy, CI, or runtime-mutation work.

## Success criteria

- Rust editor behavior is unchanged by default.
- Preview languages are opt-in through repo configuration.
- Preview diagnostics, hover, status, and supported actions visibly label
  preview evidence.
- Static limits appear before suggested action language in hover and status.
- Wrong-root, stale, missing, and malformed artifacts fail closed.
- The editor consumes saved-workspace artifacts and adapter output only; it
  does not run hidden analysis or mutate code.
- LSP tests, VS Code e2e, editor workflow fixtures, and
  `cargo xtask lsp-cockpit-report` prove the path before closeout.

## Proposed shape

Use the existing cockpit for preview languages instead of building a separate
preview UI. When preview adapters emit editor-projectable artifacts and repo
configuration enables the language, Lane 3 extends the VS Code activation and
LSP selectors for TypeScript, TSX, JavaScript, JSX, and Python. The saved
workspace path then routes diagnostics, hover, status, and bounded actions
through the same projection model used by Rust.

Every preview projection carries the boundary in the user's path:

```text
Language: TypeScript / JavaScript / Python
Status: preview
Evidence: syntax-first
Static limit: <kind or stable text>
Action: advisory only, when evidence supports a bounded next step
```

Rust stays the default stable editor path. Preview routing starts only after
TypeScript and Python preview outputs provide the fields the editor can
project without guessing.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Do nothing in the editor. | Preview evidence remains harder to act on at the point of coding, so agents and humans lose the saved-workspace packet loop. |
| Enable all preview selectors by default. | Too noisy for mixed workspaces and too easy to read as Rust-level maturity. |
| Add a separate preview-language editor UI. | Splits the cockpit, doubles command behavior, and creates avoidable drift from Rust. |
| Parse static-limit prose into action semantics. | Brittle until a structured field such as `static_limit_kind` exists; prose can be displayed as evidence but must not drive behavior. |
| Run analyzer, provider, generator, or mutation workflows from the editor. | Violates the projection-only boundary and makes editor behavior harder to audit. |

## Behavior specs to create or update

- `RIPR-SPEC-0036`: editor preview routing contract covering selectors,
  opt-in language routing, Rust default preservation, and fail-closed states.
- `RIPR-SPEC-0037`: preview static-limit projection contract covering
  preview labels, hover/status ordering, static-limit display, and bounded
  action language.

## Architecture decisions needed

- ADR-0011 should record that editor preview routing is projection-only. The
  editor consumes existing artifacts and saved-workspace analysis; it is not a
  second analyzer, policy engine, test generator, source editor, runtime
  mutation runner, or provider integration point.

## Implementation campaign shape

Campaign 27 already owns the language-adapter preview. Lane 3 should add its
editor source-of-truth stack first, then wait for the Python preview adapter
dependency before behavior changes.

1. `docs(lane3): define editor preview routing source-of-truth stack`.
2. `docs(proposal): add Lane 3 editor preview routing proposal`.
3. `docs(spec): add editor preview routing contract`.
4. `docs(spec): add preview static-limit projection contract`.
5. `docs(adr): editor preview routing is projection-only`.
6. `plans(c27): add Lane 3 editor preview routing implementation plan`.
7. `test(lsp): preserve Rust routing contract`.
8. `lsp(language): add opt-in editor language routing`.
9. `lsp(language): surface preview labels and static limits`.
10. `fixtures: add preview editor workflow fixtures`.
11. `test(vscode): smoke preview saved-workspace routing`.
12. `docs(editor): document preview-language workflow`.
13. `campaign(lane3): close editor preview routing`.

## Evidence plan

- Rust default LSP tests pin diagnostics, hover, actions, status, and command
  payload behavior before preview selectors land.
- Preview editor workflow fixtures cover Rust default, TypeScript preview,
  Python preview, mixed-language opt-in, and preview-disabled workspaces.
- Hover and status fixtures show preview labels and static limits before
  suggested action language.
- VS Code e2e proves extension activation, selectors, diagnostics, hover,
  status, and bounded actions through the packaged extension path.
- `cargo xtask lsp-cockpit-report` proves the saved-workspace cockpit
  contract after preview routing lands.
- Docs explain preview status, syntax-first evidence, static limits, and the
  no-edit/no-generation/no-mutation editor boundary.

## Risks

- Preview findings may look as mature as Rust findings. Mitigation: label
  every preview projection and show static limits before action language.
- Rust behavior may drift while adding selectors. Mitigation: land Rust
  routing-contract tests before preview behavior.
- Mixed-language workspaces may route evidence to the wrong file or owner.
  Mitigation: rely on adapter-produced stable identity fields and fixture-pin
  mixed-language routing.
- Static-limit prose may become an implicit behavior contract. Mitigation:
  display prose as evidence only until structured `static_limit_kind` exists.
- Editor scope may expand into analysis, generation, policy, or runtime
  execution. Mitigation: record the projection-only ADR before implementation.

## Non-goals

- No source edits.
- No generated tests.
- No provider calls.
- No runtime mutation execution.
- No policy, gate, or default-blocking behavior.
- No hidden analysis reruns from the editor.
- No CodeLens, inlay hints, semantic tokens, inline patches, or unsaved-buffer
  overlays.
- No separate preview editor command model.
- No runtime adequacy claims for preview languages.

## Exit criteria

This proposal can move to `accepted` when:

- the editor preview routing and static-limit projection specs are merged;
- the projection-only ADR is merged;
- the Campaign 27 Lane 3 implementation plan is merged;
- TypeScript, JavaScript, and Python preview routing is opt-in and
  fixture-pinned;
- Rust saved-workspace editor behavior remains unchanged by default;
- preview labels and static limits are visible in diagnostics, hover, status,
  and supported actions where applicable;
- VS Code e2e and `cargo xtask lsp-cockpit-report` prove the real editor path;
- user-facing editor docs explain preview limits and non-goals;
- a closeout handoff records proof, remaining limitations, and future editor
  campaigns.
