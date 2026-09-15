# Regression Surface Mapping for Classifier Changes (#1052 and #1054)

## Affected Modules

### Core Classifier Entry Point
- **File**: crates/ripr/src/analysis/classifier.rs
- **Function**: `classify_probe()` [lines 12-20]
  - Orchestrates: `resolve_owner_function` → `find_related_tests` → `ProbeContext` → `ClassifiedProbeEvidence` → `evidence.classify()`

### Related-Test Matching
- **File**: crates/ripr/src/analysis/classify/related_tests.rs
- **Function**: `find_related_tests()` [lines 7-48]
  - Current logic: textual proximity matching in test names/bodies
  - **Issue #1052**: prefers wrong tests over exact-value oracle tests for struct field changes
  - Key components:
    * Line 14: `probe_tokens` extraction via `extract_identifier_tokens()`
    * Line 29-31: `calls_owner` check (direct owner references)
    * Line 34-38: `same_file_or_named` check (textual proximity bias — THE PROBLEM)
  - **Unit tests**: lines 78-229 (6 tests total)

### Reach Classification
- **File**: crates/ripr/src/analysis/classify/reach.rs
- **Function**: `reach_evidence()` [lines 4-28]
  - Takes `related_tests` and `owner_fn`
  - Returns `StageState::No` if `related_tests.is_empty()`
  - Returns `StageState::Yes` if any related test found
  - **Issue #1054**: inconsistent across sibling fields covered by same test
    * Both fields should get same reach verdict if same test covers both
    * Root cause: `find_related_tests()` may filter differently per field
- **Unit tests**: lines 30-96 (2 tests)

### Reveal Evidence (Oracle Matching)
- **File**: crates/ripr/src/analysis/classify/reveal.rs
- **Function**: `analyze_related_assertions()` [lines 40-93]
  - Matches assertions to probes via `assertion_matches_probe()` [lines 95-106]
  - Filters related_tests based on assertion matching
  - **Issue context**: assertion matching filters which tests contribute to reach verdict
  - Line 103: `token_match` requires length > 3 (may miss short field names like "open_in")
- **Function**: `oracle_matches_family()` [lines 198-260]
  - Family-specific oracle matching (ErrorPath, SideEffect, FieldConstruction, etc.)
- **Unit tests**: lines 297-700+ (extensive oracle/family matching coverage)

### Classification Decision
- **File**: crates/ripr/src/analysis/classify/decision.rs
- **Function**: `classify()` [lines 16-47]
  - Line 27: `if reach.state == No → ExposureClass::NoStaticPath`
  - Line 39-46: exposure classification logic
  - **Issue context**: reach verdict directly gates exposure classification
  - Any related_tests → reach=yes; no related_tests → reach=no (no path)

## Golden Fixture Regression Surface

### Fixtures Exercising Related-Test Matching
**Count**: 58 fixtures with "related.*test" in SPEC.md

**Key Rust fixtures**:
- `unrelated_test_mentions_token/` — token discrimination
- `strong_error_oracle/` — exact oracle matching (3 findings)
- `boundary_gap_multiline_assert/` — multi-line assertion shapes
- `boundary_gap_nested_tests/` — nested test structure
- `boundary_gap_reordered_tests/` — test ordering

**Python/TypeScript**:
- `python_related_test_name_similarity/` — name proximity matching
- `python_unrelated_test_mention/` — false-positive filtering
- `typescript_related_test_matching/` — alias/namespace import handling
- `typescript_related_test_name_proximity/` — proximity bias testing

### Fixtures Exercising Reach Classification
**Count**: 83 fixtures with `"reach"` in ripr evidence

