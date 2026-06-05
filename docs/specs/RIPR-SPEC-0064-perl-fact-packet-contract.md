# RIPR-SPEC-0064: Perl Fact Packet Contract

Status: proposed

Owner: language-adapter / swarm

Created: 2026-06-04

Linked proposal:

- [RIPR-PROP-0018: Perl Repair Routing Lane](../proposals/RIPR-PROP-0018-perl-repair-routing-lane.md)

Linked ADRs:

- [ADR 0018: Perl LSP Fact Substrate](../adr/0018-perl-lsp-fact-substrate.md)

Linked plan:

- Perl repair routing campaign, future plan file

Linked issues:

- None yet

Linked PRs:

- None yet

Support-tier impact:

- This spec does not promote Perl. It defines a preview/advisory producer and
  consumer contract for deterministic facts. Perl facts remain ineligible for
  default gates, public badge contribution, baselines, RIPR Zero, or stable
  support claims. The canonical support-tier boundary remains
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crate, binary, dependency, parser, runtime executor, or LSP server is
  introduced by this spec.

## Problem

Perl repair routing needs a stable handoff between two tools:

- `perl-lsp` has Perl syntax, semantic, workspace, module-resolution,
  confidence, provenance, and dynamic-boundary knowledge.
- RIPR has evidence routing, canonical gaps, actionability, repair cards,
  verify commands, receipts, and user surfaces.

Without a versioned packet contract, early Perl work can drift into ad hoc JSON,
live LSP sessions, or a RIPR-owned Perl parser. Any of those paths makes
fixture replay, support-tier review, false-actionable audits, and agent packet
safety hard to evaluate.

This spec defines the first batch packet:

```text
ripr-perl-facts-v1
```

The packet is a deterministic, replayable fact export. It is not a repair-card
schema and it does not mark gaps actionable. RIPR consumes it to decide whether
a changed Perl owner becomes a canonical gap, a weak advisory finding, or a
named limitation.

## Behavior

### Producer and consumer

`perl-lsp` is the producer. It emits `ripr-perl-facts-v1` packets from a saved
workspace, diff, or explicit file set.

RIPR is the consumer. It ingests the packet through `PerlAdapter`, translates
facts into the language-neutral evidence spine, and decides:

- changed package/sub/script owner;
- reachability evidence;
- infection signal;
- propagation signal;
- revealability signal;
- canonical Perl gap ID;
- actionability or named limitation;
- repair card, verify command, agent packet, and receipt projection.

The packet must be consumable from fixtures before a live exporter exists.
RIPR must not require a live LSP protocol session, Perl runtime, package
install, or test execution to parse a packet.

### Packet status

Top-level `packet_status` is one of:

| Status | Meaning |
| --- | --- |
| `complete` | The producer exported all requested fact classes for the selected input. |
| `partial` | The producer exported some facts but also emitted limitations. |
| `unavailable` | The producer could not export Perl facts; limitations explain why. |

`partial` and `unavailable` packets are valid inputs. RIPR must fail closed:
missing required facts produce named limitations, not repair packets.

### Determinism

For the same producer version, workspace content, diff input, config input, and
export options, the packet must be byte-stable after canonical JSON
serialization except for explicitly ignored diagnostic metadata.

Determinism rules:

- Paths are repo-relative, use `/`, and are not absolute.
- IDs are derived from stable packet content, not array position.
- Arrays are sorted by stable IDs unless the field explicitly preserves source
  order.
- Ranges use one-based line and column coordinates.
- File digests are content hashes of normalized bytes.
- No host paths, usernames, temp paths, environment variables, or wall-clock
  timestamps may participate in IDs or fingerprints.
- Optional `exported_at` metadata may exist for operator diagnostics, but RIPR
  must ignore it for IDs, fingerprints, canonical gaps, and receipt movement.
- Unknown values are explicit strings or `null`; they are not omitted when the
  schema says a field is required.

### Top-level shape

The packet is JSON with this top-level shape:

```json
{
  "schema_version": "ripr-perl-facts-v1",
  "packet_id": "perl-facts:repo:7f8c...",
  "packet_status": "complete",
  "packet_fingerprint": "sha256:...",
  "producer": {
    "name": "perl-lsp",
    "version": "0.0.0",
    "capabilities": ["syntax", "workspace", "test_facts"]
  },
  "root": {
    "repo_relative": ".",
    "vcs_head": "abc123",
    "path_style": "repo_relative"
  },
  "input": {
    "base": "origin/main",
    "head": "HEAD",
    "diff_id": "sha256:...",
    "requested_fact_classes": ["owners", "tests", "oracles"]
  },
  "files": [],
  "owners": [],
  "changes": [],
  "tests": [],
  "oracles": [],
  "relations": [],
  "dynamic_boundaries": [],
  "verify_commands": [],
  "limitations": [],
  "provenance": []
}
```

Unknown future top-level fields are ignored by consumers when
`schema_version = "ripr-perl-facts-v1"` and all required v1 fields are valid.
Unknown enum values inside required v1 fields are not ignored; they make the
packet invalid unless the enum is explicitly documented as open-ended.

## Inputs

The producer may consume:

- saved workspace root;
- explicit file list;
- diff base/head or hunk list;
- repo configuration;
- Perl module/workspace metadata known to `perl-lsp`;
- test runner configuration known to `perl-lsp`;
- operator-provided export options.

The packet must record enough `input` metadata for RIPR to explain what was
analyzed, but the packet must not require RIPR to rerun `perl-lsp` to interpret
the facts.

## Outputs

### Files

Each file fact records the stable file identity:

```json
{
  "file_id": "file:lib/My/App.pm",
  "path": "lib/My/App.pm",
  "role": ["source"],
  "digest": "sha256:...",
  "package_names": ["My::App"],
  "provenance_refs": ["prov:file-index:1"]
}
```

`role` values:

- `source`
- `test`
- `helper`
- `generated`
- `config`
- `unknown`

Generated files can be visible but must not become repair-packet targets unless
a later spec explicitly allows that.

### Owners

Owners identify changed or test-reachable Perl behavior:

```json
{
  "owner_id": "perl:lib/My/App.pm::My::App::discount",
  "file_id": "file:lib/My/App.pm",
  "kind": "sub",
  "package": "My::App",
  "name": "discount",
  "range": {"start_line": 12, "start_column": 1, "end_line": 20, "end_column": 2},
  "confidence": "high",
  "provenance_refs": ["prov:syntax:discount"]
}
```

Owner `kind` values:

- `package`
- `sub`
- `method`
- `script`
- `module_initializer`
- `test_sub`
- `unknown`

Owner IDs must be language-qualified and path-qualified:

```text
perl:<normalized/path>::<package-or-script>::<owner-name>
```

If package or owner identity is unresolved, the packet must emit an owner with
`kind = "unknown"` and a limitation. RIPR must not turn that owner into an
actionable repair packet.

### Changes

Changes connect diff input to owners and behavior hints:

```json
{
  "change_id": "change:lib/My/App.pm:15:predicate",
  "file_id": "file:lib/My/App.pm",
  "owner_id": "perl:lib/My/App.pm::My::App::discount",
  "range": {"start_line": 15, "start_column": 7, "end_line": 15, "end_column": 24},
  "behavior_hint": "predicate_boundary",
  "changed_text_digest": "sha256:...",
  "provenance_refs": ["prov:diff:1"]
}
```

`behavior_hint` values:

- `predicate_boundary`
- `return_value`
- `exception_path`
- `hash_or_object_field`
- `output_observer`
- `warn_observer`
- `log_observer`
- `call_effect`
- `unknown`

`behavior_hint` is an input hint only. RIPR decides infection and canonical gap
state after evidence routing.

### Tests

Test facts describe statically known Perl tests:

```json
{
  "test_id": "test:t/app.t:test_discount_threshold",
  "file_id": "file:t/app.t",
  "framework": "Test::More",
  "name": "test_discount_threshold",
  "range": {"start_line": 4, "start_column": 1, "end_line": 12, "end_column": 2},
  "runner_hints": ["prove"],
  "confidence": "medium",
  "provenance_refs": ["prov:test-discovery:1"]
}
```

