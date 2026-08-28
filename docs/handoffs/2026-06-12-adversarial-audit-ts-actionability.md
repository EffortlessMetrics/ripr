# Adversarial Audit: TypeScript Actionability Over-Emit Failure Modes

**Date**: 2026-06-12  
**Scope**: TypeScript preview repair-packet actionability logic — enumerating every way a TS finding could be wrongly marked actionable (repair_packet_ready := true when unsafe) and stating the fail-closed invariants required to prevent each.

**Context**: Presently `repair_packet_ready: false` is hardcoded in `preview_actionability.rs:63`. This audit assumes a future change flips it to `true` conditionally and identifies guard conditions that MUST hold to prevent emitting an unsafe, unverifiable, or non-refundable packet to an agent.

---

## Executive Summary

A TypeScript repair packet is actionable only if **ALL of these fail-closed invariants hold**:

1. **Dynamic/Unresolved Expected Value**: expected_value_or_variant is NOT dynamic (literal, static-analyzable)
2. **Oracle Strength Floor**: oracle_strength >= Strong (exact value, not snapshot/smoke/mock-guess)
3. **Related Test Ownership**: related test is owner-call-aware (not heuristic-only, not cross-package)
4. **Verify Command Realism**: verify_command is framework-aware, not a guess (jest/vitest/bun/npm/pnpm/yarn)
5. **Receipt Command Availability**: receipt_command exists and runs (not stubbed, not delegated to external provider)
6. **Edit Boundary Delineation**: allowed_edit_surface is explicit and narrow (not "any TypeScript file")
7. **No Cross-Language Fallback**: no unresolved cross-language oracle visibility (external test language undetected)
8. **No Mock/Snapshot as Discriminator**: mock-payload and snapshot-based oracles cannot be treated as actionable discriminators
9. **Custom Matcher Rejection**: custom/unknown matchers must be advisory-only
10. **No Guessed Repair Kind**: repair_kind must be derived from static analysis, not guessed from confidence score
11. **No Line-Keyed Finding ID**: finding IDs must be content-addressed (probe:file:family:hash), not line numbers
12. **No Unverifiable Assertion Shape**: assertion_shape must be concrete code, not prose or template variables
13. **No Confidence Inflation**: oracle_confidence must not inflate for missing expected values
14. **No Cross-Package Test Target**: test file must be in same package root as owner file (monorepo-aware)
15. **No Unresolved Test Framework**: test framework must be detected (jest/vitest/bun, not guessed)
16. **No Async Chain Omission**: async modifier (.resolves/.rejects) in oracle chain must be preserved
17. **No Circular Edit Constraint**: allowed_edit_surface must not contain forbidden files from same packet
18. **No Provider Delegation**: receipt_command must not delegate to an external provider (LLM, API, mutation engine)
19. **No Unresolved Import Chain**: imported owner references must be statically resolvable in test file
20. **No Static Limit Masking**: static_limit_kind must not be buried in evidence; must block packet emission explicitly

---

## Detailed Failure Mode Enumeration

### CATEGORY A: Oracle/Evidence Integrity

#### A1. Dynamic Expected Value Slips Through
**Failure mode**: A matcher argument is computed at runtime (e.g., `expect(x).toBe(getSomeValue())`) but the system marks expected_value_or_variant as "known" and emits the packet.

**Why it's fatal**: The agent cannot write a test that verifies the changed behavior because the oracle depends on a dynamic function call. The repair packet is non-refundable: running the verify_command will fail or produce false negatives.

**Evidence field**: `oracle.expected_value_or_variant` (from `extract_matcher_expected_value()` in oracle.rs)

**Fail-closed invariant**:
```
INVARIANT A1: oracle.has_dynamic_matcher_arg == false
   AND oracle.expected_value_or_variant.is_some()
   AND oracle.expected_value_or_variant.unwrap() is a literal or static path (not a call/variable)

ENFORCE AT: before repair_packet_ready := true
   - Grep oracle.has_dynamic_matcher_arg in evidence
   - Reject packet if true
   - Mark finding advisory: "dynamic matcher argument unresolved; cannot emit actionable packet"
```

---

#### A2. Oracle Strength Below Strong
**Failure mode**: Related test uses a weak oracle (snapshot, smoke test, mock, broad error) and the system marks it actionable, expecting the agent to write a test with equivalent weak oracle.

**Why it's fatal**: Weak oracles (toBeTruthy, toMatchSnapshot, toHaveBeenCalled) do not discriminate the changed behavior — the test may pass even if the fix is incorrect. A repair packet with a weak oracle is not safe to delegate.

**Evidence field**: `oracle.oracle_strength` (from oracle.rs::oracle_for_matcher)

**Fail-closed invariant**:
```
INVARIANT A2: For oracle_strength to justify actionable packet, BOTH must hold:
   (a) oracle_strength == Strong (ExactValue, ExactErrorVariant only), OR
   (b) oracle_strength == Medium AND oracle_kind == CustomMatcher AND assertion is human-verified

ENFORCE AT: before repair_packet_ready := true
   - Read oracle_strength from related test evidence
   - Reject if Smoke, Weak, Unknown
   - For Medium, require human gate (set flag repair_packet_ready_requires_human_review)
   - Mark findings with Snapshot/SmokeOnly/MockExpectation/BroadError advisory only
```

---

#### A3. Snapshot/Custom-Matcher Oracle Masquerading as Discriminator
**Failure mode**: Test has `expect(x).toMatchSnapshot(value)` where `value` is the "expected" and the system treats the snapshot as a concrete discriminator, emitting an actionable packet.

**Why it's fatal**: Snapshots are version-controlled text blobs, not executable discriminators. The agent cannot run `jest --updateSnapshot` in a CI/CD gate; snapshots require human review to accept. A snapshot-based packet is not safe for autonomous agents.

