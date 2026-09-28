# Handoff: Campaign 9 Closeout

Date: 2026-05-07
Branch / PR: `campaign-hot-sidecar-latency-closeout`
Latest merged PR: #451 `cache: index repo evidence hot path`

## Current Work Item

`campaign/hot-sidecar-latency-closeout`

Campaign 9 made the editor and operator warm paths measurable and bounded
without broadening the analyzer, output schemas, public API, SARIF, badge, VS
Code, or saved-workspace LSP surface.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Cache and refresh behavior measured before optimization | #421 and #422 added the latency audit and bounded `repo-exposure-latency-report`. |
| Warm-path reuse changed only fact layers or in-memory indexes | #423 added file-fact cache reuse; #451 indexed related-test candidates, lazy value facts, and owned classification handoff. Rendered JSON, Markdown, diagnostics, hovers, SARIF, badges, and agent packets remain uncached. |
| Pilot first-run path is bounded and actionable | #447 added budget-aware pilot behavior; #448 clarified first-screen recommendation text. |
| Closeout proof found and addressed evidence-progress opacity | #450 added opt-in evidence progress traces; #451 used that trace to reduce the evidence hot path. |
| Saved-workspace LSP contract preserved | `cargo test -p ripr lsp`, `cargo test -p ripr lsp::tests`, and `cargo xtask lsp-cockpit-report` passed on current `main` after #451 merged. |
| Warm latency proof passes on current main | A 120-second cold bounded `repo-exposure-latency-report` completed and stored the classified-seam cache; the following default 30-second run passed on JSON and Markdown cache hits. |

## PR Chain

- #421 `cache: audit current hot sidecar latency`
- #422 `cache: add repo exposure latency report`
- #423 `cache: reuse repo exposure warm path facts`
- #447 `pilot: add bounded first-run analysis`
- #448 `pilot: clarify first-screen recommendation`
- #450 `cache: trace repo exposure evidence progress`
- #451 `cache: index repo evidence hot path`
- `campaign/hot-sidecar-latency-closeout`

## Verification Run

Closeout proof before opening this PR:

```bash
cargo xtask goals next
cargo test -p ripr lsp
cargo test -p ripr lsp::tests
cargo xtask lsp-cockpit-report
cargo xtask repo-exposure-latency-report
RIPR_REPO_EXPOSURE_LATENCY_TIMEOUT_MS=120000 cargo xtask repo-exposure-latency-report
cargo xtask repo-exposure-latency-report
```

The first default latency run after concurrent main changes was a cache miss and
timed out during evidence construction. The 120-second run completed cold
compute in about 33 seconds and stored the classified-seam cache. The following
default 30-second run passed: `repo-exposure-json` about 14.6 seconds and
`repo-exposure-md` about 13.4 seconds, both cache hits.

## Next Work Item

No ready work item remains in `.ripr/goals/active.toml`.

Choose the next product campaign explicitly from the roadmap before starting
new implementation work. Do not reopen the hot-sidecar lane unless fresh
latency proof shows drift.

## What Not To Do

- Do not add new LSP features, unsaved-buffer overlays, CodeLens, inlay hints,
  or semantic tokens as part of this closeout.
- Do not cache rendered JSON, Markdown, diagnostics, hover text, SARIF, badges,
  or agent packets.
- Do not weaken the static vocabulary or present runtime mutation confirmation
  as part of static repo-exposure evidence.
- Do not claim cold repo exposure fits the default 30-second budget; the proven
  closeout path is bounded cold fill followed by default-budget warm cache hits.
