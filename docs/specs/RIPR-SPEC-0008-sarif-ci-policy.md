# RIPR-SPEC-0008: SARIF and CI Policy

Status: proposed

## Problem

`ripr` needs a GitHub-native reporting surface for static exposure findings and
repo seam evidence before CI policy can be useful in real repositories.

JSON and human output are already stable enough for local and agent workflows,
but GitHub code scanning, review artifacts, and baseline-aware policy need a
SARIF contract with stable rule IDs, configured severity mapping, visible
suppression handling, and conservative defaults.

## Behavior

`ripr` should emit SARIF 2.1.0 for two static evidence surfaces:

- diff-scoped Findings from `ripr check --format sarif`;
- repo-scoped classified seams from `ripr check --format repo-sarif`.

The SARIF layer is a renderer over existing analyzer evidence. It must not
classify findings or seams itself.

The SARIF layer should:

- use stable rule IDs for every Finding exposure class and seam grip class;
- map `ripr.toml` configured severity into SARIF result levels;
- omit results whose configured severity is `off`;
- keep suppressed results visible when the configured severity permits them,
  using SARIF suppression metadata instead of silently deleting them;
- include stable fingerprints so baseline comparison can identify already-known
  results;
- preserve existing human, JSON, GitHub annotation, badge, LSP, and context
  schemas unless a later scoped PR explicitly changes them;
- keep CI behavior advisory by default;
- make baseline-aware blocking opt-in.

## Rule IDs

Finding rule IDs are:

| Exposure class | Rule ID |
| --- | --- |
| `exposed` | `ripr.finding.exposed` |
| `weakly_exposed` | `ripr.finding.weakly_exposed` |
| `reachable_unrevealed` | `ripr.finding.reachable_unrevealed` |
| `no_static_path` | `ripr.finding.no_static_path` |
| `infection_unknown` | `ripr.finding.infection_unknown` |
| `propagation_unknown` | `ripr.finding.propagation_unknown` |
| `static_unknown` | `ripr.finding.static_unknown` |

Seam rule IDs are:

| Seam grip class | Rule ID |
| --- | --- |
| `strongly_gripped` | `ripr.seam.strongly_gripped` |
| `weakly_gripped` | `ripr.seam.weakly_gripped` |
| `ungripped` | `ripr.seam.ungripped` |
| `reachable_unrevealed` | `ripr.seam.reachable_unrevealed` |
| `activation_unknown` | `ripr.seam.activation_unknown` |
| `propagation_unknown` | `ripr.seam.propagation_unknown` |
| `observation_unknown` | `ripr.seam.observation_unknown` |
| `discrimination_unknown` | `ripr.seam.discrimination_unknown` |
| `opaque` | `ripr.seam.opaque` |
| `intentional` | `ripr.seam.intentional` |
| `suppressed` | `ripr.seam.suppressed` |

Rule IDs are stable public integration strings. A later schema version may add
new rule IDs, but existing IDs must not be renamed.

## Severity Mapping

Configured severities come from `ripr.toml`, with built-in defaults when the
file is absent.

| `ripr` severity | SARIF result behavior |
| --- | --- |
| `warning` | emit result with `level: "warning"` |
| `info` | emit result with `level: "note"` |
| `note` | emit result with `level: "note"` |
| `off` | omit result |

`ripr` SARIF does not emit `level: "error"` in v1. Static exposure evidence is
advisory unless an explicit CI policy command treats new configured-warning
results as blocking.

Finding severities cannot be `off` in repository config today. Seam severities
can be `off`; the default seam policy omits `strongly_gripped`, `intentional`,
and `suppressed` from SARIF unless the repository explicitly configures those
classes to a visible severity.

## Result Shape

Every SARIF result should include:

- `ruleId` matching the tables above;
- `level` from configured severity;
- one primary physical location when file and line are known;
- a short message that uses static exposure vocabulary;
- `partialFingerprints.riprFingerprintV1`;
- `properties.tool = "ripr"`;
- `properties.kind = "finding"` or `"seam"`;
- stable identifiers such as `finding_id`, `probe_id`, or `seam_id` when
  available;
- class metadata such as `classification`, `probe_family`, `seam_kind`, or
  `grip_class`;
- static evidence summaries already computed by the analyzer, when available.

Finding fingerprints should prefer:

```text
rule_id | finding_id | probe_id | normalized_file | line
```

Seam fingerprints should prefer:

```text
rule_id | seam_id | normalized_file | line
```

Fallback fingerprints may use `rule_id | normalized_file | line | message`,
but only when the stable finding or seam identifier is unavailable.

## Suppressions

Suppressions are audit metadata, not silent deletion, unless configured severity
is `off`.

For v1:

- `kind = "exposure_gap"` suppressions matching a Finding ID should attach a
  SARIF `suppressions` entry to that result.
- Expired or malformed suppressions must not suppress a result.
- Seam results whose grip class is `suppressed` are omitted by default because
  `severity.seams.suppressed = "off"`. If a repository configures that class to
  `info`, `note`, or `warning`, the result should be emitted with SARIF
  suppression metadata.