**Evidence field**: `oracle.oracle_kind` == OracleKind::Snapshot

**Fail-closed invariant**:
```
INVARIANT A3: oracle_kind == Snapshot → repair_packet_ready = false, gap_state = "advisory"
   
ENFORCE AT: actionability decision
   - If oracle_kind matches Snapshot, SmokeOnly, or BroadError, reject packet immediately
   - Mark as advisory: "snapshot/smoke/broad-error oracle cannot discriminate changed behavior"
   - Document in missing_actionability_fields: ["discriminator_type_not_actionable"]
```

---

#### A4. Mock Payload Oracle Without Call Shape
**Failure mode**: Test has `expect(fn).toHaveBeenCalledWith(x)` but the system has not extracted the callee function, argument types, or payload schema. It marks the packet actionable anyway, expecting the agent to write a mock-based test.

**Why it's fatal**: Without the complete call shape, the agent cannot know which function to mock or what arguments to verify. The verify_command will fail or become a false positive.

**Evidence field**: `oracle.mock_payload` (should be Some(TypeScriptMockPayload) with callee + arguments)

**Fail-closed invariant**:
```
INVARIANT A4: oracle_kind == MockExpectation AND oracle_strength == Medium
   → mock_payload must be Some(TypeScriptMockPayload)
   AND mock_payload.callee.is_some()
   AND mock_payload.arguments.len() > 0

ENFORCE AT: before repair_packet_ready := true
   - If oracle_kind == MockExpectation && mock_payload.is_none(), reject
   - Mark as advisory: "mock payload not extracted; cannot emit actionable mock-based repair packet"
```

---

#### A5. Error Payload Without Variant Name
**Failure mode**: Test has `expect(fn).toThrow()` (no argument) or `expect(fn).rejects.toThrow(Error)` (base class only), but the system marks expected_value_or_variant as "unknown" yet still emits the packet.

**Why it's fatal**: The agent will write a test that catches any error, not the specific error variant the changed code now throws. The test is a false positive.

**Evidence field**: `oracle.error_payload`, `oracle.oracle_kind` == OracleKind::ExactErrorVariant

**Fail-closed invariant**:
```
INVARIANT A5: If oracle_kind == ExactErrorVariant:
   error_payload must be Some(TypeScriptErrorPayload)
   AND error_payload.variant_name.is_some()
   AND error_payload.variant_name is a concrete string (not "Error", "unknown")

ENFORCE AT: oracle extraction (oracle.rs)
   - If toThrow/toThrowError has no argument or only base class, mark as BroadError, not ExactErrorVariant
   - Emit oracle_strength = Weak
   - Reject from actionable packets
```

---

#### A6. Async Modifier (.resolves/.rejects) Stripped During Extraction
**Failure mode**: Test has `expect(promise).resolves.toBe(42)` but the oracle extraction drops the `.resolves` modifier and marks expected_value_or_variant as "42". The agent writes a sync test instead.

**Why it's fatal**: The test becomes a false negative (hangs) if the changed code does not return a Promise.

**Evidence field**: `async_modifier` from expect_assertion_chain_modifier() (oracle.rs)

**Fail-closed invariant**:
```
INVARIANT A6: If async_modifier is present (Some("resolves") or Some("rejects")):
   - Preserve it in TypeScriptAssertion.async_modifier
   - Emit assertion_shape with async syntax: expect(promise).resolves.toBe(expected)
   - Verify verify_command includes --testTimeout or equivalent for async tests

ENFORCE AT: oracle extraction and assertion_shape generation
   - grep for "resolves\|rejects" in oracle evidence
   - Reject packet if async_modifier lost
   - Mark as advisory: "async chain modifier unresolved"
```

---

### CATEGORY B: Test Ownership & Relation Integrity

#### B1. Cross-Package Test Target (Monorepo)
**Failure mode**: In a monorepo with `packages/a/src/lib.ts` and `packages/b/src/lib.test.ts`, the system selects the test from package B as the "related test" for a source file in package A, and emits it as owned relation.

**Why it's fatal**: A test in a different package may not run when the source package is updated. The repair packet is non-refundable: verify_command runs in the wrong context.

**Evidence field**: `related_test` file path

**Fail-closed invariant**:
```
INVARIANT B1: related_test must be in the same package root as owner file
   
   same_package_root(owner.file, test.file, workspace_root) == true

ENFORCE AT: test candidate selection (related_tests.rs::related_test_candidates)
   - Filter candidates by same_package_root() before ranking
   - Pass workspace_root to is_actionable_entry() / classify_typescript_finding()
   - Reject packet if test file is in different package
   - Mark as advisory: "cross-package test candidate; cannot emit actionable packet without workspace verification"
```

---

#### B2. Heuristic-Only Relation (Not Owner-Call-Aware)
**Failure mode**: Test searches for the function name as a string in the test body (heuristic_relation) rather than detecting an actual call or import. E.g., test has `const s = "applyDiscount"` but never calls `applyDiscount()`. System marks it as owner-call relation.

**Why it's fatal**: The test does not actually exercise the owner function. Any assertion is a false positive.

**Evidence field**: `relation_kind` from related_tests.rs (should be DirectOwnerCall, ImportedOwnerCall, ReceiverOwnerCall, ClassMethodCall, ModuleValueReference — NOT heuristic fallback)

**Fail-closed invariant**:
```
INVARIANT B2: has_oracle_eligible_relation must be true for actionable packet
   
   has_oracle_eligible_relation := relation_kind is one of:
     - DirectOwnerCall (contains_call_name() + not shadowed)
     - ImportedOwnerCall (import source matches, import call in body)
     - ReceiverOwnerCall (receiver constructed, member call exists)
     - ClassMethodCall (class constructed, method call exists)
     - ModuleValueReference (module import, reference in expect())

ENFORCE AT: actionability.rs::typescript_actionability_for()
   - If !has_oracle_eligible_relation, emit gap_state = "advisory", category = "ambiguous_related_test"
   - NEVER emit repair_packet_ready = true
   - Mark in missing_actionability_fields: ["related_test_or_observer"]
```

