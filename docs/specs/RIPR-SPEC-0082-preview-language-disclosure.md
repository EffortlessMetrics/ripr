# RIPR-SPEC-0082: Preview-Language Disclosure

Status: proposed

Owner: product / swarm

Created: 2026-06-12

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #1111 — Silent "No probes found" honesty gap for preview-language content

Linked PRs:

- None yet

Support-tier impact:

- No tier change. This spec adds advisory disclosure output when TypeScript,
  JavaScript, or Python files are in the analyzed scope and the repo has opted
  in to those adapters. It does not promote any preview adapter to a higher
  support tier, does not change pass/fail authority, and does not alter what
  the adapters classify.
- The disclosure is additive output only. Claim boundaries remain governed by
  the canonical ledger in [support tiers](../status/SUPPORT_TIERS.md).
- Empty-result semantics remain unchanged: "No probes found" still means the
  static analyzer found no mutation exposure probes, not that the diff is
  safe.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

When a user runs `ripr check --diff` on a diff that contains TypeScript,
JavaScript, or Python files, an empty result is ambiguous. The output says
"No diff-derived mutation exposure probes found" but gives no signal that the
diff contained preview-language files. A reader cannot distinguish between:

1. The diff contained no preview-language content, so nothing was in scope.
2. The diff contained preview-language content, but the preview adapter is
   NOT enabled (the default — no `ripr.toml`), so those files were never
   analyzed at all.
3. The diff contained preview-language content and the adapter ran, but
   produced no findings (advisory; may be incomplete parser support, etc.).

Cases (2) and (3) are a silent honesty gap. The most common is case (2): a user
running `ripr check --diff ts.diff` on a TypeScript change with the default
Rust-only configuration gets a falsely-reassuring empty result. Without
disclosure, operators incorrectly conclude the diff is fully analyzed and clean.
This is the exact #1111 repro.

## Behavior

### Detection (regardless of enablement)

The pipeline detects preview-language files in the analyzed scope by routing
every changed path (diff mode) or every workspace file (repo mode) through the
same path router (`analysis::language::route`) that dispatches to adapters.
**Detection does not require the adapter to be enabled** — it is pure path /
extension matching. The count is real, never fabricated.

A `PreviewLanguageAdvisory` is produced for each compiled preview language
(`LanguageId::is_available`) that has at least one file in scope. Only the
languages `ripr` advertises as preview — TypeScript, JavaScript, Python — are
disclosed. Non-analyzable files (`.md`, `.yaml`, etc.) never trigger an
advisory. Each advisory carries:

- `language`: stable wire string (e.g. `"typescript"`, `"python"`)
- `file_count`: number of files in scope that routed to this adapter
- `sample_paths`: up to three normalized file paths (forward-slash)
- `enabled`: whether this preview adapter was enabled (ran) for this analysis

Advisories are propagated through `AnalysisResult` → `CheckOutput`.

### Two honesty cases

1. **Adapter ENABLED + preview files in scope** (`enabled == true`) — the
   adapter ran; an empty/partial result is advisory and may be incomplete, not
   a Rust-grade clean result.
2. **Adapter NOT enabled (default) + preview files in scope** (`enabled ==
   false`) — the files were detected but NOT analyzed; the user is told their
   change was not analyzed and how to enable the adapter. This is the primary
   #1111 fix.

Pure-Rust diffs produce no advisory in either case.

### Human output

When any advisory is present, a `Note:` line is appended after the findings
(or after the "No probes found" line for empty results).

Enabled case:

```
Note: 1 Typescript(s) analyzed under preview support — preview evidence is advisory and may be incomplete. An empty result here is NOT a clean Rust-grade result.
```

Not-enabled (default) case:

```
Note: this diff contains 1 Typescript(s). The Typescript adapter is preview and not enabled, so these files were not analyzed — this is NOT a clean Rust-grade result. Enable it in ripr.toml [languages] to analyze them.
```

The note is omitted entirely for pure-Rust diffs. The note does not change
exit code or pass/fail status.

### JSON output (`--json`)

When any advisory is present, an additive `preview_languages` array is emitted
after `findings`. It is absent when the array would be empty (pure-Rust scope).
No schema version bump is required per the additive field policy in
[`docs/OUTPUT_SCHEMA.md`](../OUTPUT_SCHEMA.md).

Not-enabled (default) example:

```json
"preview_languages": [
  {
    "language": "typescript",
    "file_count": 1,
    "sample_paths": ["src/utils.ts"],
    "enabled": false,
    "analyzed": false,
    "category": "preview_language_advisory",
    "why": "preview adapter not enabled; files detected but not analyzed; empty result is not Rust-grade clean; enable in ripr.toml [languages]"
  }
]
```

Enabled example carries `"enabled": true`, `"analyzed": true`, and the
advisory-may-be-incomplete `why` string.

### Non-claims

- This spec does NOT fix gaps in TypeScript/JavaScript probe classification
  (e.g., `jest test()` / `expect()` detection). That is separate work.
- This spec does NOT change what the preview adapter analyzes or classifies.
- This spec does NOT promote TypeScript, JavaScript, or Python to a higher
  support tier.
- This spec does NOT change the exit code or the pass/fail gate authority.

## Non-Goals

- Per-file disclosure granularity beyond `sample_paths`.
- Disclosure in SARIF, GitHub, badge, or repo-exposure output formats.
- Fixing TypeScript `jest test()` / `expect()` probe detection gaps.
- Runtime mutation testing, coverage measurement, or correctness claims.
- Auto-enabling a preview adapter — the user must opt in via `ripr.toml` to
  ANALYZE preview files; disclosure only tells them analysis did not run.

