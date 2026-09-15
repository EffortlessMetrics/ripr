# RIPR-SPEC-0172: cross-crate test relation through dependency edges

Status: proposed

Owner:

Created: 2026-09-01

Linked proposal:

Linked ADRs:

Linked plan:

Linked issues:

- #2972 (slice #2664-B: allow cross-crate test match via dependency edge)
- #2664 (cross-crate test discovery is same-package-only)
- #2971 (slice #2664-A: the ambiguous-name fail-closed rule this admit extends)

Linked PRs:

Support-tier impact:

- No tier change. The admit widens which captured tests are reported as
  related evidence for an already-analyzed probe; it adds no runtime
  evidence, no harness execution, and no new analysis surface.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md) remains the
  tier authority.

Policy impact:

## Problem

The #2971 rule filters cross-crate tests whose bare call matches an owner
name that is ambiguous across crates, because name similarity alone cannot
establish identity — that is the token-coincidence false-`related` family.
The filter is correct, but it also discards genuine evidence: in the
standard workspace layout, `crate_c`'s integration tests exercise
`crate_a`'s API through a declared path dependency, and changing `crate_a`
reports no related test even though a strong oracle exists one crate over.

The captured path-dependency graph (#2665 slices A/B) carries exactly the
missing identity evidence: which packages the calling test's package can
name imports from, under which declared dependency name, in which Cargo
section.

## Behavior

When a probe's owner name is ambiguous across crates, a cross-crate test
may be admitted as related evidence only if every boundary below holds;
each boundary fails closed (under-emit is the contract for relation
credit):

- the `calls_owner` strong signal stays mandatory; an edge never admits on
  name or token similarity alone, and no other weak signal can ride
  through it;
- the workspace index must be complete (the same precondition as the
  #2971 uniqueness bypass) and the path-dependency adjacency must report
  a `Complete` capture: `limited` (partial edge inventory) and
  `unavailable` admit nothing, never a blanket admit;
- the calling test's file and the owner's file must attribute to their
  nearest discovered manifest directory (the longest-prefix rule over the
  shared manifest inventory), and an unattributable side admits nothing;
  nested packages resolve to the nested manifest, not the first `crates/`
  segment;
- no same-named definition may be attributed to the calling test's own
  package (a local shadow), no second same-named definition may itself be
  callable from the test's package through a captured callable edge (a
  competing candidate), and an unattributable same-named definition is
  never refutable;
- exactly one direct forward declaration from the test's manifest to the
  owner's manifest participates, in a callable section: normal or dev
  dependencies (dev dependencies are callable from the declaring
  package's test targets). Reverse edges and build-only edges admit
  nothing — a bare call binds only names the declaring crate's own
  dependency declarations provide;
- call identity comes from the parser-backed captured `calls` facts only:
  a `score(` occurrence inside a comment or a string literal fabricates
  no call, and a call the extractor could not capture stays unadmitted.
  A method-shaped call (`receiver.owner(`) resolves on the receiver's
  type, not the calling crate's scope, and admits nothing;
- the captured call must be attributable to the admitted dependency: a
  call qualified through the edge's declared dependency name
  (`dependency_name::owner(`) admits without an import; otherwise a bare
  call admits only when the comment- and string-stripped **file source**
  imports the owner from that dependency name at file level (`use
  dependency_name::...owner;`, nested paths and brace lists naming the
  owner as a whole item). Imports inside nested modules are invisible to
  tests outside them and are not credited. Aliased (`as`) and glob (`*`)
  imports are refused, brace shapes the conservative parser cannot read
  fail closed, and an import naming a different dependency defeats the
  admit;
- a `let` binding of the owner name in the test body (invisible to the
  function index) defeats the admit unconditionally.

Everything else about relation reporting — the reason/confidence tags,
ordering, and the shared rendering surfaces — is unchanged.

## Non-Goals

No transitive reach: a dependency-of-a-dependency never admits. No
weakening of the #2971 uniqueness bypass or the same-package prefix
guard. No mutation execution, no coverage claims, and no vocabulary
beyond the static finding contract. No corpus or golden re-bless is
implied by the admit alone: goldens change only when a fixture genuinely
pairs an ambiguous cross-crate owner with an import-evidenced dependent
call.

## Required Evidence

- a positive per callable section: a normal-dependency admit and a
  dev-dependency admit, each with the `use` import present;
- a positive for the qualified-call form admitting with no import;
- positives for the nested `use` path and the brace-list import;
- negative controls proving each boundary: reverse edge, build-only
  edge, no edge to the owner's crate, nested wrong-owner identity,
  import from a different dependency, aliased import, local `let`
  binding, method-only call form, comment-only call occurrence,
  string-literal-only call occurrence, competing edge-connected
  candidate, local same-named shadow, unattributable same-named
  definition, `limited` graph, and `unavailable` graph;
- unchanged pins for the #2971 rule: the same-package collision filter,
  the unique-owner cross-crate retention, and the incomplete-index
  fail-closed behavior.

## Acceptance Examples

- `crate_a` and `crate_b` both define `score`; `crate_c` declares
  `crate_a = { path = "../crate_a" }` and its test says
  `use crate_a::score; score(7)`: a probe on `crate_a::score` reports
  the `crate_c` test as related evidence; the same probe on
  `crate_b::score` does not.
- The same `crate_c` test importing `other_dep::score` instead: neither
  owner is credited.
- The same shape with `score(7)` appearing only in a `//` comment or a
  string literal: not admitted, even with the import present.
- Owner in the nested package `crates/outer/inner`, `crate_c` depending
  only on `outer`: not admitted; an edge to `inner` admits.
- A workspace with no captured edge between the packages, or a graph
  that is `limited` or `unavailable`: the #2971 filter stands.

## Test Mapping

- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_ambiguous_owner_when_cross_crate_dependent_test_calls_owner_then_edge_admits`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_forward_dev_dependency_edge_when_cross_crate_test_calls_owner_then_admits`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_nested_use_path_import_when_cross_crate_test_calls_owner_then_admits`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_brace_list_use_import_when_cross_crate_test_calls_owner_then_admits`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_qualified_call_through_dependency_name_when_no_import_exists_then_admits`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_ambiguous_owner_when_test_crate_has_no_edge_to_owner_crate_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_reverse_dependency_edge_when_cross_crate_test_calls_owner_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_build_only_dependency_edge_when_cross_crate_test_calls_owner_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_import_from_another_dependency_when_cross_crate_test_calls_owner_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_aliased_import_when_cross_crate_test_calls_owner_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_comment_only_call_evidence_when_cross_crate_test_calls_owner_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_string_literal_call_evidence_when_cross_crate_test_calls_owner_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_local_let_binding_when_cross_crate_test_calls_owner_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_method_call_form_when_cross_crate_test_calls_owner_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_local_same_name_definition_when_cross_crate_test_calls_owner_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_competing_edge_connected_candidate_when_cross_crate_test_calls_owner_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_unattributed_same_name_definition_when_cross_crate_test_calls_owner_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_nested_package_owner_when_test_depends_only_on_outer_crate_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_nested_package_owner_when_test_depends_on_the_nested_crate_then_admits`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_custom_cargo_test_target_path_when_edge_connects_then_admits`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_limited_graph_when_cross_crate_test_calls_ambiguous_owner_then_stays_filtered`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_unavailable_graph_when_cross_crate_test_calls_ambiguous_owner_then_stays_filtered`
- `crates/ripr/src/analysis/workspace/path_dependencies.rs::tests::forward_dependency_declarations_keep_section_and_declared_name`

## Implementation Mapping

- `crates/ripr/src/analysis/classify/related_tests.rs` —
  `DependencyEdgeContext`, `dependency_edge_admits_owner_call`,
  `has_callable_forward_dependency`, `imports_owner_from_dependency`,
  `use_path_names_owner`, `nearest_manifest_identity`,
  `body_binds_owner_name`, `strip_comments_and_strings`
- `crates/ripr/src/analysis/workspace/path_dependencies.rs` —
  `PathDependencyAdjacency::forward_dependency_declarations`
  (section-and-name declarations per pair)
- `crates/ripr/src/analysis/classifier.rs` — `classify_probe` threading
- `crates/ripr/src/analysis/language/rust.rs` — per-pass context
  construction (diff mode: whole-workspace index only; repo mode:
  always)

## Metrics

- unit_test_pass_rate
- golden_fixture_pass_rate