---

#### B3. Owner Name Shadowed by Test Variable
**Failure mode**: Test has `const applyDiscount = 123` (local variable) and also calls the imported `applyDiscount()`. The system detects the name but not the shadowing, emitting the packet.

**Why it's fatal**: The test may exercise the local variable instead of the owner function. The assertion is unreliable.

**Evidence field**: Test body text analysis in related_tests.rs

**Fail-closed invariant**:
```
INVARIANT B3: Before marking DirectOwnerCall relation, verify:
   - owner_name NOT declared locally in test body
   - Test calls the imported or in-scope owner_name function, not a variable

ENFORCE AT: related_tests.rs::owner_call_relation()
   - If owner_name_shadowed_by_unrelated_import(test, owner) → skip DirectOwnerCall
   - If local_identifier_declared_in_test_body(test, owner_name) → skip DirectOwnerCall
   - Fall back to heuristic_relation (which will be rejected by B2)
```

---

#### B4. Import Source Mismatch (Wrong Module)
**Failure mode**: Test imports `applyDiscount` from `./utils` but the owner is in `./pricing`. System matches by name alone.

**Why it's fatal**: The test exercises a different function with the same name. The repair packet is a false positive.

**Evidence field**: Import source from test imports; owner.file path

**Fail-closed invariant**:
```
INVARIANT B4: For ImportedOwnerCall or ModuleValueReference relation:
   import_source_matches_owner(import, test.file, owner) must be true
   
   Where: import_source_matches_owner resolves the import source path
          and compares normalized paths with owner.file

ENFORCE AT: related_tests.rs
   - Every import-based relation must pass import_source_matches_owner()
   - Reject if source module is unresolved or guessed
   - Mark as advisory: "imported owner module unresolved"
```

---

#### B5. Test Mocking the Owner Module
**Failure mode**: Test has `jest.mock('./module')` or `vi.mock('./module')` and the system still marks the test as related/owned.

**Why it's fatal**: A mocked module does not exercise the real owner code. Any assertion is a false positive.

**Evidence field**: Test body detection of mock() calls

**Fail-closed invariant**:
```
INVARIANT B5: If test_mocks_owner_module(test, owner) == true:
   - REJECT all relation types (DirectOwnerCall, ImportedOwnerCall, etc.)
   - Emit gap_state = "advisory"
   - Mark as missing: ["related_test_or_observer"]

ENFORCE AT: related_tests.rs::related_test_candidates()
   - Before returning any candidate, check test_mocks_owner_module()
   - Filter out mocked-module candidates
```

---

### CATEGORY C: Verify & Receipt Command Integrity

#### C1. Verify Command is a Guess, Not Framework-Aware
**Failure mode**: System detects test framework as "unknown" or "jest|vitest|bun|?" but fills in verify_command as a guess: `npm test` (not scoped to the changed file or test name).

**Why it's fatal**: The agent cannot verify the repair worked. The command is a false positive: it may pass for unrelated reasons.

**Evidence field**: `verify_command` from package.rs::verify_command_for_discovery()

**Fail-closed invariant**:
```
INVARIANT C1: verify_command MUST be generated by a detected, not guessed, test framework
   
   Allowed producers:
   - verify_command_for_discovery() with framework ∈ {jest, vitest, bun, node}
   - Framework detected from package.json dependencies or config files
   - NOT a fallback like "npm test" without --testNamePattern or --testPathPattern

ENFORCE AT: package.rs
   - If framework.is_unknown() or framework.is_guessed(), set verify_command = None
   - Emit as missing_actionability_field: ["verify_command"]
   - Mark finding advisory: "test framework not detected; cannot emit verify command"
```

---

#### C2. Receipt Command Does Not Exist
**Failure mode**: System generates receipt_command as a path that does not exist or a shell script that is not executable.

**Why it's fatal**: After the agent runs verify_command, the receipt cannot be recorded. The gap-decision ledger cannot track the repair.

**Evidence field**: `receipt_command` from package.rs or gap_decision_ledger.rs

**Fail-closed invariant**:
```
INVARIANT C2: receipt_command must exist and be executable
   
   receipt_command must be:
   - A resolvable shell command (not a placeholder or template)
   - NOT delegated to an external provider (LLM API, GitHub Actions, mutation engine)
   - Generate a JSON artifact at a known path (e.g., target/ripr/receipts/...)

ENFORCE AT: before render_agent_gap_record_packet_json()
   - Validate receipt_command non-empty (validate_agent_gap_record_packet checks this)
   - Before marking actionable, verify receipt_command invokes a local CLI, not an API
   - Reject if receipt_command contains curl/api.openai.com/anthropic or equivalent
```

---

#### C3. Receipt Command Delegates to External Provider
**Failure mode**: receipt_command is `curl https://api.anthropic.com/mutations/confirm` or similar — delegating mutation proof to an external LLM.

**Why it's fatal**: The agent cannot refund the repair if the external service is down or returns a false positive. The packet is not self-contained.

**Evidence field**: `receipt_command` text

**Fail-closed invariant**:
```
INVARIANT C3: receipt_command must NEVER call an external provider
   
   Forbidden patterns:
   - curl/wget to api.*.com
   - LLM APIs (anthropic, openai, google, cohere)
   - Mutation-testing SaaS (stryker-dashboard, etc.)
   - GitHub Actions API without local consent
   - AWS/GCP/Azure APIs for mutation proof

ENFORCE AT: before repair_packet_ready := true
   - Grep receipt_command for http(s), curl, invoke, api, provider patterns
   - Reject if found
   - Mark as advisory: "receipt command delegates to external provider; cannot emit actionable packet"
```

---