Supported first framework values:

- `Test::More`
- `Test2::V0`
- `Test2::Suite`
- `Test::Exception`
- `Test::Fatal`
- `unknown`

Runner hints:

- `prove`
- `yath`
- `carton`
- `dzil`
- `unknown`

Runner hints are advisory facts. RIPR must not assume the tool is installed
unless later command execution confirms it.

### Oracles

Oracle facts describe what a test observes:

```json
{
  "oracle_id": "oracle:t/app.t:8:eq",
  "test_id": "test:t/app.t:test_discount_threshold",
  "kind": "exact_return_assertion",
  "strength": "strong_exact",
  "target_owner_id": "perl:lib/My/App.pm::My::App::discount",
  "expression": "is($got, 10, 'discount threshold')",
  "range": {"start_line": 8, "start_column": 1, "end_line": 8, "end_column": 37},
  "confidence": "medium",
  "provenance_refs": ["prov:oracle:1"]
}
```

Oracle `kind` values:

- `exact_return_assertion`
- `predicate_boundary_assertion`
- `exception_observer`
- `hash_or_object_field_assertion`
- `output_observer`
- `warn_observer`
- `log_observer`
- `smoke_ok`
- `mention_only`
- `dies_only`
- `unknown_helper`
- `dynamic_framework_indirection`
- `unknown`

Oracle `strength` values:

- `strong_exact`
- `weak_smoke`
- `weak_broad`
- `mention_only`
- `unknown`

`ok(...)`, smoke tests, mention-only references, dies-only tests, unknown
helpers, and dynamic framework indirection must not be converted into strong
revealability by the producer. RIPR may use them as weak evidence or named
limitations.

### Relations

Relations connect changes, owners, tests, and oracles:

```json
{
  "relation_id": "relation:change:15:test:t/app.t",
  "change_id": "change:lib/My/App.pm:15:predicate",
  "owner_id": "perl:lib/My/App.pm::My::App::discount",
  "test_id": "test:t/app.t:test_discount_threshold",
  "oracle_id": "oracle:t/app.t:8:eq",
  "relation_kind": "direct_owner_call",
  "reachability_hint": "reachable",
  "confidence": "medium",
  "provenance_refs": ["prov:relation:1"]
}
```

`relation_kind` values:

- `direct_owner_call`
- `package_reference`
- `method_receiver`
- `test_name_match`
- `file_proximity`
- `helper_call`
- `fixture_setup`
- `unknown`

`reachability_hint` values:

- `reachable`
- `weakly_reachable`
- `static_unknown`

Relations are evidence inputs. RIPR decides the final reachability and
actionability state.

### Dynamic boundaries and limitations

Dynamic boundaries name cases that should fail closed:

```json
{
  "boundary_id": "limit:lib/My/App.pm:dynamic-dispatch:22",
  "kind": "dynamic_dispatch",
  "file_id": "file:lib/My/App.pm",
  "owner_id": "perl:lib/My/App.pm::My::App::discount",
  "range": {"start_line": 22, "start_column": 3, "end_line": 22, "end_column": 19},
  "confidence": "high",
  "provenance_refs": ["prov:semantic:dynamic:1"]
}
```

Boundary and limitation `kind` values:

- `dynamic_dispatch`
- `module_resolution_unknown`
- `generated_symbol`
- `role_composition`
- `monkeypatch_or_symbol_patch`
- `eval_or_string_code`
- `symbol_table_mutation`
- `framework_indirection`
- `unknown_helper`
- `unsupported_syntax`
- `missing_test_runner`
- `missing_diff_owner`
- `packet_incomplete`
- `unknown`

If a dynamic boundary affects a change, relation, owner, or oracle needed for
strict actionability, RIPR must emit a named limitation instead of a repair
packet.

### Verify commands

Verify command facts are command templates, not executed results:

```json
{
  "command_id": "verify:t/app.t:prove",
  "runner": "prove",
  "argv": ["prove", "t/app.t"],
  "scope": "file",
  "test_id": "test:t/app.t:test_discount_threshold",
  "confidence": "medium",
  "preconditions": ["prove_on_path"],
  "provenance_refs": ["prov:runner:1"]
}
```

