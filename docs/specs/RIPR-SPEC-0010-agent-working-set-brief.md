# RIPR-SPEC-0010: Agent Working-Set Brief

Status: proposed

## Problem

`ripr` can already produce repo exposure reports, agent seam packets, targeted
test receipts, SARIF, badges, and editor actions. Those surfaces are useful
when a human has already selected a seam, but an actively coding agent usually
starts from a smaller question:

```text
I am editing these files or lines. Which seams matter right now, what test
should I imitate, what discriminator is missing, and what command proves
whether my patch improved the static evidence?
```

The existing agent seam packet is the full work order for one seam. The
working-set brief is the router that chooses the top few relevant seam packets
for the current edit. Without this contract, future CLI, LSP, or MCP surfaces
would risk dumping the full repo inventory into agent context or inventing
inconsistent ranking rules.

## Behavior

`ripr agent brief` should emit a small JSON brief that ranks the seams most
relevant to a supplied working set.

Command forms:

```bash
ripr agent brief --root . --diff change.diff --json
ripr agent brief --root . --base main --json
ripr agent brief --root . --files src/pricing.rs --json
ripr agent brief --root . --seam-id f3c9e4d21a0b7c88 --json
```

The command should:

- default to at most three seams;
- reject or clamp requests above the hard cap of ten seams;
- avoid dumping the full repo seam inventory by default;
- respect configured severity, suppressions, and explicit `off` policy;
- include `why_now` evidence explaining why each seam was selected;
- include the nearest test to imitate when the static evidence can name one;
- include missing discriminator, candidate value, and assertion-shape summaries
  when those fields exist in the underlying seam packet;
- include verification commands that help the agent capture before/after
  static evidence;
- remain advisory and static;
- avoid automatic edits, generated tests, runtime mutation execution, cache
  policy changes, LSP refresh changes, and editor protocol changes.

The brief should use already-computed repo seam evidence and agent seam packet
fields. It should not classify seams itself.

### Ranking

The first implementation should use a deterministic ranking policy:

1. `changed_line_intersects_seam`
2. `changed_owner_function`
3. `changed_test_for_related_seam`
4. `changed_assertion_near_related_test`
5. `same_file_seam`
6. `explicit_seam_id`
7. `repo_actionable_fallback`

Tie-breakers:

1. configured severity (`warning`, then `info`, then `note`);
2. grip class priority (`weakly_gripped`, `ungripped`,
   `reachable_unrevealed`, unknown-stage classes, then `opaque`);
3. related-test confidence;
4. file path;
5. line;
6. seam ID.

`--seam-id` is an explicit lookup and should return that seam first when it is
visible under current config. Other seams may be included only when the caller
also asks for more than one result.

### JSON Shape