#### C4. Verify Command Will Not Run in CI/CD Context
**Failure mode**: verify_command is `jest src/lib.test.ts` but the repo has a CI build that strips test files (e.g., `build:prod` excludes `**/*.test.ts`). The command fails in the CI context.

**Why it's fatal**: The agent runs verify_command in a CI/CD environment, not a local dev environment. If the command fails in CI but passes locally, the repair is not verifiable.

**Evidence field**: `verify_command`, package.json scripts

**Fail-closed invariant**:
```
INVARIANT C4: verify_command must be runnable in CI/CD context with no local setup
   
   - Test files must be present in the CI artifact
   - verify_command must not depend on local node_modules (use npm ci)
   - Framework must be present in CI (no optional devDependencies stripped)

ENFORCE AT: before repair_packet_ready := true
   - DEFER to user verification: emit warning in repair packet
   - "verify_command may not run in your CI context; confirm it works before delegating"
   - Do NOT mark actionable until user confirms
```

---

#### C5. Receipt Command Is a Template, Not a Concrete Command
**Failure mode**: receipt_command is `ripr outcome --before <git-ref-before> --after <git-ref-after> --out <path>` with template variables not substituted.

**Why it's fatal**: The agent cannot run the command as-is; it must parse and substitute variables, which is not safe.

**Evidence field**: `receipt_command` text

**Fail-closed invariant**:
```
INVARIANT C5: receipt_command must be a concrete, copy-pasteable shell command
   
   NO template variables (< >, {{ }}, $(...), etc.)
   Must include concrete values for:
   - File paths (--before, --after, --out)
   - Gap IDs
   - Language/scope

ENFORCE AT: before repair_packet_ready := true
   - Parse receipt_command for template markers
   - Reject if found
   - Mark as advisory: "receipt command is a template; substitute variables before emission"
```

---

### CATEGORY D: Edit Boundary Integrity

#### D1. Allowed Edit Surface is Implicit or Overly Broad
**Failure mode**: System derives allowed_edit_surface from repair_route.target_file or related_test.file, but the file path is ambiguous (e.g., `lib.test.ts` when multiple files match) or covers too many files (e.g., entire `src/` directory).

**Why it's fatal**: The agent may edit unintended files. The repair packet is not bounded.

**Evidence field**: `allowed_edit_surface` array from gap_seam_packets.rs::allowed_edit_surface_for_gap_route()

**Fail-closed invariant**:
```
INVARIANT D1: allowed_edit_surface must be explicit and minimal
   
   - Each entry must be a concrete file path (no glob patterns, no directories)
   - Length must be 1 (single target file) or justified (multiple tests in same suite)
   - Must be derived from static analysis, not guessed

ENFORCE AT: gap_seam_packets.rs::allowed_edit_surface_for_gap_route()
   - If route.target_file.is_some(), extract single concrete path
   - If route.related_test.is_some(), extract single concrete test file path
   - Reject if length > 1 without explicit justification
   - Reject if any entry is a directory or glob pattern
```

---

#### D2. Forbidden Files Not Delineated
**Failure mode**: allowed_edit_surface includes `tests/` but forbidden_files is empty. The agent may edit shared test utilities or setup files that affect other tests.

**Why it's fatal**: The repair affects unrelated tests. The receipt shows false positive improvements.

**Evidence field**: `forbidden_files` array from gap_seam_packets.rs::forbidden_files_for_gap_record()

**Fail-closed invariant**:
```
INVARIANT D2: forbidden_files must be computed and non-empty if allowed_edit_surface is broad
   
   - If allowed_edit_surface includes test directories, explicitly list forbidden files:
     - __tests__/setup.ts
     - jest.config.ts
     - vitest.config.ts
     - test utilities
   - If allowed_edit_surface is a single file, forbidden_files can be empty

ENFORCE AT: gap_seam_packets.rs::forbidden_files_for_gap_record()
   - Compute forbidden_files by inverse: all files except allowed_edit_surface + safe utilities
   - Include in packet JSON
   - Fail if allowed_edit_surface is broad and forbidden_files is empty
```

---

#### D3. Edit Surface Spans Multiple Packages
**Failure mode**: allowed_edit_surface includes `packages/a/src/lib.test.ts` and `packages/b/src/lib.test.ts` (two tests in different packages for the same gap).

**Why it's fatal**: The agent may fix the bug in only one package. The repair is incomplete and the receipt is false.

**Evidence field**: `allowed_edit_surface` file paths

**Fail-closed invariant**:
```
INVARIANT D3: allowed_edit_surface must NOT span multiple package roots
   
   - If workspace_root is known, enforce same_package_root() for all files
   - Reject packets with files in different packages

ENFORCE AT: before repair_packet_ready := true
   - For each file in allowed_edit_surface, compute package_root
   - Ensure all have the same package_root
   - Reject if not
```

---

#### D4. Edit Surface Includes Uneditable Files (Read-Only, Generated)
**Failure mode**: allowed_edit_surface includes `dist/index.d.ts` (generated TypeScript declaration) or `.git/config` (read-only).

**Why it's fatal**: The agent cannot edit the file. The repair fails.

**Evidence field**: `allowed_edit_surface` + file system permissions

**Fail-closed invariant**:
```
INVARIANT D4: Files in allowed_edit_surface must be:
   - User-editable (not read-only, not generated)
   - Not in .gitignore (unless explicitly intentional)
   - Not build artifacts (dist/, build/, .next/, etc.)

ENFORCE AT: before repair_packet_ready := true
   - Check file permissions: writable == true
   - Check .gitignore: not ignored
   - Check suffix: not .d.ts or other generated extensions
   - Reject if any file fails
```

---

### CATEGORY E: Repair Kind & Discriminator Integrity

#### E1. Repair Kind is Guessed, Not Derived from Analysis
**Failure mode**: System infers repair_kind from oracle_strength or gap_state rather than from explicit analysis of the changed behavior. E.g., "write_targeted_test" is the default, not a derived fact.