`runner` values:

- `prove`
- `yath`
- `carton`
- `dzil`
- `unknown`

`scope` values:

- `test`
- `file`
- `suite`
- `unknown`

RIPR may project a verify command only when the command fact is present,
relevant to the selected test or file, and not blocked by a limitation. Missing
verify commands prevent repair-packet readiness.

### Provenance

Every fact that affects actionability must carry at least one provenance ref:

```json
{
  "provenance_id": "prov:syntax:discount",
  "source": "syntax",
  "file_id": "file:lib/My/App.pm",
  "range": {"start_line": 12, "start_column": 1, "end_line": 20, "end_column": 2},
  "confidence": "high"
}
```

`source` values:

- `syntax`
- `semantic`
- `workspace`
- `module_resolution`
- `test_discovery`
- `oracle_extraction`
- `runner_detection`
- `diff`
- `operator_config`
- `unknown`

Confidence values:

- `high`
- `medium`
- `low`
- `unknown`

## Strict actionability boundary

The packet never sets `gap_state = "actionable"`. RIPR sets gap state after
routing evidence through the canonical actionability contract.

A Perl gap can become actionable only when RIPR can derive all fields from the
packet and existing RIPR context:

- `canonical_gap_id`
- changed owner
- RIPR evidence
- missing discriminator
- repair kind
- target `Test::More`, `Test2`, `Test::Exception`, or `Test::Fatal` shape
- suggested test location
- verify command
- receipt command
- confidence
- evidence refs
- allowed edit boundaries
- forbidden edit boundaries
- stop-if conditions
- must-not-change constraints

If any field is missing, low-confidence, contradicted, or blocked by a dynamic
boundary, RIPR emits a weak advisory state or named limitation.

### RIPR-derived identities

Packet `owner_id` values are the canonical Perl owner identity when they are
language-qualified and path-qualified:

```text
perl:<normalized/path>::<package-or-script>::<owner-name>
```

RIPR-derived Perl gap IDs are not emitted by `perl-lsp` fact packets. RIPR
derives them only after consuming packet facts. The first fixture-backed
identity key is:

```text
owner_id + behavior_hint + missing_discriminator + assertion_shape
```

The key must not include line ranges, array positions, `change_id`, test IDs,
host paths, temp paths, or timestamps. A line move or changed `change_id` for
the same owner, behavior, discriminator, and assertion shape must keep the same
canonical gap ID. Partial packets, unknown owners, and dynamic-boundary cases
can still expose facts, but they must not become canonical actionable debt.

## Non-Goals

- No RIPR-owned Perl parser.
- No live LSP protocol dependency for `ripr check`.
- No Perl runtime, package install, or test execution dependency by default.
- No source edits or generated tests.
- No provider/model calls.
- No runtime mutation execution.
- No public badge, baseline, RIPR Zero, or default gate contribution.
- No support-tier promotion.
- No repair packet from `ok(...)`, mention-only evidence, dies-only evidence,
  unknown helpers, unresolved dynamic dispatch, generated symbols, role
  composition, monkeypatching, or framework indirection without explicit
  fixture-backed facts and strict actionability.

## Required Evidence

The first implementation slices must provide:

- parser-free packet deserialization tests for required top-level fields;
- version rejection tests for unknown `schema_version`;
- enum validation tests for required v1 enums;
- deterministic fixture tests proving stable IDs and sorted arrays;
- positive fixture with source owner, related test, exact oracle, and verify
  command;
- weak oracle fixtures for `ok(...)`, mention-only, dies-only, and unknown
  helper evidence;
- dynamic-boundary fixtures for module-resolution unknown, dynamic dispatch,
  generated symbol, monkeypatch/symbol patch, and framework indirection;
- fail-closed fixtures proving missing owner, missing verify command, missing
  provenance, and packet status `partial` do not emit repair packets.

## Acceptance Examples

### Exact return assertion

Input facts:

- changed owner `perl:lib/My/App.pm::My::App::discount`;
- change behavior hint `return_value`;
- related `Test::More` test in `t/app.t`;
- oracle `exact_return_assertion` with `strength = "strong_exact"`;
- verify command `prove t/app.t`;
- high or medium provenance for each fact.

Expected RIPR behavior:

- consume the packet without invoking `perl-lsp`;
- derive a canonical Perl gap ID if the changed return lacks the needed
  discriminator;
- emit a repair card only if all strict actionability fields are present;
- otherwise emit a named limitation that identifies the missing field.

### Smoke-only ok evidence

Input facts:

- related test reaches the changed owner;
- oracle is `smoke_ok` with `strength = "weak_smoke"`;
- verify command is present.

Expected RIPR behavior:

- keep reachability evidence;
- keep revealability weak;
- suggest exact assertion shape only when strict fields are present;
- otherwise emit advisory weak evidence with no repair packet.

### Dynamic dispatch boundary

Input facts:

- changed owner uses dynamic dispatch;
- dynamic boundary kind is `dynamic_dispatch`;
- related test exists.

Expected RIPR behavior:

- preserve the visibility of the relation;
- emit a named limitation;
- do not emit a canonical actionable gap or agent repair packet.

### Missing verify command

Input facts:

- changed owner, related test, and exact oracle exist;
- no verify command is present.

Expected RIPR behavior:

- consume evidence;
- mark repair-packet readiness false;
- explain that verify command evidence is missing.

## Test Mapping

Follow-up implementation tests should map as:

- `spec/ripr-perl-facts-v1`: packet parse, schema version, enum validation,
  deterministic sort, and fixture roundtrip.
- `analysis/perl-adapter-fixture-facts`: owner, change, test, oracle, relation,
  dynamic boundary, verify command, and provenance fixtures.
- `analysis/perl-strict-actionability`: all required actionability fields,
  missing-field fail-closed cases, and weak-oracle non-actionability.
- `output/perl-repair-cards`: projection through JSON, Markdown, SARIF, PR, CI,
  LSP, and swarm packet surfaces after the adapter exists.

## Implementation Mapping

The next PRs are:

1. Add packet model and parser tests for canned fixtures.
2. Add fixture-only `PerlAdapter` consuming the packet.
3. Add canonical Perl owner and gap ID mapping.
4. Add `perl-lsp` exporter integration.
5. Add source/test/oracle fact fixtures.
6. Add related-test linking and strict actionability.

This spec does not require all implementation slices to land in one PR.

## CI Proof

Docs-only spec PR:

- `cargo xtask check-doc-artifacts`
- `cargo xtask check-doc-index`
- `cargo xtask check-static-language`
- `cargo xtask markdown-links`
- `git diff --check`

Implementation PRs:

- focused unit tests for packet parsing and validation;
- focused fixture tests for each fact class;
- `cargo xtask check-output-contracts` when output surfaces change;
- `cargo xtask check-fixture-contracts` when fixtures/goldens change;
- `cargo xtask check-pr` before claiming a branch is review-ready.

## Metrics

The packet contract alone does not move support tier. Later metrics must report:

- packet parse success rate;
- packet invalid reason distribution;
- unsupported dynamic-boundary distribution;
- fact provenance coverage;
- verify-command availability;
- top-1/top-3 Perl repair-card precision;
- false-actionable rate;
- receipt closure or improvement rate.

Perl support remains preview/advisory until a separate support-tier PR cites
fixtures, dogfood, metrics, receipts, and rollback criteria.

## Failure Modes

- Unknown `schema_version`: reject packet with an unavailable-adapter
  diagnostic.
- Missing required top-level field: reject packet and explain the missing
  field.
- Unknown required enum value: reject packet unless the field is explicitly
  documented as open-ended.
- Missing provenance on an actionability-relevant fact: consume only as weak
  context or limitation.
- `packet_status = "partial"`: consume available facts but fail closed for
  repair packets.
- `packet_status = "unavailable"`: emit adapter unavailable state, not Perl
  repair guidance.
- Absolute paths or host-specific paths: reject packet or strip from
  user-facing output before projection.
- Dynamic boundary on required owner, relation, or oracle: emit named
  limitation, not actionable gap.
