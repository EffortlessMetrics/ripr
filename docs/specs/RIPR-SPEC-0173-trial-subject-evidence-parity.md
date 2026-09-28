# RIPR-SPEC-0173: harness trial subject evidence parity

Status: proposed

Owner:

Created: 2026-09-03

Linked proposal:

Linked ADRs:

Linked plan:

Linked issues:

- #3603 (finding: trial subject evidence parity — helper-callback bodies
  and method unwrap/expect oracles)
- #3604 (finding: dead trial construction still claims a subject — make
  the named-invocation reachability boundary explicit)
- #3636 (capability: fail-closed reachability authority for
  custom-harness trial subjects)
- #3532 (the harness registry and libtest_mimic adapter this parity
  contract binds)

Linked PRs:

Support-tier impact:

- No tier change. The parity contract widens which parsed evidence a
  registered trial subject claims inside the existing static analysis;
  it adds no runtime evidence, no harness execution, and no new
  analysis surface. [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)
  remains the tier authority.

Policy impact:

- The classified seam-cache generations bump with any semantic change to
  trial subject evidence (CACHE_SCHEMA_VERSION and its sharded/compact
  twins), so pre-change caches cannot serve evidence-blind
  classifications.

## Problem

A registered libtest_mimic trial subject is one named invocation, but
what the trial exercises is not the invocation text — it is the body of
the callback the invocation hands to the harness, plus whatever the
callback body asserts. The first adapter generation scanned only the
invocation argument span, so two under-emit gaps opened relative to an
ordinary `#[test]` that reaches the same behavior:

1. `Trial::test("name", helper_fn)` recorded no call, oracle, or literal
   evidence from `helper_fn`'s body — production behavior reached only
   through the callback stayed unobserved by that subject.
2. Ordinary `#[test]` parsing records `.unwrap()`/`.expect()` method
   calls as smoke evidence; the trial token scanner recorded only
   assertion macros, so trial subjects understated observation evidence
   relative to equivalent ordinary tests.

Both directions under-emit (fail closed), and the fix must not open the
opposite failure: crediting a same-named function the callback does not
actually name is the token-coincidence false-`exposed` family.

A third, opposite-direction boundary was left implicit by the first
adapter generation (#3604): the token scan establishes a subject for any
anchored `Trial::test("name", ...)` in the registered target, including
constructors in dead construction — an unused helper, an `if false`
branch, or a collection never passed to the harness's run entry point.
Calls and oracles inside such dead constructors join the subject and
enter the executable-test denominator although the harness never
registers or executes the trial. The discovery scan is intentionally
syntactic (trials collected inside macro token trees carry no parsed
expression nodes), so #3604 named the over-credit boundary on the claim
rather than absorbing it silently. #3636 closes that boundary from the
other side with a bounded, fail-closed reachability authority: anchor
the registered run entry point, resolve its trial argument through the
forms a token scanner can establish, and split today's uniform credit
into provably-reachable (admitted, no disclosure), unknown (admitted,
aggregate disclosure), and provably-unreachable (subject fact and claim
retained, executable-test fact withheld, typed limitation naming the
trial). The authority must bias hard toward unknown: a false
unreachable silently drops a real subject, which is worse than a
disclosed unknown.

## Behavior

A named-invocation subject keeps its identity span and body — the
registration invocation — while its `calls`, `assertions`, and
`literals` widen over exactly the code the subject exercises. Two
widening directions, each fail-closed:

- a bare-identifier callback (`Trial::test("name", helper_fn)`) that
  resolves to exactly one function in the registered target contributes
  that function's parsed body evidence one level deep, with real line
  attribution. Resolution is admitted only when binding identity is
  provable: no local binding of the name inside the trial's enclosing
  body (`let`, parameter, closure or loop pattern, or a fn-local `use`),
  no top-level `use` binding the same leaf name, and exactly one
  same-named function that is a top-level `fn` item of the file. A fn
  that lives only inside a nested module or impl is not name-visible to
  the invocation and never admits. Transitive callee bodies stay
  unclaimed — one level is the contract;
- method-position `.unwrap()`/`.expect()` calls inside the claimed span
  register smoke oracles (`SmokeOnly`/`Smoke`) with the receiver
  expression as text and real source line attribution — the same
  evidence the ordinary `#[test]` parser records for
  `ast::MethodCallExpr`. Keyword receiver participants (`self`, `Self`,
  `await`, `crate`, `super`) are ordinary postfix receivers, so
  `self.value().unwrap()` never truncates to `value.unwrap()`. The
  receiver walk crosses only balanced bracket groups, generic argument
  groups, literals, identifiers, those keywords, and `.`/`::`
  connectors, and never reaches across a statement boundary.

Macro input is skipped wholesale for method scanning (assertion macros
still classify themselves), matching what parsed method-call nodes could
see on the ordinary path: a method token inside `println!(...)` never
classifies. Qualified assertion invocations slice from the full
contiguous macro path (`insta::assert_snapshot![...]`, including `crate`,
`self`, and `super` segments), byte-for-byte like the ordinary parser's
macro-call slice. A dormant `macro_rules!` definition inside the claimed
span or inside a helper body is a template that never executes: nothing
inside it — assertion macro, method call, call fact, or literal — joins
the subject's evidence, at the token level (a definition inside another
macro's token tree carries no node to query) and at the file level (the
same ancestor authority that keeps dormant templates from becoming
subjects), and the template's source span is erased from every merged
lexical extraction so live same-line evidence survives. Lexical call and
literal extraction masks comments (line and nested block) and string
contents before scanning, so commented-out code is never evidence.
Closures, path callbacks, unresolved names, ambiguous names, and
shadowed names contribute nothing beyond the invocation span.