**Breakdown**:
- 83 fixtures: have reach verdict in findings
- 89 fixtures: classify to `weakly_exposed`
- 89 fixtures: classify to `no_static_path` (at least one finding)
- **6 fixtures with ≥1 no_static_path classification**:
  * `fixtures/python_mixed_language_no_cross_route/expected/check.json`
  * `fixtures/python_ranking_noise_control/expected/check.json`
  * `fixtures/python_unrelated_test_mention/expected/check.json`
  * `fixtures/typescript_static_limit_taxonomy/expected/check.json`
  * `fixtures/typescript_strict_actionability/expected/check.json`
  * `fixtures/unrelated_test_mentions_token/expected/check.json`

### Reach + WeaklyExposed Combinations
Fixtures with both `reach.state="yes"` and `classification="weakly_exposed"`:
- Nearly all 83 reach fixtures (reach required for weakly_exposed)
- Example: `strong_error_oracle/` → 3 findings, 1 weakly_exposed with reach=yes

## Classifier Unit Tests

### Struct/Field-Specific Tests (classifier.rs)
**File**: crates/ripr/src/analysis/classifier.rs [lines 22-400+]

**Notable tests**:
1. `given_owner_symbol_when_resolving_owner_then_matches_full_identity` [lines 32-71]
   - Tests: related_tests matched by owner name
   - Assertion: `finding.related_tests.len() == 1` and name matches
2. `given_unrelated_test_mentions_probe_token_when_owner_is_not_called_then_no_static_path` [lines 74-119]
   - Tests: token mention WITHOUT owner call → no_static_path
   - Assertion: `finding.class == ExposureClass::NoStaticPath`
   - **This test will FAIL after #1052 fix if the struct field is now found**
3. `given_three_character_probe_token_in_test_name_when_owner_is_not_called_then_test_is_related` [lines 122-171]
   - Tests: 3+ char token in test name → related (textual proximity)
   - Assertion: `finding.ripr.reach.state == StageState::Yes` and test name matches

### Related-Test Matching Unit Tests (related_tests.rs)
**File**: crates/ripr/src/analysis/classify/related_tests.rs [lines 78-229]

**6 tests**:
1. `given_owner_function_when_tests_share_name_across_packages_then_filters_to_package` [84-108]
   - Tests: package prefix filtering (crates/X isolation)
2. `given_same_named_tests_when_finding_related_then_orders_by_file_path` [110-127]
   - Tests: sorting/dedup by file path
3. `given_probe_token_in_test_name_when_owner_is_not_called_then_test_is_related` [129-146]
   - Tests: token matching fallback when owner not called
4. `given_workspace_paths_when_extracting_package_prefix_then_handles_nested_markers` [148-166]
   - Tests: path normalization (crates/, /src/, /tests/)
5. `given_non_workspace_paths_when_extracting_package_prefix_then_returns_none` [168-173]
   - Tests: non-workspace path handling
6. `given_mixed_separator_path_when_normalizing_then_uses_workspace_relative_form` [175-179]
   - Tests: path separator normalization (\ → /)

**Critical for #1052**: Test #3 exercises the token-matching path used by struct field probes.

### Reach Evidence Unit Tests (reach.rs)
**File**: crates/ripr/src/analysis/classify/reach.rs [lines 30-96]

**2 tests**:
1. `given_no_related_tests_when_building_reach_evidence_then_stage_is_no` [36-46]
   - Tests: empty related_tests → reach=No
   - Assertion: `state == StageState::No, summary == "No static test path found for the changed owner"`
2. `given_related_tests_when_building_reach_evidence_then_names_owner_and_tests` [48-65]
   - Tests: any related_tests → reach=Yes
   - Assertion: `state == StageState::Yes` and names first 3 tests

**Critical for #1054**: These tests will PASS regardless of sibling-field consistency because they test reach as a binary (empty vs non-empty).

### Reveal/Oracle Matching Unit Tests (reveal.rs)
**File**: crates/ripr/src/analysis/classify/reveal.rs [lines 297-700+]

**Major tests** (selective list):
1. `reveal_evidence_keeps_assertionless_related_test_without_observe_signal` [302-312]
   - Tests: test with no assertions still appears in related_tests list
2. `reveal_evidence_records_matching_assertions_and_sorts_related_tests` [315-345]
   - Tests: assertions matched and sorted by test name