**Why it's fatal**: The repair kind determines what the agent is asked to do. A guessed repair_kind may not match the actual repair needed.

**Evidence field**: `repair_kind` in gap_repair_route

**Fail-closed invariant**:
```
INVARIANT E1: repair_kind must be explicitly derived, not defaulted
   
   Allowed values:
   - AddBoundaryAssertion (exact boundary value missing)
   - AddErrorDiscriminator (error variant undetected)
   - AddOutputGolden (output contract missing)
   - StrengthenExistingTest (test exists but oracle is weak)
   - InspectStaticLimit (static analysis boundary found)

ENFORCE AT: before repair_packet_ready := true
   - Reject if repair_kind == "unknown" or empty
   - Reject if repair_kind does not match probe_family or exposure_class
   - Require explicit evidence for repair_kind choice
```

---

#### E2. Missing Discriminator is Prose, Not Actionable
**Failure mode**: missing_discriminator is `"the new behavior when amount > threshold"` (prose) instead of a concrete value or variant name.

**Why it's fatal**: The agent cannot write a test assertion for prose. The guidance is unusable.

**Evidence field**: `missing_discriminator` in gap_repair_route

**Fail-closed invariant**:
```
INVARIANT E2: missing_discriminator must be concrete or unambiguous
   
   Allowed forms:
   - Exact literal: "42", "true", "null"
   - Variant name: "InsufficientFundsError", "TIMEOUT"
   - Well-known constant: "DEFAULT_THRESHOLD", "MAX_RETRIES"
   - NOT prose: "the new behavior", "error when...", "value is"

ENFORCE AT: before repair_packet_ready := true
   - Parse missing_discriminator
   - Reject if it reads like prose (contains "when", "if", "the", "error that")
   - Require concrete value or variant from static analysis
```

---

#### E3. Changed Behavior Not Stated Precisely
**Failure mode**: changed_behavior is `"behavior has changed"` instead of `"if (amount >= threshold) now returns 0 instead of full discount"`.

**Why it's fatal**: The agent does not understand what behavior to test for. The repair is undirected.

**Evidence field**: `changed_behavior` in gap_repair_route

**Fail-closed invariant**:
```
INVARIANT E3: changed_behavior must explain the delta with before/after or clear causality
   
   Ideal form: "if (condition) now does X instead of Y"
   Acceptable: "behavior changed from X to Y in context Z"
   NOT: "behavior has changed", "updated logic", "fixed bug"

ENFORCE AT: route construction in analyzer
   - Require concrete before/after or cause/effect
   - Reject prose-only descriptions
   - Mark as advisory if unclear
```

---

### CATEGORY F: Confidence & Strength Integrity

#### F1. Oracle Confidence Inflated for Missing Expected Value
**Failure mode**: extracted expected_value_or_variant is None, but oracle_confidence is computed as "high" anyway because oracle_kind is ExactValue.

**Why it's fatal**: The agent believes the oracle is strong when it is not. The test may be written incorrectly.

**Evidence field**: `oracle_confidence` from oracle.rs::derive_oracle_confidence()

**Fail-closed invariant**:
```
INVARIANT F1: oracle_confidence MUST reflect both oracle_kind and expected_value resolution
   
   if expected_value_or_variant.is_none():
     oracle_confidence = "low" OR "unknown", regardless of oracle_kind
   
   if has_dynamic_matcher_arg:
     oracle_confidence = "low" OR "unknown", regardless of oracle_kind

ENFORCE AT: oracle.rs::derive_oracle_confidence()
   - Factor in has_dynamic_matcher_arg and expected_value_or_variant.is_none()
   - Do NOT inflate for oracle_kind alone
   - Return Confidence::Low if either is unresolved
```

---

#### F2. Package Confidence Guessed from Dependencies
**Failure mode**: typescript_package_confidence is "high" because jest is in package.json, but jest is mocked in the specific test.

**Why it's fatal**: The confidence level is a false positive. The packet may be marked actionable based on inflated confidence.

**Evidence field**: `typescript_package_confidence` from package.rs

**Fail-closed invariant**:
```
INVARIANT F2: typescript_package_confidence should NOT be the primary gate for actionability
   
   Use confidence as a secondary ranking signal, not as the actionability decision.
   Actionability is determined by:
   - Static limits (gate all: static_limitation)
   - Exposure class (gate some: already_observed)
   - Evidence completeness (gate most: missing_fields)
   - NOT by confidence score alone

ENFORCE AT: actionability.rs::typescript_actionability_for()
   - Do not consult confidence score in the actionability decision
   - Use confidence for queue ranking only
```

---

### CATEGORY G: Finding ID & Provenance Integrity

#### G1. Line-Keyed Finding ID (Not Content-Addressed)
**Failure mode**: Finding ID is `probe:src_lib.ts:42:typescript_preview` (keyed by line number 42) instead of content-addressed. Code refactoring moves the probe to line 50, and the suppression list does not match.

**Why it's fatal**: Suppressions do not transfer correctly. The repair packet may be emitted despite prior suppression.

**Evidence field**: `canonical_gap_id` or `probe.id`

**Fail-closed invariant**:
```
INVARIANT G1: Finding IDs must be content-addressed, not line-keyed
   
   Format: probe:<file>:<family>:<content_hash>
   Example: probe:src_lib.ts:error_path:c1a03250
   
   NOT: probe:src_lib.ts:42:typescript_preview

ENFORCE AT: finding generation
   - Compute content-addressed fingerprint from changed code + probe family
   - Hash must be stable across file refactoring
   - Use SHA-256 or similar, normalize paths (/ on all platforms)
```

---

#### G2. Canonical Gap ID Unresolved
**Failure mode**: canonical_gap_id is empty, and the system cannot correlate findings across Rust and TypeScript representations.