### Named-invocation reachability boundary and authority (#3604, #3636)

`HarnessSubjectClaim::NamedInvocation` is a syntactic claim bounded by
the registered target: a named invocation exists in the registered
target. It does not claim the harness registers or executes the trial,
and it stays uniform across reachable, unknown, and unreachable
constructions — there is no per-subject reachability field, because the
unknown bucket is exactly the case where per-subject attribution is not
established. What the claim contributes to the executable-test
denominator is decided by a separate bounded authority
(`harness_registry/reachability.rs`):

- Anchor: the registered marker path spelled qualified
  (`<marker>::run`) or a bare `run` bound by a top-level import from
  the marker — the same import-resolution machinery the trial scanner
  uses. Method-position `x.run(..)` never anchors; calls inside dormant
  `macro_rules!` templates never anchor; a bare `run` with conflicting
  imports never anchors.
- Supported resolution forms, from the run call's second argument:
  direct trial-construction containment (`vec![..]`/array literals,
  including trials collected inside other macros' token trees);
  `&`, `&mut`, `vec![..]`, `[..]`, and `local[..]` container peeling;
  immutable let-bound chains at block depth zero in the same function
  body bound before the run call (`mut` bindings, duplicate bindings,
  and missing bindings fail closed); and one level of builder-function
  resolution under the same fail-closed gates as the callback resolver,
  with a token accountancy inside the builder body. The hop budget is
  bounded (eight); exhausting it fails closed.
- Verdicts: a trial visible inside a resolved span is admitted with no
  disclosure; everything the resolver can neither connect nor exclude
  is admitted and disclosed by one aggregate
  `registration_reachability_unknown` limitation naming the trials;
  exclusion requires proof — no supported run entry call exists in the
  target at all, or every anchored run argument resolved completely and
  the trial is not in the union — and is recorded per trial as
  `registration_unreachable`. An unsupported entry spelling
  (`run_tests`), an unanchorable bare `run`, a bare `run` bound from a
  non-marker path (possibly a re-export), or an aliased `run` import
  invoked under its local name keeps every trial admitted and
  disclosed; the authority never concludes absence in
  their presence.

The boundary stays named, not silent: it is stated on the claim
variant, in `docs/OUTPUT_SCHEMA.md`'s `subjects[].claim` and
`limitations[]` fields, and here.

## Non-Goals

- No assertion-form parity beyond trial subjects: #3284 owns
  assertion-form parity generally, and this spec only binds the
  libtest_mimic trial scanner to the same method-oracle evidence the
  ordinary parser records. RIPR-SPEC-0153 keeps assertion-form parity
  out of its contract, and this spec does not restore it there.
- No transitive helper bodies: one level deep is the boundary; a
  callee-of-the-callback stays unclaimed.
- No name-heuristic crediting: resolution consumes producer-owned
  function facts and parsed structure only, and every ambiguity fails
  closed.
- No strength promotion: helper-body evidence reuses the ordinary
  parser path, and method oracles are smoke-strength by construction;
  no oracle may claim a strength the underlying evidence cannot
  support.
- No reachability beyond the bounded run-argument forms above
  (#3636): re-binding through arbitrary expression shapes, method-call
  builders, `mut` collection mutation chains, more than one
  builder-function level, branch reachability of the run call itself
  (`if false { run(..) }` is treated as an anchored entry), and
  aliased or re-exported run entries are not resolved — every such
  shape fails closed to the aggregate unknown disclosure, never to an
  exclusion. The token-stream scope also stays: trials inside macro
  token trees carry no parsed expression nodes, so resolution works on
  token spans and text-level shapes only.
- No mutation execution, no coverage claims, and no vocabulary beyond
  the static finding contract.

## Required Evidence

- a discriminating positive: an unshadowed top-level helper callback's
  call, macro-oracle, smoke-oracle, and literal evidence joins the
  subject with real source lines, and the mirrored `TestFact` carries
  the same evidence;
- negative controls proving each fail-closed boundary: a local `let`
  closure shadowing a same-named file-level fn, a same-named fn
  parameter, a fn that exists only inside a nested module, and a
  top-level `use` binding the callback's leaf name — none of the
  same-named fn's body evidence may be claimed while the subject still
  classifies;
- a one-level boundary pin: the transitive callee's body stays
  unclaimed;
- a discriminating method-oracle positive: `.unwrap()` and `.expect()`
  inside a trial body register `SmokeOnly`/`Smoke` oracles with
  receiver-ful text and exact source lines, while the assertion-macro
  oracle on the same subject carries its real source line;
- keyword-receiver positives: `self.value().unwrap()`,
  `self.value().await.unwrap()`, and a chained form carry the full
  receiver text, exact lines, and observed receiver tokens;
- fail-closed method-oracle controls: a struct field named `expect`, a
  path-shaped `Result::unwrap(...)` call, and method tokens inside
  non-assertion macro input never classify;
- dormant-template controls: a `macro_rules!` definition inside a trial
  closure and inside a helper body contributes no oracle, call, or
  literal from the template (the full expected evidence set, exactly)
  while the subject still classifies and live surrounding helper
  evidence still admits; the template spans and masking are pinned as a
  mechanism, and live calls, literals, and assertions sharing the
  template's line survive;
- comment-masking controls: block-commented calls and numbers inside a
  helper body contribute no evidence while live surrounding evidence
  still admits;
- qualified-path parity: `insta::assert_snapshot![...]` and
  `crate::snap::assert_json_snapshot!(...)` oracle texts carry the full
  macro path and match the ordinary parser's slice;
- a warm-cache regression: a classified-seam envelope seeded under the
  previous generation must miss the current generation's key with
  identical identity fields, and the schema-generation pin moves in the
  same PR as the semantic change;
- a dead-construction pin (#3604, evolved by #3636): a trial
  constructor in an `if false` branch and one in a helper nothing
  calls, in a target with no run entry call, still classify, still
  claim `named_invocation`, and still carry their constructor evidence —
  while their executable-test facts leave the denominator and a
  per-trial `registration_unreachable` limitation names each of them;
- admission pins (#3636): a direct `vec![]` argument, an array-literal
  argument, an immutable let-bound chain, a `&local[..]` index, and a
  one-level builder function all admit their trials with no reachability
  limitation, while a dead constructor beside a completely resolved run
  argument is excluded by the complete-resolution proof;
- disclosure pins (#3636): a `mut` push-built collection and an
  unsupported `run_tests` entry spelling keep their trials admitted in
  the denominator with one aggregate
  `registration_reachability_unknown` limitation naming the trials and
  its reason, and never degrade into an exclusion; so do an aliased
  run entry (`use libtest_mimic::run as execute;` plus an `execute(..)`
  call), a bare `run` re-exported from a non-marker path, a builder
  with an early `return`, and a builder with a conditional at body
  depth zero (#3639 review);
- a warm-cache regression (#3636): the reachability exclusion is
  identical on the file-fact-cache warm path (the registry re-applies
  after cache load), and the classified-seam generations bump so a
  pre-#3636 envelope cannot serve the old denominator.

## Acceptance Examples

- `Trial::test("beta", check_beta)` with `fn check_beta()` calling
  `parse_config` and asserting: the `beta` subject (and its `TestFact`)
  observes `parse_config` with the helper's real line and claims the
  helper's assertion and smoke oracles.
- The same registration with `let check_beta = || ...;` above it: the
  subject classifies but claims none of the file-level `check_beta`'s
  body.
- A trial body calling `parse_port("8080").unwrap()`: the subject gains
  a smoke oracle whose text is the full receiver chain, on the call's
  real line — the same evidence an ordinary `#[test]` with the same
  body would carry.
- `self.value().await.unwrap()` inside a trial: the oracle text is the
  full chain, not `value().await.unwrap()`.
- A registered harness workspace analyzed on a pre-change cache: the
  generation bump forces a miss, so the widened evidence is recomputed
  rather than served stale.
- A registered target whose trials are built only inside an `if false`
  branch and in a helper nothing calls, with no run entry call (#3604,
  #3636): both subjects still classify as `named_invocation` and carry
  their constructor evidence, but neither enters the executable-test
  denominator, and each is named by a `registration_unreachable`
  limitation.
- The same target with a let-chained `vec![..]` run argument (#3636):
  the chained trials are admitted with no disclosure while a third
  trial built in a helper nothing calls is excluded and named — the
  complete resolution of the run argument is the proof.
- A run argument that is a `mut` collection built by `push` (#3636):
  the trial stays in the executable-test denominator and one aggregate
  `registration_reachability_unknown` limitation names it; nothing is
  excluded.

## Test Mapping

- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::helper_callback_bodies_contribute_one_level_of_subject_evidence`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::trial_method_unwrap_expect_oracles_carry_real_lines_and_receivers`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::shadowed_callback_names_fail_closed`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::trial_method_oracle_receivers_carry_keyword_and_chained_forms`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::given_dormant_macro_rules_template_when_trial_scanned_then_no_smoke_oracle_from_template`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::given_dormant_macro_rules_template_in_helper_callback_then_no_oracle_either`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::dormant_template_spans_cover_the_parsed_definition_and_mask_exactly`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::given_block_commented_helper_evidence_then_calls_and_literals_stay_inert`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::given_one_line_dormant_macro_then_live_same_line_evidence_survives`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::trial_qualified_assertions_keep_the_full_path`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::given_dead_construction_then_subjects_still_claim_named_invocation`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::given_run_entry_then_supported_argument_forms_admit_trials`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::given_dead_constructor_beside_resolved_argument_then_it_leaves_the_denominator`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::given_unresolvable_run_argument_then_trials_stay_admitted_and_disclosed`
- `crates/ripr/src/analysis/facts/harness_registry/tests.rs::warm_file_fact_cache_applies_reachability_identically`
- `crates/ripr/src/analysis/seam_cache.rs::tests::previous_generation_classified_seam_envelope_with_identical_identity_is_a_miss`
- `crates/ripr/src/analysis/seam_cache.rs::tests::schema_versions_pin_the_role_composition_generation`

## Implementation Mapping

- `crates/ripr/src/analysis/facts/harness_registry.rs` —
  `apply_libtest_mimic_target` (evidence merge, pending-subject
  admission) and `admit_pending_subjects` (denominator admission and
  limitation recording);
  `bare_ident_callback`, `resolve_helper_function`,
  `enclosing_function`, `enclosing_body_binds_name`,
  `parser_oracles_for_node_tokens` (method oracles, macro-input skip),
  `receiver_start_index` (keyword receiver participants)
- `crates/ripr/src/analysis/facts/harness_registry/reachability.rs` —
  the bounded reachability authority (#3636): run-entry anchoring
  (`match_run_path`, `resolve_run_binding`), argument resolution
  (`Resolver::resolve_argument`, `resolve_builder_body`,
  `peel_containers`, `depth_zero_let_bindings`), and the verdict map
- `crates/ripr/src/analysis/facts/model.rs` — the evidence-boundary and
  named-invocation reachability-boundary (#3604, #3636) documentation on
  `HarnessSubjectFact` and `HarnessSubjectClaim::NamedInvocation`
- `crates/ripr/src/analysis/seam_cache.rs` — classified/sharded/compact
  schema-generation bumps (the #3634-era `1.4` -> `1.5`, `0.10` ->
  `0.11`, `0.11` -> `0.12` transitions, then the #3636-era `1.5` ->
  `1.6`, `0.11` -> `0.12`, `0.12` -> `0.13` transitions) and the
  warm-cache regressions for both generation steps

## Metrics

- unit_test_pass_rate
- golden_fixture_pass_rate