3. `reveal_evidence_ignores_unmatched_assertions` [347-370]
   - **CRITICAL**: unmatched assertions are filtered out
   - Tests that assertions with no token match are excluded
4. `assertion_matching_accepts_token_family_and_single_assertion_fallbacks` [372-415]
   - Tests: token matching (length > 3), family matching, single-assertion fallback
   - **Issue #1052 context**: token_match requires length > 3
5. `oracle_family_matching_covers_family_specific_shapes` [521-700+]
   - **Extensive**: FieldConstruction, ErrorPath, SideEffect, etc.
6. `discriminate_evidence_names_strength_and_oracle_kind` [418-518]
   - Tests: oracle strength verdicts (Strong/Medium/Weak/Smoke/None/Unknown)

**Critical for #1054 sibling fields**: Test #2 sorts by test name; if same test covers multiple fields, the assertion matching should preserve both.

## Integration Smoke Tests
**File**: crates/ripr/tests/cli_smoke.rs

- **Line 1240**: related_tests field presence in JSON
- **Line 1582**: `max_related_tests` config validation
- **Line 2557/2577**: related_tests JSON structure checks
- **Line 3000**: related_tests field exclusion in Shields format

## Risk Surface Summary

### High-Risk Regression Areas

#### 1. Related-Test Association Logic
**File**: related_tests.rs [lines 29-41]
- 58+ fixtures test this
- **Current risk**: textual proximity (test name contains probe token) outweighs direct owner references
- **Fix for #1052**: prefer tests that reference owner identifiers (struct name, field names) over generic token matches
- **Change impact**:
  * `calls_owner` check should be weighted higher
  * Owner-identifier tokens (field name, struct name) should outrank generic tokens
  * May require re-blessing fixtures where spurious proximity matches were being used

#### 2. Reach Verdict Consistency Across Sibling Fields
**File**: reach.rs + decision.rs + related_tests.rs
- 83 fixtures depend on reach classification
- **Issue #1054**: sibling fields covered by same test get inconsistent reach
- **Root cause**: related_tests filtering may differ per field based on assertion matching
- **Example**: 
  ```
  struct IssuesConfig {
    open_in: ...,   // Line 46 — related_tests empty? reach=No → no_static_path
    open_cap: ...,  // Line 47 — related_tests [test]? reach=Yes → weakly_exposed
  }
  test_issues_config() { assert_eq!(config.open_in, ...); assert_eq!(config.open_cap, ...); }
  ```
  * Both should see the same test in related_tests
  * Issue: token_match length > 3 filter, field name length sensitivity

#### 3. Oracle/Assertion Matching
**File**: reveal.rs [lines 63-84, 95-106]
- Long-tested behavior (extensive test coverage in reveal.rs)
- **Critical flow**:
  1. `analyze_related_assertions()` iterates related_tests
  2. For each test, matches assertions via `assertion_matches_probe()`
  3. Only matched assertions are added to RelatedTest list
  4. If no assertions match, test still appears but oracle=None
- **Token matching logic** [line 101-103]:
  ```rust
  token_match = probe_tokens.iter().any(|token| token.len() > 3 && assertion.text.contains(token))
  ```
  * Field names like "open_in" (7 chars) pass; "id" (2 chars) fail
  * **Issue #1054**: short field names may not match assertions even if test covers them
- **Change risk**:
  * Lowering token length threshold from 3 to 2 may introduce false positives
  * Changing assertion matching affects which probes reach "observe" stage
  * Affects all 89 weakly_exposed + 89 no_static_path fixtures

#### 4. Classification Gate on Reach
**File**: decision.rs [line 27]
- **Non-negotiable gate**: `reach.state == No → NoStaticPath` always
- Any change to related_tests that removes a test from reach will flip classification
- Affects all 83+ fixtures with reach verdicts
- **Safe direction**: only ADD related_tests (stay conservative), never remove
- **Dangerous direction**: changing reach calculation without re-testing all 83 fixtures

