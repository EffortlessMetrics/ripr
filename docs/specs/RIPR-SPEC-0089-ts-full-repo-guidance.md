# RIPR-SPEC-0089: TypeScript Full-Repo Scan Guidance Disclosure

Status: proposed

Owner: product / swarm

Created: 2026-06-13

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #1175 — TS adoption gap: silent empty result from repo-exposure scan on TS workspace

Linked PRs:

- None yet

Support-tier impact:

- No tier change. This spec adds a named guidance disclosure to the
  `repo-exposure-md` and `repo-exposure-json` outputs when a
  TypeScript-predominant workspace produces zero seams. It does not promote the
  TypeScript adapter to a higher support tier, does not change pass/fail
  authority, and does not claim any TypeScript exposure the scan did not produce.
- The disclosure is additive output only. Claim boundaries remain governed by
  the canonical ledger in [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

A TypeScript maintainer's first instinct for "is my repo covered?" is
`ripr check --root . --format repo-exposure-md` (or `--format repo-exposure-json`).
Because the repo-exposure scan is Rust-seam-oriented and TypeScript is analyzed
diff-first by design, the result is always a silent empty table:

```
| seams_total | 0 |
```

No signal, no guidance, no named explanation. The user receives a falsely
reassuring "nothing to look at" result with no direction to use `--diff` or
`--base origin/main`. This violates the campaign rule: a missing capability
must become a NAMED signal, never a vague empty result.

## Behavior

### Detection

When `ripr check --format repo-exposure-md` (or `--format repo-exposure-json`)
runs and BOTH of the following hold:

1. The classified seam inventory is empty (zero Rust seams).
2. TypeScript or JavaScript source files are present in the workspace (detected
   by path extension, same router as `PreviewLanguageAdvisory` detection).
3. No Rust source files are found in the workspace (the empty-seam result is
   due to absent Rust, not a Rust workspace with no probeable shapes).

…then a named guidance disclosure is emitted.

**Fail-closed invariants:**

- The guidance never claims any TypeScript seam or finding was found.
- The guidance fires ONLY when Rust is absent and TS is present with zero seams.
- A Rust workspace that happens to produce zero seams does NOT trigger the
  guidance (condition 3).
- A mixed workspace (Rust + TS with Rust seams present) does NOT trigger the
  guidance (condition 1).

### JSON output (`repo-exposure-json`)

The disclosure is an additive entry in `limitations[]`, reusing the
existing limitations vocabulary established by the `seam_limit_applied` entry
(RIPR-SPEC-0005). `run_status` remains `"complete"` — the Rust scan itself
completed normally.

```json
{
  "schema_version": "0.3",
  "scope": "repo",
  "run_status": "complete",
  "limitations": [
    {
      "category": "typescript_diff_first",
      "ts_file_count": 2,
      "repair_route": "TypeScript is analyzed diff-first; run 'ripr check --base origin/main' or '--diff <file>' to evaluate changed TypeScript behavior. Full-repo TypeScript exposure is not yet modeled (named limitation)."
    }
  ],
  "metrics": { "seams_total": 0, ... },
  "seams": []
}
```

The `ts_file_count` field is the count of TS/JS source files detected in the
workspace. It is never fabricated — it comes from `workspace::discover_preview_language_files`.

### Markdown output (`repo-exposure-md`)

A `## Limitations` section is inserted before the empty-seams message:

```markdown
## Limitations

**typescript_diff_first** (ts_file_count: 2)

TypeScript is analyzed diff-first; run 'ripr check --base origin/main' or
'--diff <file>' to evaluate changed TypeScript behavior. Full-repo TypeScript
exposure is not yet modeled (named limitation).
```

### Static-language rule

This spec uses no mutation-runtime vocabulary (the terms forbidden by the
`check-static-language` gate). The disclosure text uses: "not yet modeled",
"named limitation", "diff-first".

## Fixture

`fixtures/ts_full_repo_guidance` — a workspace with only TypeScript files and a
`package.json`, no Rust. Expected outputs pin the guidance disclosure in both
JSON and Markdown.

## Required Evidence

- `fixtures/ts_full_repo_guidance` golden fixture (check.json, human.txt, repo-exposure.json,
  repo-exposure.md) pinning both JSON and Markdown guidance outputs.
- Four unit tests in `crates/ripr/src/output/repo_exposure.rs` covering
  JSON emission, JSON alongside seam-limit, Markdown emission, and
  Markdown suppression for Rust workspace.
- `cargo xtask check-static-language` passing (no mutation-runtime vocabulary
  in the guidance text).
- Behavioral repro: running `ripr check --root fixtures/ts_full_repo_guidance/input
  --format repo-exposure-md` shows the guidance; the Rust sample workspace does not.

## Non-Goals

- Full-repo TypeScript seam analysis. The disclosure explicitly names this as
  "not yet modeled".
- Changes to the TypeScript diff adapter.
- Any change to the `run_status` field value.

## Acceptance Examples

- `ripr check --root fixtures/ts_full_repo_guidance/input --format repo-exposure-json`
  emits `limitations: [{category: "typescript_diff_first", ts_file_count: 2, repair_route: "..."}]`
  with `seams: []` and `run_status: "complete"`.
- `ripr check --root fixtures/ts_full_repo_guidance/input --format repo-exposure-md`
  emits a `## Limitations` section with `typescript_diff_first` and `ts_file_count: 2`.
- `ripr check --root crates/ripr/examples/sample --format repo-exposure-md`
  does NOT emit a `## Limitations` / `typescript_diff_first` section (Rust workspace
  with seams present).
- A Rust workspace that happens to have zero seams AND no TS files does NOT emit
  the guidance.

## Test Mapping

- `crates/ripr/src/output/repo_exposure.rs::tests::json_emits_ts_guidance_limitations_when_ts_workspace_and_empty_seams`
- `crates/ripr/src/output/repo_exposure.rs::tests::json_emits_ts_guidance_alongside_seam_limit_when_both_apply`
- `crates/ripr/src/output/repo_exposure.rs::tests::markdown_emits_ts_guidance_section_when_ts_workspace_and_empty_seams`
- `crates/ripr/src/output/repo_exposure.rs::tests::markdown_does_not_emit_ts_guidance_when_rust_seams_present`

## Implementation Mapping

- `crates/ripr/src/output/repo_exposure.rs` — `TsFullRepoGuidance` struct,
  `render_repo_exposure_json`, `write_repo_exposure_json`, `render_repo_exposure_md`
  all accept `Option<&TsFullRepoGuidance>`.
- `crates/ripr/src/output/render.rs` — `detect_ts_full_repo_guidance` (private),
  `detect_ts_full_repo_guidance_pub` (pub(crate)); wired into `RepoExposureJson`
  and `RepoExposureMd` arms of `render_check_with_config`.
- `crates/ripr/src/analysis/mod.rs` — `workspace_preview_language_files` and
  `workspace_rust_files` pub(crate) wrappers used by the detection logic.
- `crates/ripr/src/cli/commands.rs` — streaming JSON path uses
  `detect_ts_full_repo_guidance_pub` before calling `write_repo_exposure_json`.
- `crates/ripr/src/cli/commands/pilot.rs` — file-write path uses
  `detect_ts_full_repo_guidance_pub` before calling both render functions.

## CI Proof

- `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` — exit 0.
- `cargo test --workspace` — all pass including the four new disclosure tests.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo fmt --check` — clean.
- `cargo xtask check-static-language` — pass.
- `cargo xtask check-fixture-contracts` — pass (diff.patch + check.json + human.txt added).
- `cargo xtask check-traceability` — pass (RIPR-SPEC-0089 entry added).
- `cargo xtask check-doc-index` — pass (spec registered in README.md).
- `cargo xtask check-spec-format` — pass.
- Behavioral repro: `ripr check --root fixtures/ts_full_repo_guidance/input --format repo-exposure-md`
  shows `## Limitations` / `typescript_diff_first`; Rust sample workspace shows none.

## Metrics

- Gate: all four disclosure acceptance tests pass.
- The fixture `ts_full_repo_guidance` golden pins both JSON and Markdown outputs.
- Promote to accepted when a TypeScript project confirms the guidance appears in
  practice and the silent-empty-result gap is confirmed closed.