**Why it's fatal**: Cross-language deduplication fails. The same gap may produce multiple repair packets.

**Evidence field**: `canonical_gap_id` in GapRecord

**Fail-closed invariant**:
```
INVARIANT G2: canonical_gap_id must be populated before repair_packet_ready := true
   
   For preview languages (TypeScript), derive from:
   - Gap kind (MissingBoundaryAssertion, etc.)
   - Anchor: file, line, owner
   - Static hash of changed expression
   
   Canonical form: gap:<language>:<kind>:<hash>

ENFORCE AT: before render_agent_gap_record_packet_json()
   - Require non_empty(&record.canonical_gap_id)
   - Validate in validate_agent_gap_record_packet()
   - Marked as missing_actionability_fields otherwise
```

---

#### G3. No Traceability from Finding to Spec or Test
**Failure mode**: The finding has no evidence_ids linking it back to a test or spec requirement. The agent cannot understand the original requirement.

**Why it's fatal**: The repair is not anchored in requirements. The agent may fix the symptom, not the root cause.

**Evidence field**: `evidence_ids` array in GapRecord

**Fail-closed invariant**:
```
INVARIANT G3: evidence_ids must be non-empty and traceable
   
   evidence_ids entries should include:
   - Probe IDs (probe:file:family:hash)
   - Related test IDs (test:file:name:line)
   - Spec references (spec-0042, ADR-5, etc.)
   
   Do NOT use opaque ids; make them resolvable

ENFORCE AT: gap ledger generation
   - Collect evidence_ids from classification
   - Require at least one probe ID and one related test ID
   - Reject if evidence_ids is empty
```

---

### CATEGORY H: Static Limit & Boundary Integrity

#### H1. Static Limit Kind Buried in Evidence
**Failure mode**: A TypeScriptStaticLimit is detected but instead of blocking the packet at the top level, it is buried in the evidence array as advisory text. The packet is marked actionable anyway.

**Why it's fatal**: The static limit bounds the safe guidance. Ignoring it produces an incomplete or unsafe repair packet.

**Evidence field**: `static_limit_kind`, `static_limit_detail` in GapRecord

**Fail-closed invariant**:
```
INVARIANT H1: static_limit_kind blocks repair_packet_ready = true
   
   If record.static_limit_kind.is_some():
     - repair_packet_ready = false
     - gap_state = "static_limitation"
     - category = record.static_limit_kind
     - REJECT packet immediately

ENFORCE AT: actionability.rs::typescript_actionability_for()
   - First gate: if static_limit.is_some(), return early (lines 51-62)
   - Do NOT proceed to oracle/relation checks
```

---

#### H2. Cross-Language Oracle Visibility Unresolved
**Failure mode**: The changed code is TypeScript, but the related test is Python or Rust (detected via cross_language_oracle_visibility_unresolved()). The system still marks the TS finding actionable.

**Why it's fatal**: The TypeScript preview adapter cannot safely construct a TypeScript test from evidence extracted in another language. Type safety is compromised.

**Evidence field**: Cross-language checks in evidence_record.rs

**Fail-closed invariant**:
```
INVARIANT H2: cross_language_oracle_visibility_unresolved() must block actionable packets
   
   If cross_language_oracle_visibility_unresolved(entry) == true:
     - Do NOT emit repair_packet_ready = true
     - Mark as advisory: "cross_language_oracle_visibility_unresolved"
     - Link to evidence in external_language_test_requirement

ENFORCE AT: is_actionable_entry() check in agent_seam_packets.rs (lines 1324-1328)
   - BEFORE emitting any packet, check !cross_language_oracle_visibility_unresolved(entry)
   - REJECT if true
```

---

#### H3. Cross-Language Test Target Unresolved
**Failure mode**: The gap requires a test in a language other than TypeScript (e.g., Python integration test), but the system marks the TS finding as actionable anyway, expecting a TS-only test.

**Why it's fatal**: The test does not exercise the actual cross-language boundary. The repair is incomplete.

**Evidence field**: Cross-language test target checks in evidence_record.rs

**Fail-closed invariant**:
```
INVARIANT H3: cross_language_test_target_unresolved() must block actionable packets
   
   If cross_language_test_target_unresolved(entry) == true:
     - Do NOT emit repair_packet_ready = true
     - Mark as advisory: "cross_language_test_target_unresolved"
     - Guidance: "add a cross-language test to verify the boundary"

ENFORCE AT: is_actionable_entry() check
   - BEFORE emitting any packet, check !cross_language_test_target_unresolved(entry)
   - REJECT if true
```

---

### CATEGORY I: Language Status & Preview Integrity

#### I1. Preview Language Marked Actionable Without Warning
**Failure mode**: TypeScript (preview language) finding is marked repair_packet_ready = true without a flag indicating the findings may be incomplete or experimental.

**Why it's fatal**: The agent treats preview evidence as stable evidence. Gates may incorrectly authorize changes.

**Evidence field**: `language_status` == "preview" in GapRecord

**Fail-closed invariant**:
```
INVARIANT I1: All preview-language packets must carry explicit preview warnings
   
   - JSON field: "preview_status": "experimental"
   - must_not_change includes: "do not treat preview-language evidence as gate authority"
   - authority_boundary != "preview_advisory_only" (i.e., NOT actionable for gates)

ENFORCE AT: before repair_packet_ready := true for preview languages
   - Add "preview_status": "experimental" to packet
   - Add explicit do_not_change warnings
   - Set authority_boundary to "preview_advisory_only" (not override to "actionable")
```

---

#### I2. Stable Language Requirements Not Met
**Failure mode**: TypeScript is marked as language_status = "stable" but the oracle extraction is incomplete (oracle_kind == Unknown, expected_value == None).

**Why it's fatal**: The language was not ready for stable status. Packets are emitted without complete evidence.