### Golden Blessing Scope

A fix to address #1052 + #1054 will likely require re-blessing:

#### Definite Re-bless:
1. **unrelated_test_mentions_token/** — reach verdict may change if token matching improves
2. Any fixture with struct field probes that should now classify higher (reach=No → reach=Yes)
3. Fixtures in the **6 no_static_path set** if related-test matching significantly improves:
   - python_mixed_language_no_cross_route/
   - python_ranking_noise_control/
   - python_unrelated_test_mention/
   - typescript_static_limit_taxonomy/
   - typescript_strict_actionability/
   - unrelated_test_mentions_token/

#### Likely Re-bless:
- All 58 fixtures with "related.*test" in SPEC.md if owner-identifier preference changes ordering
- Python/TypeScript related_test_matching fixtures if Rust logic changes ripple through language adapters

#### Conservative Re-bless (unlikely):
- Fixtures with assertion_count==1 fallback (last resort) — may be sensitive to token length changes

### Unverifiable/Unknown Risks

1. **Cross-language adapter integration**
   - Rust logic in related_tests.rs is shared by Python/TypeScript adapters
   - Changes affect all three languages; 58 fixtures includes ~20 Python and ~10 TypeScript
   - Verify: crates/ripr/src/analysis/language/{rust,python,typescript}.rs call find_related_tests()

2. **LSP hover/diagnostics rendering**
   - File: crates/ripr/src/lsp/hover.rs references related_tests
   - Changing related_tests structure may affect editor UX
   - Risk level: LOW (shape is stable, only population changes)

3. **Agent seam packets and downstream contracts**
   - File: docs/OUTPUT_SCHEMA.md versioning
   - related_tests field is part of JSON output contract
   - If oracle matching changes, oracle_strength/oracle_kind verdicts may change
   - Risk level: MEDIUM (may require JSON schema version bump)

4. **PR evidence summaries and reports**
   - crates/ripr/src/output/pr_evidence_ledger.rs, pr_evidence_summary.rs
   - These may render related_tests with specific assumptions
   - Risk level: LOW (data pass-through, not logic-dependent)

## Specific Defect Repro Files (For Verification)

### #1052 Repro (Related-Test Matcher Issues)
Expected location: should create fixture (currently exists in dogfood)
- Adds `pub(crate) struct RepoLane { ... }` to src/config.rs
- Adds test `repo_lane_toml_parse_pins_exact_field_defaults` in src/main.rs
  * Deserializes RepoLane from TOML
  * `assert_eq!(lane.field1, expected1)` for each field
- Probe: `probe:src_config.rs:40:call_deletion`
- **Current behavior**: weakly_exposed with unrelated body-rendering tests
- **Expected**: exposed with exact-oracle test in evidence
- **Golden file to watch**: would be in `fixtures/repo_lane_exact_oracle/expected/check.json` (or similar)

### #1054 Repro (Reach Inconsistency)
Expected location: should create fixture
- Adds sibling fields `open_in` (line 46) and `open_cap` (line 47) to struct IssuesConfig
- Adds test `issues_toml_parse_pins_exact_field_defaults`
  * `assert_eq!(absent.issues.open_in, ...);`
  * `assert_eq!(absent.issues.open_cap, ...);`
- Probes:
  * `probe:src_config.rs:46:field_construction` (open_in)
  * `probe:src_config.rs:47:field_construction` (open_cap)
- **Current behavior**: open_in gets NO finding; open_cap gets no_static_path
- **Expected**: both get same reach verdict (both exposed)
- **Golden file to watch**: would be in `fixtures/issues_config_sibling_fields/expected/check.json` (or similar)

---

**Summary**: 
- **83 fixtures** exercise reach classification
- **58 fixtures** exercise related-test matching
- **6 fixtures** actively assert no_static_path classifications
- **All classifier unit tests** must re-validate after fix
- **High risk of silent regressions** unless all 83 golden fixtures are blessed post-fix
