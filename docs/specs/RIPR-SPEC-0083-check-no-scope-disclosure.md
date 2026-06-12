# RIPR-SPEC-0083: Check No-Scope Disclosure

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

- #1111-adjacent — Silent empty result when `ripr check` is run with no scope

Linked PRs:

- None yet

Support-tier impact:

- No tier change. This spec adds advisory disclosure output when `ripr check`
  is invoked with no analysis scope. It does not promote any feature to a
  higher support tier, does not change pass/fail authority, and does not alter
  what the analyzer classifies.
- The disclosure is additive output only. Claim boundaries remain governed by
  the canonical ledger in [support tiers](../status/SUPPORT_TIERS.md).
- Empty-result semantics remain unchanged: "No probes found" still means the
  static analyzer found no mutation exposure probes. The no-scope case is
  additionally disclosed as "nothing was analyzed."

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

When a user runs bare `ripr check` (no `--diff`, `--base`, `--files`, or
full-repo `--mode` flag), the tool analyzes nothing but still prints:

```
No diff-derived mutation exposure probes found.
```

Exit code is 0. A new user reads this as "clean." This is the cardinal
"silence reads as clean" sin for the no-scope case. `ripr doctor` recommends
`ripr check --base origin/main`, but `check` itself gives no guidance, so the
most natural first command a new user types produces a falsely-reassuring empty
result.

This extends the #1111 honesty theme (an empty result that misleads) to the
no-scope case.

## Behavior

### Trigger conditions

The disclosure fires when ALL of the following are true:

1. The CLI `check` command was invoked.
2. No explicit analysis scope was provided: none of `--diff`, `--base`, or
   `--mode` (any value) was given in the CLI arguments.
3. The result is empty (zero findings).

The guidance does NOT fire when:

- `ripr check --diff <file>` was given and that diff had 0 probes (real result).
- `ripr check --base origin/main` was given and produced 0 probes (real result).
- `ripr check --mode fast` / `--mode deep` scanned the repo and found 0
  probes (real result).

In those cases the existing "No diff-derived mutation exposure probes found."
message is honest and correct.

### Scope detection

The CLI `check()` handler tracks a `scope_explicitly_provided` boolean,
initialised to `false`. It is set to `true` when any of `--diff`, `--base`, or
`--mode` is parsed from the argv. After running the analysis, if the flag is
still `false` and the result is empty, `output.no_scope_provided` is set to
`true`. The renderers read this field.

### Human output

When `no_scope_provided` is true, the following note is appended after the
"No diff-derived mutation exposure probes found." line:

```
Note: no analysis scope was provided — `ripr check` is diff-first. Run
`ripr check --base origin/main` to analyze your changes, or
`ripr check --root . --mode fast` for a full-repo scan. An empty result here
does NOT mean your changed behavior is covered.
```

The note is omitted entirely when scope was provided (real analyzed-empty
result). The note does not change the exit code or pass/fail status.

### JSON output (`--json`)

When `no_scope_provided` is true, an additive `scope_disclosures` array is
emitted after `findings`. It is absent when `no_scope_provided` is false.
No schema version bump is required per the additive field policy in
[`docs/OUTPUT_SCHEMA.md`](../OUTPUT_SCHEMA.md).

No-scope example:

```json
"scope_disclosures": [
  {
    "scope_status": "no_scope_provided",
    "category": "no_scope_disclosure",
    "why": "no analysis scope provided; ripr check is diff-first; empty result does not mean changed behavior is covered; run ripr check --base origin/main or ripr check --root . --mode fast"
  }
]
```

When scope was provided (real analyzed-empty), `scope_disclosures` is absent.

### Non-claims

- This spec does NOT change the exit code or gate authority.
- An empty result with no-scope disclosure does NOT mean the diff is safe; it
  means nothing was analyzed.
- This spec does NOT change what the analyzer classifies.
- This spec does NOT auto-run the suggested scope for the user.

## Non-Goals

- Disclosure in SARIF, GitHub, badge, or repo-exposure output formats.
- Auto-detecting which base revision to suggest.
- Changing behavior when scope is explicitly provided (even if empty).
- Runtime mutation testing, coverage measurement, or correctness claims.

## Required Evidence

- CLI arg parse result for `--diff`, `--base`, and `--mode` flags.
- `CheckOutput.no_scope_provided: bool` field (additive, default `false`).

## Inputs

| Input | Required? | Purpose |
| --- | --- | --- |
| CLI argv `--diff`, `--base`, `--mode` presence | yes | Determines `scope_explicitly_provided` signal |
| Analysis result `findings.is_empty()` | yes | Disclosure only fires for empty results |

## Outputs

| Output | Schema impact | Notes |
| --- | --- | --- |
| Human text `Note:` line | None | Additive; absent when scope was provided; does not change exit code |
| JSON `scope_disclosures[]` | Additive field | Absent when scope was provided; no schema version bump |

## Acceptance Examples

1. **No-scope case (the bug)**: bare `ripr check` with no args → human output
   includes `Note: no analysis scope was provided` and guidance; JSON includes
   `scope_disclosures[0].scope_status == "no_scope_provided"`.
2. **Scope provided, empty**: `ripr check --base origin/main` produces 0 probes
   → existing "No diff-derived mutation exposure probes found." message only;
   NO `Note:` guidance; NO `scope_disclosures` in JSON.
3. **Scope provided via diff**: `ripr check --diff comment.diff` produces 0
   probes → same as case 2; no disclosure.
4. **Scope provided via mode**: `ripr check --mode fast` produces 0 probes →
   same as case 2; no disclosure.

## Test Mapping

- `crates/ripr/src/output/human.rs::tests::render_emits_no_scope_guidance_when_no_scope_provided_and_empty`
- `crates/ripr/src/output/human.rs::tests::render_omits_no_scope_guidance_when_scope_provided_and_empty`
- `crates/ripr/src/output/human.rs::tests::render_no_scope_guidance_uses_conservative_static_language`
- `crates/ripr/src/output/json::tests::json_render_emits_scope_disclosures_when_no_scope_provided`
- `crates/ripr/src/output/json::tests::json_render_omits_scope_disclosures_when_scope_provided`

## Implementation Mapping

- `crates/ripr/src/app.rs` — `CheckOutput::no_scope_provided` field (additive, default `false`).
- `crates/ripr/src/app/check/output_builder.rs` — sets `no_scope_provided: false` (library API always has scope).
- `crates/ripr/src/cli/commands.rs` — `scope_explicitly_provided` tracking in `check()`; sets `output.no_scope_provided = true` when no scope + empty.
- `crates/ripr/src/output/human.rs` — emits `Note:` guidance in the empty-findings branch when `no_scope_provided`.
- `crates/ripr/src/output/json/report.rs` — emits additive `scope_disclosures[]` when `no_scope_provided`.

## CI Proof

- `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` — exit 0 each.
- `cargo test -p ripr -p xtask` — all pass including the disclosure tests.
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
- `cargo xtask check-support-tiers` pass.
- Behavioral repro: `ripr check` (no args) prints `Note: no analysis scope
  was provided` in human output and `scope_disclosures` in JSON; `ripr check
  --diff <file>` with 0 probes shows NO guidance in either format.

## Metrics

- Gate: all disclosure acceptance tests pass.
- Promote to accepted when a new-user onboarding scenario confirms the empty-
  no-scope result is no longer read as "clean."