**Evidence field**: `language_status` == "stable" + oracle evidence

**Fail-closed invariant**:
```
INVARIANT I2: Before promoting TypeScript to stable, ensure:
   - Oracle extraction is > 95% complete (no Unknown oracles)
   - Expected values are extracted for > 90% of discovered tests
   - Cross-package filtering is enforced
   - Mock-payload and error-variant detection are stable
   - Async modifier preservation is tested
   - No dynamic-matcher false positives

ENFORCE AT: xtask or CI gate
   - Run oracle extraction on large test corpus
   - Compute completion metrics
   - Require human sign-off before language_status = "stable"
   - Until then, language_status = "preview" always
```

---

### CATEGORY J: Projection Eligibility & Gate Integrity

#### J1. Projection Eligibility Not Checked Before Actionable
**Failure mode**: GapRecord.projection_eligibility["agent_packet"].eligible is false, but the system still marks repair_packet_ready = true.

**Why it's fatal**: The gap was explicitly marked ineligible (suppressed, waived, etc.) but the packet is emitted anyway.

**Evidence field**: `projection_eligibility` map in GapRecord

**Fail-closed invariant**:
```
INVARIANT J1: repair_packet_ready = true ONLY if projection_eligibility["agent_packet"].eligible == true
   
ENFORCE AT: validate_agent_gap_record_packet() (agent_seam_packets.rs lines 839-871)
   - Line 840-843: Check projection exists and eligible
   - Line 844-849: If not eligible, return Err()
   - Do NOT proceed to render_agent_gap_record_packet_json()
```

---

#### J2. Preview Language Gate Bypassed
**Failure mode**: language_status == "preview" but the system still marks repair_packet_ready = true without consulting safe_gate_predicate.preview_language flag.

**Why it's fatal**: Preview-language packets are delegated to agents as if they were stable.

**Evidence field**: `safe_gate_predicate.preview_language` in GapRecord

**Fail-closed invariant**:
```
INVARIANT J2: If safe_gate_predicate.preview_language == true:
   - Reject from repair_packet_ready = true
   - Mark as advisory
   - Add must_not_change: "do not treat preview language as gate authority"

ENFORCE AT: validate_agent_gap_record_packet()
   - Add check: if record.safe_gate_predicate.preview_language, return Err()
   - Or move preview check to separate gate before actionability
```

---

## Summary Table: Fail-Closed Invariants

| Invariant | Category | Enforcement Point | Action if Violated |
|-----------|----------|-------------------|-------------------|
| A1: No dynamic expected values | Oracle | oracle extraction | Set repair_packet_ready = false |
| A2: oracle_strength >= Strong | Oracle | actionability decision | Mark advisory |
| A3: No snapshot/smoke oracles | Oracle | actionability decision | Mark advisory |
| A4: Mock payload with call shape | Oracle | oracle extraction | Mark advisory |
| A5: Error variant named explicitly | Oracle | oracle extraction | Downgrade to BroadError |
| A6: Async modifier preserved | Oracle | oracle extraction | Mark advisory if lost |
| B1: Same package root (monorepo) | Test Ownership | test candidate filtering | Reject candidate |
| B2: Owner-call-aware relation | Test Ownership | actionability decision | Mark advisory |
| B3: Owner name not shadowed | Test Ownership | test candidate filtering | Skip DirectOwnerCall |
| B4: Import source verified | Test Ownership | test candidate filtering | Reject ImportedOwnerCall |
| B5: Test does not mock owner | Test Ownership | test candidate filtering | Skip relation |
| C1: Framework is detected, not guessed | Verify Command | package discovery | Set verify_command = None |
| C2: Receipt command exists | Receipt Command | validation gate | Reject packet |
| C3: Receipt does not call external provider | Receipt Command | validation gate | Reject packet |
| C4: Verify command runs in CI | Verify Command | user confirmation gate | Emit warning, do not auto-mark actionable |
| C5: Receipt is concrete, not template | Receipt Command | validation gate | Reject packet |
| D1: Allowed edit surface explicit & minimal | Edit Boundary | route analysis | Reject if ambiguous/broad |
| D2: Forbidden files delineated | Edit Boundary | route analysis | Reject if forbidden is empty and allowed is broad |
| D3: Edit surface in same package | Edit Boundary | validation gate | Reject cross-package |
| D4: Edit files are writable, not generated | Edit Boundary | validation gate | Reject if read-only or .d.ts |
| E1: Repair kind derived, not defaulted | Repair Kind | route construction | Reject if unknown |
| E2: Missing discriminator is concrete | Discriminator | route construction | Mark advisory if prose |
| E3: Changed behavior explained with before/after | Changed Behavior | route construction | Mark advisory if unclear |
| F1: Oracle confidence reflects completeness | Confidence | oracle extraction | Downgrade if expected value missing |
| F2: Confidence not sole actionability gate | Confidence | actionability decision | Use confidence for ranking, not gating |
| G1: Finding IDs content-addressed | Finding ID | finding generation | Use probe:file:family:hash |
| G2: Canonical gap ID populated | Canonical Gap | validation gate | Reject if empty |
| G3: Evidence IDs traceable | Provenance | gap ledger | Reject if evidence_ids empty |
| H1: Static limit blocks packet | Static Limit | actionability early return | Return gap_state = "static_limitation" |
| H2: Cross-language oracle visibility resolved | Cross-Language | is_actionable_entry() | Reject packet if unresolved |
| H3: Cross-language test target resolved | Cross-Language | is_actionable_entry() | Reject packet if unresolved |
| I1: Preview language warnings present | Language Status | packet rendering | Add preview flags and warnings |
| I2: Stable language requirements met | Language Status | promotion gate | Require human sign-off before stable |
| J1: Projection eligibility checked | Gate Eligibility | validate_agent_gap_record_packet() | Return Err if not eligible |
| J2: Preview language gate respected | Preview Gate | validation gate | Reject if safe_gate_predicate.preview_language |

