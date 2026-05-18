# ADR 0016: Editor Adoption Assurance Is Read-Only Diagnosis

Status: proposed

Date: 2026-05-18

## Context

The editor cockpit, first-run usability slice, and first-pr bridge already let
RIPR project existing artifacts into a local repair loop. The next adoption
risk is setup ambiguity: users need to know whether the extension started, what
server is active, whether the workspace root is safe, which artifacts are
fresh, and which receipt or first-pr packet state applies.

Those questions are diagnostic and projective. Solving them by installing
binaries, mutating config, generating artifacts, running hidden analysis, or
publishing PR output would make Lane 3 the owner of release, analyzer, and PR/CI
behavior.

## Decision

Editor adoption assurance remains read-only. The editor may inspect and project
known extension/server/workspace/config/artifact state, but it must not produce
or mutate the artifacts that other lanes own.

The editor may show:

```text
extension version
resolved server path
server version
supported protocol or feature set
workspace root
config path
enabled and available languages
artifact freshness
first-pr packet state
receipt state
next safe action
```

Unsafe, stale, wrong-root, malformed, unsupported, disabled, unavailable,
ambiguous, path-unsafe, or command-unsafe states fail closed before repair
claims or first-pr claims appear.

## Alternatives Considered

| Alternative | Why rejected |
| --- | --- |
| Auto-install or replace the server from VS Code. | Release/platform owns install rails; silent replacement would make setup harder to audit. |
| Generate first-pr packets from the editor. | First-pr packet production belongs to CLI/report surfaces. The editor projects existing packets. |
| Run hidden analysis when artifacts are missing. | Hidden analysis breaks saved-workspace expectations and makes status hard to reproduce. |
| Treat multi-root ambiguity as best effort. | Wrong-root repair packets and receipts are worse than no action. |
| Add richer editor UI furniture first. | The adoption gap is diagnosis and safe action gating, not CodeLens or inline UI. |

## Consequences

- `ripr: Diagnose Setup` and `ripr: Show Status` can become the first-run
  instrument panel without taking ownership of install, analysis, policy, or
  PR/CI production.
- The editor can point to safe regeneration, refresh, or inspection commands,
  but it does not execute hidden mutations to create trust artifacts.
- Multi-root, wrong-root, stale, malformed, and incompatible states suppress
  repair actions.
- Preview adapter unavailable and preview-limited states stay advisory and do
  not imply Rust-level confidence.
- Future editor affordances still require their own campaign and must preserve
  the projection-only boundary.

## Non-goals

- No analyzer behavior changes.
- No first-pr packet producer changes.
- No generated CI summary composition.
- No PR comment publishing.
- No policy/gate/default-blocking behavior.
- No source edits, generated tests, provider/model calls, or mutation
  execution.
- No auto-install, binary download, config mutation, or hidden server
  replacement.
- No CodeLens, inlays, semantic tokens, inline patches, or unsaved-buffer
  overlays.
