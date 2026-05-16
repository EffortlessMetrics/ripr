# Editor Gap Cockpit Workflow

Use this workflow when a RIPR diagnostic should become one safe local repair
task. The editor gap cockpit is a read-only projection over existing RIPR
artifacts:

```text
diagnostic -> hover evidence -> gap state / static limit
-> related test or repair packet -> one focused test
-> verify -> receipt -> refresh
```

Rust is the stable default path. TypeScript, JavaScript, and Python findings
are opt-in preview evidence and must stay visibly labeled as preview. The
editor does not edit source, generate tests, call providers, run mutation
testing, publish PR comments, or decide gates.

## 1. Open The Workspace

Open the repository root in VS Code. Rust/Cargo evidence is enabled by default.
For preview-language evidence, the repository must opt in with
`[languages]` in `ripr.toml`, for example:

```toml
[languages]
enabled = ["rust", "typescript", "python"]
```

The extension can activate on Rust, TypeScript, TSX, JavaScript, JSX, and
Python files, but diagnostics still come only from saved-workspace analysis for
enabled languages. If a preview adapter is not built into the current binary or
the language is disabled by config, the editor should explain that state
instead of publishing preview diagnostics.

## 2. Read Status First

Start with the status bar item or run:

```text
ripr: Show Status
```

`ripr: Show Status` is the first surface for "what now?" It names the active
workspace root, resolved server source, editor selectors, enabled languages
from the last refresh, freshness state, and the next safe action when one is
available.

Useful states include:

- actionable gap available;
- preview evidence available;
- no actionable gap;
- already observed;
- stale evidence; refresh before acting;
- language disabled by config;
- preview adapter unavailable in this binary;
- wrong-root artifact ignored;
- malformed or unsupported artifact ignored;
- server unavailable or workspace unresolved.

When the status is stale, wrong-root, malformed, disabled, or unavailable, that
state dominates. Treat repair actions as unsafe until a fresh, matching
saved-workspace artifact exists.

## 3. Inspect The Diagnostic

Open the Problems panel or inspect the editor underline. RIPR diagnostics carry
typed identity in `diagnostic.data`, such as `canonical_gap_id`, `gap_id`,
`seam_id`, `finding_id`, `language`, `language_status`, and `gap_state` when
those fields are available.

The editor uses those typed fields to find matching evidence, gap records,
repair routes, related tests, verify commands, and receipt commands. It should
not infer repair behavior from human prose.

## 4. Hover Before Acting

Hover the diagnostic before choosing an action. A useful gap hover should read
in this order:

```text
language / stable-preview status
static limits
gap state
why this matters
related test or repair target
verify command
receipt command
limits and non-claims
```

For preview-language findings, static limits appear before suggested action
language. A static limit is evidence about what RIPR could not safely infer from
syntax-first analysis; it is not a runtime verdict and does not make a preview
finding policy-eligible.

See [Static limits](STATIC_LIMITS.md) for the stable `static_limit_kind`
vocabulary and how to interpret each kind.

## 5. Choose One Bounded Action

Use only actions that appear for the current diagnostic and artifact state.
Actions are conditional:

| Action | Required evidence |
| --- | --- |
| Open related test | workspace-local, same-language related-test path |
| Copy repair packet | gap identity and repair route |
| Copy brief | enough evidence to describe one focused test |
| Copy verify command | safe command for the current workspace |
| Copy receipt command | verify-to-receipt chain exists |
| Copy static-limit note | static limit exists |
| Refresh | server available |

If an action is missing, the current artifact does not have enough trusted
evidence for that handoff. Stale or invalid artifacts should suppress repair
actions and leave refresh or status inspection as the safe next step.

## 6. Write One Focused Test

Write the test yourself or hand the copied packet to an external agent with a
narrow instruction:

```text
Write one focused test for this gap.
Use the related test or repair target when provided.
Do not edit production code unless explicitly scoped.
Run the verify command.
Return the receipt.
Stop.
```

The editor packet is a task boundary, not permission for broad refactors. It
does not call a model provider and does not authorize generated tests.

## 7. Verify And Emit A Receipt

Use the copied verify command after the test is saved. The verify step compares
static before and after artifacts. It does not run mutation testing and does
not prove runtime adequacy.

Use the copied receipt command to write the review trail for the selected gap.
The receipt records what RIPR evidence moved, which artifact versions were
compared, and what remains limited or unresolved.

## 8. Refresh The Editor

After the receipt is written, run:

```text
Refresh Analysis - Saved Workspace Check
```

or save the relevant buffer to queue the next saved-workspace refresh. The next
diagnostic/status state should show whether the gap moved, disappeared, stayed
limited by static evidence, or needs a new action.

## No-Output And Failure States

No diagnostics does not always mean no useful information exists. Use
`ripr: Show Status` to distinguish:

| State | Meaning | Safe next step |
| --- | --- | --- |
| No actionable gap | RIPR did not find a focused local repair for the current saved workspace. | Continue normal review or refresh after a relevant change. |
| Already observed | Current evidence already has the observed discriminator for this gap. | No repair packet is needed for that diagnostic. |
| Stale evidence | Saved files or artifacts changed after the last trusted analysis. | Save or refresh before acting. |
| Disabled language | Repo config excludes that language. | Enable the language intentionally, or stay Rust-only. |
| Adapter unavailable | The current binary lacks that preview adapter. | Use a build that includes the adapter, or remove the language from config. |
| Wrong-root artifact | The artifact was generated for a different workspace root. | Regenerate artifacts from the open workspace. |
| Malformed or unsupported artifact | The editor cannot trust the artifact shape. | Regenerate with a compatible RIPR version. |

When in doubt, status plus refresh is the safe path. Do not copy repair packets
from stale, wrong-root, malformed, disabled, or unavailable states.

## Preview Boundaries

Preview language evidence can be useful, but it remains syntax-first and
advisory:

- `language_status = "preview"` is part of the evidence boundary.
- `static_limit_kind` names what RIPR could not safely infer.
- Static limits appear before suggested action language.
- Preview findings do not imply Rust-level maturity.
- Preview findings do not become gate-eligible by default.

For enabling and rolling back preview languages, see
[Language adapter preview workflow](LANGUAGE_ADAPTER_PREVIEW.md).

## What The Editor Does Not Do

The gap cockpit deliberately stays local and read-only:

- no hidden analyzer reruns outside saved-workspace refresh;
- no source edits;
- no generated tests;
- no provider or model calls;
- no runtime mutation execution;
- no runtime adequacy claims;
- no policy or gate decisions;
- no PR comment publishing;
- no CodeLens, inlay hints, semantic tokens, inline patches, or unsaved-buffer
  overlays.

For a first install-to-receipt walkthrough, see
[Editor first run to first receipt](EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md). For
command inventory and server behavior, see [Editor extension](EDITOR_EXTENSION.md).
For the older seam-oriented walkthrough, see [Editor evidence workflow](EDITOR_EVIDENCE_WORKFLOW.md).
For the typed projection contract, see
[RIPR-SPEC-0047: Editor Gap Projection](specs/RIPR-SPEC-0047-editor-gap-projection.md).