---

## Implementation Roadmap: Guard All 20 Failure Modes

### Phase 1: Rust Validation Gates (Implement First)
These gates run in Rust before any TypeScript code sees the packet.

1. **validate_agent_gap_record_packet()** (agent_seam_packets.rs:839–871)
   - ✓ Check projection_eligibility["agent_packet"].eligible (J1)
   - ✓ Check repair_route present (E1)
   - ✓ Check verification_commands non-empty (C1)
   - ✓ Check repairability or InspectStaticLimit (E1)
   - ✓ Check allowed_edit_surface non-empty (D1)
   - ✓ Check receipt_command non-empty (C2)
   - **ADD**: Check safe_gate_predicate.preview_language (J2)
   - **ADD**: Check canonical_gap_id non-empty for preview languages (G2)
   - **ADD**: Check canonical_gap_id for stable languages too (G2)
   - **ADD**: Check receipt_command does not contain provider patterns (C3)

2. **oracle.rs extraction**
   - ✓ Derive oracle_kind, oracle_strength (A2)
   - ✓ Extract expected_value_or_variant with has_dynamic_matcher_arg flag (A1)
   - **ADD**: Validate oracle_confidence reflects completeness (F1)
   - **ADD**: Preserve async_modifier (.resolves/.rejects) and emit assertion_shape (A6)
   - **ADD**: For toThrow/toThrowError with no/base-class argument, downgrade to BroadError (A5)
   - **ADD**: For toMatchSnapshot, set oracle_kind = Snapshot, oracle_strength = Medium (A3)

3. **related_tests.rs filtering**
   - ✓ Filter by same_package_root if workspace_root provided (B1)
   - ✓ Prefer owner_call_relation over heuristic_relation (B2)
   - ✓ Check owner_name not shadowed (B3)
   - ✓ Verify import_source_matches_owner (B4)
   - ✓ Filter out test_mocks_owner_module (B5)

4. **actionability.rs decision tree**
   - ✓ Early return if static_limit present (H1)
   - ✓ Mark advisory if Exposed (not actionable, keep preview)
   - ✓ Mark advisory if NoStaticPath (missing_context) (B2)
   - ✓ Mark advisory if !has_oracle_eligible_relation (ambiguous_related_test) (B2)
   - ✓ Mark advisory if missing_discriminators.is_empty() (missing_target_shape) (E2)
   - ✓ Mark advisory if all evidence present but fields missing (incomplete_repair_packet) (E3, G2)
   - **ADD**: Check cross_language_oracle_visibility_unresolved (H2)
   - **ADD**: Check cross_language_test_target_unresolved (H3)

5. **package.rs discovery**
   - ✓ Detect test framework from package.json / config (C1)
   - ✓ Generate verify_command only for detected framework (C1)
   - **ADD**: If framework is unknown/guessed, set verify_command = None, mark missing (C1)
   - **ADD**: Compute receipt_command via receipt_write_command() (C2)

### Phase 2: TypeScript Adapter (After Rust Gates Pass)
Only packets that pass Rust validation reach TypeScript.

1. **Editor UI in VS Code extension**
   - Read repair_packet_ready from JSON
   - ONLY show "actionable" UI if repair_packet_ready = true
   - Display must_not_change warnings
   - Show authority_boundary and preview warnings

2. **Actionability check before offering packet**
   - Replicate Rust invariants in TypeScript (inline validation)
   - Confirm verification_commands[0] exists
   - Confirm receipt_command is not a template
   - Do NOT emit to agent if any invariant fails

### Phase 3: Agent Harness (Fallback Defense)
Even if TypeScript slips through, the agent harness guards.

1. **Before executing verify_command**
   - Confirm command exists and is executable
   - Confirm command is not in deny list (external providers, etc.)
   - Confirm output is parseable JSON (for receipts)

2. **After executing receipt_command**
   - Confirm artifact was created
   - Validate artifact matches schema
   - Only record "success" if artifact is valid

---

## Testing Strategy

### Unit Tests (Rust)
- **oracle.rs**: 10+ tests for dynamic/literal expected values, async modifiers, error variants
- **related_tests.rs**: 10+ tests for same_package_root, owner_call_relation, import verification, shadowing
- **actionability.rs**: Tests for each actionability_category (static, observed, context, relation, shape, incomplete)
- **package.rs**: Tests for verify_command generation per framework (jest, vitest, bun, node)
- **validate_agent_gap_record_packet**: 15+ failure mode tests (missing receipt, bad projection, preview gate, etc.)

### Integration Tests
- E2E: Full TS finding → GapRecord → packet → agent execution
- Monorepo: Test with multiple packages; reject cross-package relations
- Cross-language: Inject external-language test as related; verify rejection
- Dynamic matcher: Inject `expect(x).toBe(getSomeValue())`; verify rejection
- Mock + snapshot: Inject both oracle types; verify advisory state

### Adversarial Harness
- Inject malformed GapRecords (empty receipt, template variables, broad edit surface)
- Inject crafted TypeScript findings (fake oracle_strength, guessed framework)
- Attempt to bypass invariants (set repair_packet_ready = true in JSON, bypass validation)
- Measure: 0 false positives (no unsound packets), 0% over-emit rate

---

## Conclusion

**No TypeScript repair packet should be marked actionable (`repair_packet_ready = true`) unless ALL 20 fail-closed invariants hold.** Each invariant is a guard against a distinct failure mode. Violations must be caught early (Rust validation), reported clearly (evidence), and escalated to advisory-only status (safe fallback).

**The cardinal principle**: A wrongly-emitted actionable packet (one that is NOT actually safe to delegate) is the cardinal sin. Erring on the side of caution — marking more findings advisory than necessary — is acceptable. Erring on the side of over-emit (marking unsafe packets actionable) is not.
