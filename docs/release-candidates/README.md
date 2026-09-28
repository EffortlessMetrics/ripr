# Release-candidate artifact registry

<!-- Generated from index.json by `cargo xtask check-release-targets`. Do not edit by hand. -->

[`index.json`](index.json) is the only lifecycle authority for the artifacts in this directory. This page is a projection of it and cannot strengthen any state.

An artifact is current authority only with a registered row, a matching raw-byte SHA-256, a lifecycle state permitted for the requested operation, and every state-specific identity. A filename, version string, "hard cut" wording, issue closure, or presence in this directory confers no authority. `historical_evidence_only` artifacts may be cited as history but satisfy no selection, cut, qualification, source-sync, or publication prerequisite. Unregistered files fail `cargo xtask check-release-targets`.

## 0.11.0

| Artifact | State | Authority | Successor | SHA-256 |
|---|---|---|---|---|
| [`0.11.0-hard-cut.json`](0.11.0-hard-cut.json) | `historical_evidence_only` | #2893 | [`0.11.0-live-head-selection.json`](0.11.0-live-head-selection.json) | `a706647fe1642f211c33f16bd849487524ad621eea38b5cacb3159afeb40fdc7` |
| [`0.11.0-hard-cut.md`](0.11.0-hard-cut.md) | `historical_evidence_only` | #2893 | projection of [`0.11.0-hard-cut.json`](0.11.0-hard-cut.json) | `6f09c026e1e649958eff244349e4b12e624baae803dcbbc4e58518ac3b904ae5` |
| [`0.11.0-live-head-selection.json`](0.11.0-live-head-selection.json) | `active_selection_template` | #2379 | - | `dcdc8a100dd825be5b76e7c5485c476db617116d1c2e3036322afec486577868` |
| [`0.11.0-live-head-selection.md`](0.11.0-live-head-selection.md) | `active_selection_template` | #2379 | projection of [`0.11.0-live-head-selection.json`](0.11.0-live-head-selection.json) | `de987ff5ee054b59df9fa3bfc4545d708a6e9227e01e6283053368a4ed4de028` |
| [`0.11.0-replacement-freeze.json`](0.11.0-replacement-freeze.json) | `historical_evidence_only` | #2354 | [`0.11.0-live-head-selection.json`](0.11.0-live-head-selection.json) | `f5b59af6cb8a7062c77744102eab185e6f0c14174708510af60c0861f35b88b0` |

## Lifecycle reasons

- `0.11.0-hard-cut.json`: The August 2026 development hard cut C (b8b1c9ec78b013dfac6dcf929447839132835971) and its candidate-only C-to-T path were superseded by the #2379 live-head authority reset; the receipt is retained as audit evidence and satisfies no current release prerequisite.
- `0.11.0-live-head-selection.json`: Successor route: #1609 registers one exact immutable candidate as pinned_exact_candidate; this row then retires to historical_evidence_only with superseded_by naming that candidate, and no historical row is rewritten.
- `0.11.0-replacement-freeze.json`: The July 23 replacement-freeze candidate c86807ecdbf359594ef88c0ff38b10b446139dca (freeze/source-sync-2026-07-23) was demoted to a historical reviewed baseline after qualification exposed supplemental defects (#2354); live selection moved to #2379.