The brief should use schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "scope": "working_set",
  "root": ".",
  "mode": "draft",
  "config": {
    "state": "loaded",
    "path": "ripr.toml",
    "fingerprint": "sha256:..."
  },
  "working_set": {
    "source": "diff",
    "files": ["src/pricing.rs"],
    "changed_lines": [
      {
        "file": "src/pricing.rs",
        "line": 88
      }
    ],
    "base": "main",
    "diff": "change.diff",
    "seam_id": null
  },
  "limits": {
    "requested": 3,
    "returned": 1,
    "default": 3,
    "hard_cap": 10
  },
  "top_seams": [
    {
      "seam_id": "f3c9e4d21a0b7c88",
      "owner": "src/pricing.rs::discounted_total",
      "seam_kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 88,
      "expression": "amount >= discount_threshold",
      "grip_class": "weakly_gripped",
      "severity": "warning",
      "headline_eligible": true,
      "why_now": {
        "reason": "changed_line_intersects_seam",
        "confidence": "high",
        "evidence": "changed line 88 intersects the seam origin"
      },
      "evidence": {
        "reach": "yes",
        "activate": "yes",
        "propagate": "yes",
        "observe": "yes",
        "discriminate": "weak"
      },
      "recommended_test": {
        "name": "discounted_total_boundary_discriminator",
        "file": "tests/pricing.rs",
        "reason": "place the new targeted test next to the nearest strong related test"
      },
      "nearest_strong_test_to_imitate": {
        "name": "below_threshold_has_no_discount",
        "file": "tests/pricing.rs",
        "line": 12,
        "oracle_kind": "exact_value",
        "oracle_strength": "strong",
        "relation_reason": "direct_owner_call",
        "relation_confidence": "high"
      },
      "candidate_values": [
        {
          "value": "discount_threshold (equality boundary)",
          "reason": "observed values do not include the equality-boundary case"
        }
      ],
      "missing_discriminators": [
        {
          "value": "discount_threshold (equality boundary)",
          "reason": "observed values do not include the equality-boundary case"
        }
      ],
      "assertion_shape": {
        "kind": "exact_return_value",
        "example": "assert_eq!(discounted_total(/* discount_threshold (equality boundary) */), /* expected */)"
      },
      "packet_ref": {
        "format": "agent-seam-packets-json",
        "seam_id": "f3c9e4d21a0b7c88"
      },
      "verification": {
        "before_snapshot_command": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json",
        "after_snapshot_command": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json",
        "receipt_command": "ripr outcome --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json",
        "suggested_test_command": "cargo test discounted_total_boundary_discriminator"
      }
    }
  ],
  "next": {
    "inspect_packet": "ripr check --root . --mode draft --format agent-seam-packets-json > target/ripr/workflow/agent-seam-packets.json",
    "verify_after_edit": "ripr outcome --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json"
  },
  "warnings": []
}
```

Field contract:

- `schema_version` — currently `"0.1"`.
- `scope` — always `"working_set"`.
- `mode` — effective analysis mode after CLI flags and repo config.
- `config.state` — `"loaded"` or `"missing"`.
- `config.path` — repo config path when loaded; otherwise `null`.
- `config.fingerprint` — stable fingerprint of effective config when
  available. It must not include config source text or secrets.
- `working_set.source` — `"diff"`, `"base"`, `"files"`, or `"seam_id"`.
- `working_set.files` — normalized repo-relative paths considered by the
  brief.
- `working_set.changed_lines` — normalized file/line records when line data is
  available. File-only requests may leave this empty.
- `working_set.base` — base ref when the caller supplied `--base`; otherwise
  `null`.
- `working_set.diff` — diff path when the caller supplied `--diff`; otherwise
  `null`.
- `working_set.seam_id` — requested seam ID when the caller supplied
  `--seam-id`; otherwise `null`.
- `limits.default` — always `3`.
- `limits.hard_cap` — always `10`.
- `top_seams[]` — ranked seam summaries. Each entry is intentionally smaller
  than a full agent seam packet.
- `top_seams[].why_now.reason` — one of the ranking reasons above.
- `top_seams[].why_now.confidence` — `high`, `medium`, `low`, or `unknown`.
- `top_seams[].packet_ref` — stable pointer to the full packet shape.
- `top_seams[].verification` — commands the agent can run to compare static
  evidence before and after a focused test.
- `warnings[]` — advisory warnings, such as hidden seams due to configured
  `off` severity, missing repo exposure artifacts, or missing line data.

## Static Examples

These examples are static contract examples, not generated fixture outputs.
They use the same vocabulary as repo exposure and agent seam packet artifacts,
but intentionally stay smaller than a full packet.

### Diff-scoped touched seam

When a diff touches the seam origin, the touched seam should rank first with a
high-confidence `changed_line_intersects_seam` reason:

```json
{
  "schema_version": "0.1",
  "working_set": {
    "source": "diff",
    "files": ["src/pricing.rs"],
    "changed_lines": [{ "file": "src/pricing.rs", "line": 88 }],
    "diff": "change.diff"
  },
  "limits": { "requested": 3, "returned": 1, "default": 3, "hard_cap": 10 },
  "top_seams": [
    {
      "seam_id": "f3c9e4d21a0b7c88",
      "owner": "src/pricing.rs::discounted_total",
      "seam_kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 88,
      "expression": "amount >= discount_threshold",
      "grip_class": "weakly_gripped",
      "severity": "warning",
      "why_now": {
        "reason": "changed_line_intersects_seam",
        "confidence": "high",
        "evidence": "changed line 88 intersects the seam origin"
      },
      "missing_discriminators": [
        {
          "value": "discount_threshold (equality boundary)",
          "reason": "observed values do not include the equality-boundary case"
        }
      ],
      "assertion_shape": {
        "kind": "exact_return_value",
        "example": "assert_eq!(discounted_total(/* boundary value */), /* expected */)"
      },
      "packet_ref": {
        "format": "agent-seam-packets-json",
        "seam_id": "f3c9e4d21a0b7c88"
      },
      "verification": {
        "after_snapshot_command": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json",
        "suggested_test_command": "cargo test discounted_total_boundary_discriminator"
      }
    }
  ],
  "warnings": []
}
```

### File-scoped capped brief

A file-only request may have many matching seams. The default brief remains
capped at three and reports that cap explicitly:

```json
{
  "schema_version": "0.1",
  "working_set": {
    "source": "files",
    "files": ["src/pricing.rs"],
    "changed_lines": []
  },
  "limits": { "requested": 3, "returned": 3, "default": 3, "hard_cap": 10 },
  "top_seams": [
    {
      "seam_id": "f3c9e4d21a0b7c88",
      "file": "src/pricing.rs",
      "line": 88,
      "grip_class": "weakly_gripped",
      "why_now": { "reason": "same_file_seam", "confidence": "medium" }
    },
    {
      "seam_id": "a4c733e1d9ef0220",
      "file": "src/pricing.rs",
      "line": 101,
      "grip_class": "ungripped",
      "why_now": { "reason": "same_file_seam", "confidence": "medium" }
    },
    {
      "seam_id": "c2f1b5d0a8ee9b41",
      "file": "src/pricing.rs",
      "line": 119,
      "grip_class": "activation_unknown",
      "why_now": { "reason": "same_file_seam", "confidence": "medium" }
    }
  ],
  "warnings": ["7 additional visible seams were omitted by the default cap"]
}
```

### Seam-ID lookup

An explicit seam lookup should return the requested visible seam first and keep
the entry small by pointing to the full agent seam packet:

```json
{
  "schema_version": "0.1",
  "working_set": {
    "source": "seam_id",
    "files": ["src/pricing.rs"],
    "changed_lines": [],
    "seam_id": "f3c9e4d21a0b7c88"
  },
  "limits": { "requested": 1, "returned": 1, "default": 3, "hard_cap": 10 },
  "top_seams": [
    {
      "seam_id": "f3c9e4d21a0b7c88",
      "file": "src/pricing.rs",
      "line": 88,
      "grip_class": "weakly_gripped",
      "why_now": { "reason": "explicit_seam_id", "confidence": "high" },
      "packet_ref": {
        "format": "agent-seam-packets-json",
        "seam_id": "f3c9e4d21a0b7c88"
      }
    }
  ],
  "warnings": []
}
```

### Configured-off or suppressed seams

When matching seams are hidden by configured `off` severity or suppressions,
the brief should omit them from `top_seams` and explain the omission without
dumping the hidden seam packet:

```json
{
  "schema_version": "0.1",
  "working_set": {
    "source": "files",
    "files": ["src/pricing.rs"],
    "changed_lines": []
  },
  "limits": { "requested": 3, "returned": 0, "default": 3, "hard_cap": 10 },
  "top_seams": [],
  "warnings": [
    "1 matching seam was hidden because configured severity is off",
    "1 matching seam was hidden by a reasoned suppression"
  ]
}
```

## Required Evidence

Each brief entry must include enough information for an agent to select a
focused next action without reading the full repo report:

- seam ID, file, line, owner, kind, expression, grip class, and severity;
- `why_now` reason and confidence;
- RIPR stage evidence summary;
- recommended test placement when available;
- nearest strong test to imitate when available;
- candidate values and missing discriminators when available;
- assertion shape when available;
- verification commands for before/after static evidence.

The brief must preserve static-output language. Runtime confirmation can be
referenced only as a separate optional calibration step, not as evidence the
brief itself produced.

## Inputs

- `--root <path>` selects the workspace root.
- `--diff <path>` selects a diff file and line-level working set.
- `--base <ref>` derives the working set from a base ref.
- `--files <path>[,<path>...]` selects a file-only working set.
- `--seam-id <id>` selects a specific seam.
- `--json` requests machine-readable output.
- A future `--max-seams <n>` may request fewer or more than the default, up to
  the hard cap.

Exactly one of `--diff`, `--base`, `--files`, or `--seam-id` should be
required.

## Outputs

- JSON on stdout for `--json`.
- A future human summary may be added after the JSON contract is implemented.
- No files are written by default.
- No source files are edited.

## Non-Goals

This spec does not require:

- code implementation in the spec PR;
- LSP protocol changes;
- cache, hot sidecar, or latency changes;
- runtime calibration fixture changes;
- release or install documentation changes;
- automatic test generation;
- automatic source edits;
- mutation-runtime execution;
- a full repo packet dump by default;
- new analyzer classifications;
- schema changes to existing check, repo-exposure, agent-seam-packet, SARIF,
  badge, LSP, or calibration outputs.

## Acceptance Examples

### Diff-scoped brief ranks touched seam first

```text
Given a diff that changes src/pricing.rs line 88,
and a weakly_gripped predicate_boundary seam starts at line 88,
when ripr agent brief --root . --diff change.diff --json runs,
then the first top_seams entry is that seam
and why_now.reason is changed_line_intersects_seam.
```

### File-only brief stays capped

```text
Given src/pricing.rs contains ten actionable seams,
when ripr agent brief --root . --files src/pricing.rs --json runs,
then the brief returns at most three seams by default
and reports limits.default = 3 and limits.hard_cap = 10.
```

### Severity off hides a seam

```text
Given repo config sets the relevant seam class severity to off,
when ripr agent brief selects a working set containing that seam,
then the seam does not appear in top_seams
and the brief may include an advisory warning that configured policy hid
matching seams.
```

### Seam lookup returns one packet pointer

```text
Given seam f3c9e4d21a0b7c88 is visible under current config,
when ripr agent brief --root . --seam-id f3c9e4d21a0b7c88 --json runs,
then the first top_seams entry has that seam ID
and packet_ref points at the full agent seam packet for the same seam.
```

### Brief does not generate edits

```text
Given an actionable seam with an assertion_shape example,
when ripr agent brief emits a JSON brief,
then it includes verification commands and assertion-shape guidance
but does not write source files or generated tests.
```

## Test Mapping

Planned tests:

- `agent_brief_ranks_changed_line_intersection_first`
- `agent_brief_ranks_explicit_seam_id_first`
- `agent_brief_ranks_changed_owner_before_same_file_fallback`
- `agent_brief_caps_default_to_three_seams`
- `agent_brief_rejects_or_clamps_above_hard_cap`
- `agent_brief_respects_configured_off_severity`
- `agent_brief_omits_suppressed_seams`
- `agent_brief_includes_config_fingerprint_without_source_text`
- `agent_brief_reuses_agent_packet_assertion_shape`
- `agent_brief_includes_verification_commands`
- `agent_brief_json_shape_is_stable`

## Implementation Mapping

The first implementation should be CLI-first and JSON-only. It should add a
thin routing layer over existing repo exposure and agent seam packet evidence,
not a new analyzer. The implementation PR should keep these seams separate so
reviewers can verify that ranking, rendering, policy filtering, and command
construction remain behavior-preserving.

### CLI parsing

Planned files:

- `crates/ripr/src/cli/parse.rs`
- `crates/ripr/src/cli/commands.rs`
- `crates/ripr/src/cli/help.rs`

Responsibilities:

- parse `ripr agent brief`;
- require exactly one of `--diff`, `--base`, `--files`, or `--seam-id`;
- accept `--json`;
- accept a future `--max-seams <n>` while enforcing `limits.hard_cap = 10`;
- keep the command JSON-only until the schema is implemented and pinned.

Parsing should not run analysis or rank seams. It should produce a typed
request for the app layer.

### App orchestration

Planned files:

- `crates/ripr/src/app.rs` or a narrow `crates/ripr/src/app/agent_brief.rs`
  module if the app layer is already split;
- existing config loading code for effective mode, suppressions, severity, and
  config fingerprint metadata.

Responsibilities:

- load repo config and explicit CLI overrides using the existing precedence
  rules;
- resolve the working set from the selected input mode;
- call existing repo seam exposure and agent seam packet paths;
- pass classified seams and packet summaries into the working-set selector;
- return a render-ready model to the output layer.

The app layer may invoke existing analysis/reporting functions. It must not
change cache invalidation, hot sidecar lifetime, latency reporting, or editor
refresh behavior.

### Working-set selector

Planned file:

- `crates/ripr/src/analysis/agent_brief.rs` or
  `crates/ripr/src/app/agent_brief.rs`, depending on whether the selector is
  kept as pure ranking logic or use-case orchestration.

Responsibilities:

- map `--diff` input to changed file/line records;
- map `--base` input to changed file/line records through existing diff
  helpers;
- map `--files` input to normalized repo-relative paths;
- map `--seam-id` input to an explicit seam selector;
- rank visible seams with the reason order from this spec;
- apply stable tie-breakers;
- cap default output at three seams and hard-cap requests at ten;
- produce warning records for capped, hidden, or line-data-limited results.

The selector should use existing seam IDs, owners, line numbers, grip classes,
related tests, observed values, missing discriminators, and packet fields. It
should not classify new seam states or infer new oracle semantics.

### Evidence and packet inputs

Existing sources:

- repo exposure / classified seam output for `seam_id`, `owner`, `seam_kind`,
  `file`, `line`, `expression`, `grip_class`, `headline_eligible`, RIPR stage
  evidence, related tests, observed values, and missing discriminators;
- agent seam packets for `recommended_test`, `nearest_strong_test_to_imitate`,
  `candidate_values`, `assertion_shape`, and full-packet references;
- repo config for severity, suppressions, and config fingerprint metadata.

If a field is not visible in existing evidence, the brief should omit that
field or report `unknown` confidence. It should not fabricate test names,
expected values, or assertion results.

### Policy and suppression filtering

Planned input:

- existing `ripr.toml` severity mapping and suppression handling.

Responsibilities:

- omit configured `off` seams from `top_seams`;
- omit suppressed seams from `top_seams` unless a future explicit mode asks for
  hidden results;
- emit advisory warning strings for hidden matching seams;
- keep suppressed/off seam packets undisclosed in the brief.

This keeps the brief aligned with SARIF and badge visibility rules while still
letting the agent understand that policy affected the routing result.

### JSON renderer

Planned file:

- `crates/ripr/src/output/agent_brief.rs`

Responsibilities:

- render schema version `0.1`;
- preserve the field names and reason vocabulary in this spec;
- render deterministic ordering;
- keep output smaller than full agent seam packets;
- render config fingerprint metadata without config source text;
- preserve static language and avoid runtime mutation result vocabulary.

The renderer should not compute ranking, policy, or evidence. It renders the
model it receives.

### Verification command construction

Planned file:

- either the app-layer brief module or a small helper near the output model.

Responsibilities:

- include a before snapshot command using `ripr check --format
  repo-exposure-json`;
- include an after snapshot command using the same mode and root;
- include a receipt command using existing `ripr outcome` behavior;
- include a focused test command only when a recommended or nearest test name is
  visible enough to make the command concrete.

Verification commands are advisory. They must not run automatically and must
not write source files.

### Implementation order

Preferred narrow PR order:

1. CLI parsing and typed request model.
2. Working-set selector over checked-in or unit-test fixtures.
3. JSON render model and renderer.
4. Config/severity/suppression filtering.
5. Verification command construction.
6. End-to-end CLI smoke tests.

Each step should preserve the hard boundaries in this spec. Implementation
should wait until the latency and cache-observation lane has settled or
explicitly cleared any shared analysis/cache surfaces.

## Metrics

- `agent_brief_requested_total`
- `agent_brief_top_seams_returned`
- `agent_brief_hidden_by_policy_total`
- `agent_brief_with_nearest_test_total`
- `agent_brief_with_assertion_shape_total`
- `agent_brief_with_verification_commands_total`