- Suppression metadata should include the configured justification when SARIF
  can represent it.

The SARIF renderer must not invent suppressions. It consumes the same configured
suppression path used by badge/report policy.

## CI Policy

CI policy is opt-in and baseline-aware.

Modes:

| Mode | Behavior |
| --- | --- |
| `advisory` | Emit reports and exit successfully even when results exist. This is the default. |
| `baseline-check` | Compare current SARIF to a baseline and report new configured-warning results. |
| `fail-on-new-warning` | Compare current SARIF to a baseline and exit non-zero when new configured-warning results appear. |

The first implementation should prefer an `xtask` policy command over a public
`ripr` CLI policy surface. This keeps CI blocking behavior repository-automation
scoped until the shape has been exercised.

Baseline comparison should:

- compare unsuppressed results using `ruleId` plus `partialFingerprints`;
- treat `warning` as the default blocking threshold;
- ignore `note` results when the threshold is `warning`;
- ignore results omitted by configured `off` severity;
- report missing baseline as advisory by default unless an explicit flag says
  missing baseline is an error;
- never enable a default workflow that blocks pull requests.

## Required Evidence

SARIF and CI policy evidence should cover:

- SARIF rule IDs for all Finding exposure classes;
- SARIF rule IDs for all seam grip classes;
- configured severity mapping for Findings and seams;
- configured `off` omitting seam results;
- suppression metadata for matched exposure-gap suppressions;
- SARIF JSON validity;
- baseline comparison passing when no new configured-warning results exist;
- baseline comparison flagging new configured-warning results;
- threshold handling that ignores notes when threshold is warning;
- docs showing advisory upload and opt-in blocking.

## Non-Goals

This spec does not require:

- branch protection changes;
- default-on CI blocking;
- GitHub workflow changes in the contract PR;
- badge count remapping;
- new analyzer classifications;
- new suppression semantics beyond the existing configured suppression path;
- runtime mutation outcome language in static SARIF results.

## Acceptance Examples

### Finding SARIF uses configured severity

```text
Given ripr.toml configures weakly_exposed findings as warning,
when a weakly_exposed Finding is rendered as SARIF,
then the result uses ruleId ripr.finding.weakly_exposed and level warning.
```

### Seam SARIF omits off severity

```text
Given ripr.toml configures strongly_gripped seams as off,
when repo seam SARIF is rendered,
then strongly_gripped seam results are omitted.
```

### Baseline policy is opt-in

```text
Given a SARIF report with warning-level results,
when no baseline policy mode is requested,
then CI artifact generation remains advisory and does not block by default.
```

### Baseline policy flags new warnings

```text
Given a baseline SARIF report and a current SARIF report,
when the current report has a warning-level fingerprint absent from the
baseline,
then fail-on-new-warning mode reports the new result and exits non-zero.
```

## Test Mapping

Implemented renderer tests:

- `crates/ripr/src/output/sarif.rs::tests::sarif_renders_findings_with_stable_rule_ids`
- `crates/ripr/src/output/sarif.rs::tests::sarif_renders_seams_with_stable_rule_ids`
- `crates/ripr/src/output/sarif.rs::tests::sarif_uses_configured_finding_severity`
- `crates/ripr/src/output/sarif.rs::tests::sarif_uses_configured_seam_severity`
- `crates/ripr/src/output/sarif.rs::tests::sarif_omits_off_seam_class`
- `crates/ripr/src/output/sarif.rs::tests::sarif_attaches_suppression_metadata`
- `crates/ripr/src/output/sarif.rs::tests::sarif_projects_python_repair_card_properties`
- `crates/ripr/src/output/sarif.rs::tests::sarif_projects_python_static_limit_no_action_properties`

Implemented policy tests:

- `xtask/src/main.rs::tests::sarif_policy_passes_when_no_new_results`
- `xtask/src/main.rs::tests::sarif_policy_flags_new_warning_result`
- `xtask/src/main.rs::tests::sarif_policy_ignores_note_when_threshold_warning`
- `xtask/src/main.rs::tests::sarif_policy_missing_baseline_is_advisory_by_default`

## Implementation Mapping

Implemented renderer:

- `crates/ripr/src/output/sarif.rs` renders SARIF.
- `crates/ripr/src/output/mod.rs` exposes the private renderer to `app`.
- `crates/ripr/src/app.rs` dispatches SARIF formats without changing existing
  JSON schemas.
- `crates/ripr/src/cli/parse.rs` accepts SARIF output format names.

Implemented policy:

- `xtask/src/main.rs` owns baseline-aware policy comparison.
- `docs/OUTPUT_SCHEMA.md` and `docs/CI.md` document the emitted SARIF and
  policy usage.
- `docs/CI.md` also documents the copyable non-blocking GitHub code-scanning
  upload workflow for adopters.

## Metrics

- `sarif_results_total`
- `sarif_results_by_rule`
- `sarif_results_by_level`
- `sarif_new_warning_results`