## Required Evidence

- Diff changed-file list from `analysis::diff::parse_unified_diff` (diff mode),
  or workspace file walk from `analysis::workspace::discover_preview_language_files`
  (repo mode).
- Language router output from `analysis::language::route` for each path.
- `LanguageId::is_available` to restrict disclosure to compiled-in preview
  adapters.

## Inputs

| Input | Required? | Purpose |
| --- | --- | --- |
| Diff changed-file list / workspace walk | yes | Provides paths routed to preview adapters to count files and collect sample paths |
| Language router (`analysis::language::route`) | yes | Real adapter routing — same predicate used for dispatch; does NOT require the adapter to be enabled |
| `ripr.toml` `[languages] enabled` | no | Determines the `enabled` flag (which wording is used); absence (default) yields the not-enabled disclosure |

## Outputs

| Output | Schema impact | Notes |
| --- | --- | --- |
| Human text `Note:` line | None | Additive; absent for pure-Rust scope; wording depends on `enabled` |
| JSON `preview_languages[]` | Additive field | Absent when empty; no schema version bump; carries `enabled`/`analyzed` |

## Acceptance Examples

1. **Default case (#1111 repro)**: diff contains `.ts` file, NO `ripr.toml`
   (only Rust enabled) → human output includes
   `Note: this diff contains 1 Typescript(s). The Typescript adapter is preview and not enabled, so these files were not analyzed`,
   and JSON `preview_languages[0].enabled == false`, `analyzed == false`.
2. Enabled case: diff contains `.ts` file, `ripr.toml` has
   `enabled = ["typescript"]` → human output includes
   `Note: 1 Typescript(s) analyzed under preview support`, JSON
   `preview_languages[0].enabled == true`.
3. Diff contains only `.rs` files → NO `Note:` line, NO `preview_languages`
   field in JSON output (both enabled and default config).
4. Diff contains only non-analyzable files (`.md`, `.yaml`) → NO disclosure.
5. Count in `Note:` matches `file_count` in advisory, matches files routed by
   `analysis::language::route` to that adapter.

## Test Mapping

- `crates/ripr/src/output/human.rs::tests::render_emits_preview_disclosure_when_typescript_files_in_scope`
- `crates/ripr/src/output/human.rs::tests::render_emits_preview_disclosure_when_python_files_in_scope`
- `crates/ripr/src/output/human.rs::tests::render_emits_not_enabled_disclosure_for_typescript_files_when_adapter_disabled`
- `crates/ripr/src/output/human.rs::tests::render_omits_preview_disclosure_for_pure_rust_scope`
- `crates/ripr/src/output/human.rs::tests::render_preview_disclosure_count_matches_advisory_file_count`
- `crates/ripr/src/analysis/pipeline.rs::tests::diff_pipeline_emits_preview_advisory_when_ts_files_present`
- `crates/ripr/src/analysis/pipeline.rs::tests::diff_pipeline_emits_not_enabled_advisory_for_ts_diff_with_rust_only_config`
- `crates/ripr/src/analysis/pipeline.rs::tests::diff_pipeline_no_preview_advisory_for_rust_only_diff`
- `crates/ripr/src/output/diff_report.rs::tests::diff_report_includes_preview_languages_when_ts_files_in_scope`
- `crates/ripr/src/output/diff_report.rs::tests::diff_report_omits_preview_languages_for_pure_rust_scope`

## Implementation Mapping

- `crates/ripr/src/analysis/mod.rs` — `PreviewLanguageAdvisory` struct (with
  `enabled` flag), `AnalysisResult::preview_language_advisories` field.
- `crates/ripr/src/analysis/pipeline.rs` — `is_preview_language()`,
  `detect_preview_advisories()` (diff), `detect_repo_preview_advisories()`
  (repo); detection runs after the language loop, independent of enablement.
- `crates/ripr/src/analysis/workspace/discover.rs` —
  `discover_preview_language_files()` for repo-mode detection.
- `crates/ripr/src/app.rs` — `CheckOutput::preview_language_advisories` field.
- `crates/ripr/src/app/check/output_builder.rs` — maps advisory field through.
- `crates/ripr/src/output/human.rs` — `render_preview_language_advisories()`
  (two wordings by `enabled`), `capitalize_first()`.
- `crates/ripr/src/output/json/report.rs` — additive `preview_languages`
  JSON block with `enabled`/`analyzed`/`why` in `render_with_config`.
- `crates/ripr/src/output/diff_report.rs` — `DiffPreviewLanguageAdvisory`
  (with `enabled`/`analyzed`), `preview_languages` field on `DiffReport`.

## CI Proof

- `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` — exit 0 each.
- `cargo test -p ripr` — all pass including the disclosure tests.
- `cargo clippy -p ripr -p xtask --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- `cargo xtask check-static-language` pass.
- `cargo xtask check-architecture` pass.
- `cargo xtask check-no-panic-family` pass.
- `cargo xtask check-doc-artifacts` pass.
- `cargo xtask check-doc-index` pass.
- `cargo xtask check-spec-format` pass.
- `cargo xtask check-traceability` pass.
- `cargo xtask check-output-contracts` pass.
- Behavioral repro (the #1111 default case): `ripr check --diff <ts.diff>`
  with NO `ripr.toml` present prints the not-enabled disclosure in both human
  and `--json`; `ripr check --diff crates/ripr/examples/sample/example.diff`
  shows NO preview note.

## Metrics

- Gate: all disclosure acceptance tests pass, including the default
  (not-enabled) #1111 case.
- Promote to accepted when an external TypeScript repo exercises the default
  (no-config) disclosure path end-to-end and the silent empty-result gap is
  confirmed closed.
