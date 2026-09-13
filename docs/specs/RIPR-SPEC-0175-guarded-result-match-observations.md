# RIPR-SPEC-0175: guarded Result match observations

Status: proposed

Owner:

Created: 2026-09-10

Linked proposal:

Linked ADRs:

Linked plan:

Linked issues:

- #3709 (derive owner-bound guarded Result observations)
- #3727 (parser-backed shadow/item facts for seam-callee defeat — Slice A)
- #1528 (downstream qualification parent; the #13162 comparison)
- #3284 / RIPR-SPEC-0154 (the Err-return guard family this extends; that spec
  excluded match-arm Err forms as a non-goal)

Linked PRs:

Support-tier impact:

- No tier change. The admit widens which test-body shapes produce oracle
  facts for an already-analyzed probe; it adds no runtime evidence, no
  harness execution, and no new analysis surface.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- `oracle_kind` gains `guarded_result_match` (bounded vocabulary addition,
  pinned in `policy/output_contracts.txt` and `docs/OUTPUT_SCHEMA.md`).
- The file-fact cache generation is bumped so warm pre-extension caches
  cannot reuse the old oracle semantics.
- #3727 Slice A adds parser-produced `nested_fn_names`/`let_bindings` fact
  fields to `FunctionFact`/`TestFact` and switches both shadow consumers to
  a flag-law authority (`analysis/extract/shadow.rs::ShadowAuthority`);
  another file-fact cache generation bump keeps warm pre-extension caches
  from serving parser-backed files WITHOUT the fact fields (on such files
  the flag law reads empty facts as real "no shadow", so a stale envelope
  could silently retire the lexical scanners' defeats). The classified-seam
  generations move with it (outer `1.12`, sharded `0.18`, compact `0.19`):
  the classified cache loads BEFORE any file-fact rebuild, so only its own
  generation bump can keep a warm hit from replaying pre-#3727 shadow
  classification.

## Problem

A test that guards a helper's result with an explicit Result match —

```rust
match expect_response(&mut cursor, expected_id) {
    Ok(response) => { /* exact assertions on response fields */ }
    Err(error) => {
        if !matches!(error.downcast_ref::<ParseError>(),
                     Some(ParseError::InvalidData { .. })) {
            panic!("unexpected error variant: {error}");
        }
    }
}
```

— observes the changed owner's returned `Result` directly, and its Err arm
is the result's discriminator. No producer recognized this shape: the whole
match statement was only ever captured by accident (the mock-expectation
name sniff swallowing the block when the callee happened to start with
`expect_`), it carried no binding to the callee, and the return-value seam
kept its `observation_unverified` weakly-exposed outcome even though the
suite watches the owner's result and pins the exact error variant. This is
the #13162 `expect_response` comparison shape.

## Behavior

- A bounded guarded-Result-match scanner recognizes, inside test bodies,
  `match <scrutinee> { .. }` statements where the scrutinee is a direct
  path call `path::to::callee(args)`. Method receivers, call chains,
  macros, and a trailing `?` on the scrutinee are all rejected: the
  observed result must be the callee's own. The block must hold at least
  one `Err(` arm plus at least one other arm: an `Ok(` arm (classic
  guarded match) or a catch-all arm (the guarded-routing form
  `Err(e) if <pin> => {}, rest => <loud failure>` with no `Ok` arm — the
  exact historical `expect_response` harness shape).
- The scan is the single authority for the recognized statement: the
  generic statement joiners (mock-expectation name sniff, custom-helper
  sniff) skip its start lines in both the parser-backed and lexical
  fallback paths, so the two paths agree and the accidental whole-block
  capture is retired for this shape.
- An oracle fact (`oracle_kind: "guarded_result_match"`) is emitted only
  when every Err arm both terminates and carries a recognized
  discriminator. Terminating means, under a bounded depth-0 statement
  grammar (the masked arm body splits into top-level statements), that the
  arm's FIRST control transfer — in statement order — is unconditionally
  diverging (#3731 review: an earlier successful return swallows
  everything after it, so `return Ok(..)` followed by `panic!` does not
  terminate). The accepted whole-statement forms are exactly:
  - a statement whose whole form is a `panic!`/`unreachable!`/
    `unimplemented!`/`todo!`/`bail!` invocation (optionally
    `;`-terminated);
  - a `return Err(..)` statement — in a Result-returning test the Err
    return IS the test failing. A bare `return`, a successful
    `return Ok(..)` / `return ()`, and any other returned value are NOT
    terminal (#3731 review: successful exits are not loud failures);
  - a `process::exit(..)`/`std::process::exit(..)` statement whose
    argument is a NONZERO integer literal — `exit(0)` reports success and
    a non-literal argument is not statically a failure, so both are NOT
    terminal (fail-closed);
  - the body-predicate failure form: a depth-0 `if` whose condition
    carries the negated changed-error pin (a `!matches!` variant pattern,
    a `!=`-against-variant equality, or a `.downcast::<T>()..is_none()`
    test) and whose block diverges under this same grammar; or
  - a guarded accept arm (`Err(e) if <pin> => {}`): the guard must carry
    the pin, the body must be trivial, and every catch-all arm must fail
    loudly, so a guard miss routes to an observed failure.

  A clarifying distinction between the two termination rules (#3731
  review): the Err-arm rule above decides termination by divergence, and
  the accept arm's TRIVIALITY requirement is not an exception to it — it
  belongs to the guarded-routing-form grammar. A guarded accept arm does
  not terminate the match on its own (an empty body ends the arm
  silently); it only participates in the recognized shape when a guard
  carries the pin, the body stays trivial, and a loud catch-all —
  terminating under the same divergence grammar — provides the routing
  target that observes a guard miss. The loud catch-all is the
  terminating element of the routing form; the accept arm's grammar
  constrains what may sit beside it.
  Markers merely nested inside `if`/`match`/closure blocks, `.unwrap()`/
  `.expect()` statements, `assert!` forms, and conditional failures never
  terminate — and those same shapes are neither terminal nor pinning: an
  arm that can return normally swallows the error, and crediting it would
  fabricate a discriminator. Inside the body-predicate if-form the branch
  rule is strict (#3731 review): EVERY depth-0 statement of the then-block
  — and of the else-block when present — must diverge, and no successful
  `return` may sit anywhere inside either block at any depth; a successful
  return beside the panic is an escape path that swallows the matched
  error, so the form is not terminal and the condition's pin does not
  credit (a pin-conditioned `if` whose branch returns successfully never
  credits, even when a trailing panic terminates the arm). Residual
  (documented under-credit): a bare
  `if error != Type::Variant { panic!(..) }` body terminates but does not
  pin — the pin authority does not read body inequalities as exact-variant
  identity. An arm that swallows the error, and a routing target that
  stays silent, never credit.
  Discriminators, strongest first:
  - an exact error-variant pin in binding position (the arm pattern
    proper, the pattern argument of a `matches!`/`assert_matches!` in
    the arm body or guard, or a guard equality `==`/`!=` whose compared
    operand is rooted at the arm's error binding — one operand names the
    binding (`e`, `e.kind()`), the other is the variant path, in either
    order —) ranks `strong`. Every Err arm's pin is collected into the
    fact (#3731 review): two Err arms pinning two variants carry both, so
    a changed seam matching ANY collected pin confirms. Pattern pins and
    guard pins gate the arm's SELECTION, so they always participate; a
    BODY pin counts only when it participates in the arm's divergence
    decision (#3731 review: a `let`-computed pin the control flow never
    consumes does not gate the terminal statement — the pin counts either
    when it appears inside the condition of the depth-0 `if` that guards
    the diverging statement, or when the arm's first control transfer
    references the pin's binding variable whole-word). Each pin is
    truncated individually at a documented 80-character per-pin cap and
    the joined pin list carries NO overall truncation (#3731 review: an
    overall cap dropped later variants from the fact text — and with them
    from reveal/repo parsing);
  - a concrete `.downcast[_ref|_mut]::<Type>()` pin ranks `medium`, and
    only when the cast's OWN result is observed (#3731 review round 4:
    the observer must bind to the invocation, not to any token in the
    statement) and the cast participates in the arm's divergence decision
    (a cast computed into an unconsumed `let` binding is dead
    computation, not a pin — #3731 review). Observation means the text
    immediately after the
    invocation's call-closing paren starts a boolean inspection
    (`.is_ok()`, `.is_err()`, `.is_some()`, `.is_none()`) or an observing
    unwrap (`.expect(`, `.unwrap(` — both panic on the wrong type, so
    they observe by construction), or the statement wraps the invocation
    in a whole-word `matches!`/`assert!`/`assert_eq!`/`assert_ne!`
    (whole-word, so `debug_assert!(` and `assert_matches!(` do not read
    as wrappers). `.map(`/`.map_err(` deliberately do NOT observe: they
    convert without inspecting. A discard binding (`let _ =` /
    `let _: Type =` before the invocation) observes nothing by
    construction. ALL downcast invocations in the arm participate: a
    discarded first cast no longer hides a later observed one, and the
    first OBSERVED, participating cast supplies the pin text. Residual
    (documented
    under-credit): a cast whose result flows into a variable that a
    LATER statement observes is not credited — parser-backed observation
    rides #3727.
  Every Err arm must pin: one pinned arm beside an unpinned escape arm
  is not an exact identity. Wildcard `Err(_)` arms, opaque predicates,
  and message-only variant mentions never pin: exactness is not inferred
  from names or payload text.
- A same-named local `fn` or `let` binding in the test body defeats the
  oracle for a BARE one-segment scrutinee (the shared #3714 shadow
  authority): a shadowed name is not the resolved callee. A qualified
  scrutinee (`helpers::parse`) cannot be shadowed by a local binding, so
  the defeat does not apply to it; its owner confirmation stays
  unverified downstream (#3727 tracks qualified-path identity
  resolution).
- #3727 Slice A gives the shadow authority a PARSER-BACKED decision path
  with the same rules at line granularity. The parser producer
  (`analysis/syntax/ra.rs`) descends each function's syntax tree and
  records `nested_fn_names` (names of `fn` items nested in the body,
  hoisted — they defeat the whole body) and `let_bindings` (one
  `{line, name}` entry per whole-word identifier in every initialized
  `let` binding's pattern, `line` body-relative and 0-based; the
  initializer-less `;` bound and the positional at-or-after rule carry
  over unchanged). Both fields live on `FunctionFact` and mirror on
  `TestFact`; the lexical fallback producer leaves them empty, mirroring
  `probe_shapes`. Consumers switch on the FILE'S `used_lexical_fallback`
  FLAG — never on set emptiness: parser-backed files decide from the
  facts (empty sets are real "no shadow" results); fallback files, and
  files absent from the index, run the byte-level lexical scanners
  byte-identically (`ShadowAuthority::LexicalMaskedBody`). Equivalence
  law: on parser-backed files the fact-derived decisions must equal the
  lexical scanners' decisions on the same masked body — pattern names are
  extracted with the scanner's own whole-word ASCII vocabulary, so
  realistic Rust decides identically through either authority (a pinned
  fixture battery enforces it). Named residuals: shadow shapes inside
  macro definitions or exotic whitespace spellings are real syntax or
  text the OTHER authority sees but this one does not (macro token trees
  carry no `fn`/`let` nodes; the byte scanner can see whole-word shapes
  in non-ASCII identifier spellings the ASCII fact vocabulary does not
  tokenize), and the lexical byte scanner also defeats on a `let`/`fn`
  keyword split across lines that real parsing accepts — all
  under-defeat/over-defeat residuals bounded to inputs outside the
  equivalence battery, never a vocabulary or outcome change.
- The Ok-arm bodies are sliced at extraction time and the fact carries
  whether they OBSERVE the unwrapped success value (`ok_value_observed`;
  #3731 observation authority): the decision is true when any Ok-arm body
  contains an assertion form (`assert`, covering
  `assert!`/`assert_eq!`/`assert_ne!`/`assert_matches!`), a `matches!`
  invocation, an equality/inequality, or an unwrap-family inspection
  (`.is_ok()`, `.unwrap(`, `.expect(`) — over the MASKED bodies, so a
  string- or comment-embedded marker never counts. The synthesized text
  keeps its `Ok(..) => ..` template (output-contract stability), so the
  decision rides the fact and cannot be re-derived from the text.
  Bounded residuals, both documented: a payload observed only OUTSIDE the
  arm — an assignment flowing out of the arm, asserted in a later
  statement — is invisible to the arm-body rule, so the fact reports
  unobserved and the confirmation fails closed (parser-backed arm
  observation rides #3727); an equality on a value OTHER than the
  unwrapped payload can satisfy the containment rule, because operand
  resolution is exactly what the lexical view cannot do.
- In reveal classification, the fact confirms observation for
  `error_path` and `return_value` probes whose changed owner's bare name
  is the scrutinee callee, without any changed-line token overlap, under
  five fail-closed gates (#3731 review): the oracle text must embed a
  BARE one-segment scrutinee (`match <owner>(..)` — a qualified path's
  identity is unresolvable at name level, so a qualified same-named
  callee never confirms); when the changed expression constructs an
  exact error variant, the guarded pin must name that exact variant — a
  sibling-variant or type-only guard leaves the observation
  `observation_unverified`, and the shared enum-qualifier token is not a
  specificity signal; the related test's file must not import the
  owner callee's bare name from a FOREIGN path — a `use` declaration
  whose first path segment is neither `crate`/`self`/`super` nor one of
  the analyzed crate's own names makes the bare binding
  ambiguous (`use other_crate::expect_response;` defeats; the normal
  own-crate integration-test binding `use this_crate::expect_response;`
  does not, and an `as` alias binds the alias, not the name); the
  test's own package must not define a function with the callee's bare
  name while the changed owner lives in another package (#3731 review:
  index-backed through the workspace's indexed functions and the shared
  package-scope authority — the bare call in that test may bind the
  local definition, so the confirmation is refused; both package scopes
  must resolve, and an unscopable side keeps today's behavior); and —
  for a return-value probe whose changed value is the SUCCESS payload
  (no exact Err construction) — the match's Ok arm must observe the
  unwrapped value (#3731 observation authority): a guarded-routing form
  with no Ok arm (the success value flows into a trivial catch-all) and
  a payload-ignoring `Ok(_) => ..` arm never observe a changed Ok value,
  so those confirmations are refused (fail closed, under-credit). A
  return-value probe on an exact Err construction keeps the Err-guard
  discriminator — the pin gate names the changed variant — the same
  principle that leaves ErrorPath probes independent of Ok-arm
  observation. The own
  names are the root manifest's `[package] name` plus its `[lib] name`
  target when declared (#3731 review: integration tests import the lib
  target), each admitted in raw and crate-identifier form — hyphens
  normalize to underscores, so package `foo-bar` admits
  `use foo_bar::..;`. The import scan is bounded and lexical and covers
  ALL `use` declarations in the test's file at any brace depth —
  file-level, module-nested (`mod tests { use other::expect_response; }`,
  the historical harness shape), and function-local (#3731 review: a
  nested foreign import bypassed a file-level-only scan); scanning past
  module boundaries can defeat a confirmation the import is not visible
  to, a documented under-credit residual, since lexical scope resolution
  is what the scan cannot do. Globs (`use p::*;`), re-export
  chains, and workspace-member manifests are not resolved — the residual
  ambiguity they can hide is documented under-credit/over-credit risk
  that parser-backed import resolution retires (#3727). The fact's text
  embeds the scrutinee callee, so the binding is same-entity by name.
  Effect families (side effect, call deletion) and families whose changed
  behavior need not flow through the matched result keep their existing
  observers. Wrong-owner facts stay non-confirming; their only
  association remains the pre-existing generic (weak) rules. In
  repo-mode grading, the guarded oracle kind-matches error and
  return-value seams and `oracle_discriminates_seam` applies the
  exact-variant rule plus a CALLEE-IDENTITY gate (#3731 review rounds 4
  and 5): the synthesized text's scrutinee must be BARE and exactly equal
  the seam's owner terminal name — the same bare-only rule the reveal
  side applies; a qualified scrutinee whose terminal segment matches
  (`other_crate::parse` over an owner named `parse`) is the
  token-coincidence family — so a guarded match over a different callee
  observes someone else's result and never discriminates, no matter which
  variant it pins; an unrecognizable or qualified scrutinee fails closed.
  A return-value seam whose changed value is the SUCCESS payload also
  requires the fact's observing-Ok-arm decision (the same fail-closed
  rule as the reveal side); an `ErrorVariant` seam and a return-value
  seam on an exact Err construction are unchanged — the Err guard is the
  discriminator there.
- Strength maps through the existing stage authority: `strong` credits
  discrimination (`exposed` when reach, infection, and propagation also
  hold); `medium` keeps the seam below `exposed` with observation
  confirmed and the typed weakness stated. The propagation and infection
  layers are unchanged and still gate the class independently.

## Non-Goals

- No credit from a bare `?` on the scrutinee, a terminal `Ok` alone, nearby
  tokens, diagnostic strings, or unresolved/custom macros.
- No recognition for variable-bound scrutinees (`let r = f(..); match r`),
  method-call scrutinees, or `Option` matches; those keep their existing
  meaning.
- No change to the propagation, infection, or reach authorities; a weak
  propagation witness still holds the finding at `weakly_exposed`.
- No per-language variants beyond Rust; no generalized semantic engine.
- No mutation-runtime outcome vocabulary.
- #3727 Slice A non-goals (later slices or refinements): wrapper-seam
  callee identity from probe-expression provenance (Slice B), a
  parser-derived `CallFact` with receiver/path/column (Slice C), lexical
  scope extents or column-precise shadow attribution, and any change to
  the shadow semantics that would move a golden — the fact path is
  deliberately scanner-equivalent, not more precise.

## Required Evidence

- `fixtures/guarded_result_match_owner_observation`: the real-consumer
  reduction of the classic form (guarded Result match on the changed
  owner with exact-variant downcast Err pin and exact-field Ok
  assertions) reaching `exposed` on the producer-owned error-path seam,
  with the guarded-match oracle named in the related-test evidence.
- `fixtures/guarded_result_match_positive`: the historical #13162
  routing form (guarded accept arm with an exact error-variant equality
  routed by a loud catch-all, no `Ok` arm) plus the classic pattern-pin
  form; both probes reach `exposed` with the producer-owned oracle.
- `fixtures/guarded_result_match_fail_closed`: wrong-owner guard,
  variable-binding scrutinee, shadowed callee, message-only error
  predicate, and swallowed-error arm — all non-crediting; findings stay
  at `weakly_exposed`.
- `fixtures/guarded_result_match_swallowed`: the owner-correct harness
  whose Err arms only log or panic through a wildcard — findings stay at
  `weakly_exposed` with the missing discriminator named.
- `fixtures/guarded_result_match_sibling_variant`: the owner's changed
  line constructs `Err(ParseError::InvalidData)` while the only guarded
  match pins the sibling `Err(ParseError::UnexpectedEof)` on a bare
  scrutinee — findings stay at `weakly_exposed` with the observation
  unverified (never `exposed`).
- `fixtures/guarded_result_match_conditional_failure`: guarded matches
  whose pinned Err arms can return normally (conditional panic,
  unrelated unwrap, closure-nested panic) — no oracle is emitted and
  findings stay at `no_static_path`.
- Honesty-corpus cases pinning the positive controls
  (`rust_guarded_result_match_routing_positive_control`,
  `must_promote`) and the negative controls
  (`rust_guarded_result_match_swallowed_no_promotion`,
  `rust_guarded_result_match_fail_closed_shapes`,
  `rust_guarded_result_match_sibling_variant_no_promotion`,
  `rust_guarded_result_match_conditional_failure_no_promotion`,
  `must_not_promote`, `maximum_class weakly_exposed`).
- In-crate scanner tests: positive shapes (classic and guarded-routing),
  downcast-only medium, Err pattern pins, guard-equality pins, wildcard/
  no-op arms, silent catch-alls, unguarded trivial arms, neighbor-arm
  text isolation, multi-arm unpinned rejection, message-only diagnostics,
  non-direct scrutinees, shadow defeat, comment/string immunity,
  conditional-panic / conditional-return / unrelated-unwrap /
  closure-nested non-termination, successful exits (`return Ok(())`,
  bare `return`, `return ()`, `exit(0)`, non-literal exit) never
  terminating while `return Err(..)` and nonzero exits stay terminal,
  observer-binding (another value's observer, whole-word wrapper names,
  `.map`/`.map_err` conversions, discard-with-inspection), all-
  invocation participation (a later observed cast pins after a discarded
  first cast), `.expect(` observation, adversarial statement windows
  (closure arguments, nested brackets, string-embedded observer text),
  negated-pin condition termination, first-comma `matches!` pattern
  slices, bare-only shadow scoping, computed-but-unconsumed body pins
  (`matches!` and observed downcast into an unconsumed `let`) never
  pinning while a pin the decisive statement consumes (or a pin inside a
  diverging if's condition) stays credited, successful returns beside or
  ahead of the panic inside a body-predicate if-form disqualifying the
  form and its pin while an all-diverging branch stays terminal, per-pin
  truncation controls (two long pins both surviving the untruncated
  join, one over-cap pin truncating alone), and the Ok-arm observation
  decision (a routing form and a payload-ignoring `Ok(_) => {}` arm
  report the success value unobserved, an asserting Ok arm reports it
  observed, a string-embedded marker never observes, and a payload
  asserted only after the match stays outside the bounded rule).
- In-crate reveal tests: owner-bound confirmation without token overlap,
  wrong-owner non-confirmation, type-pin weakness, effect-family refusal,
  sibling-variant non-confirmation with its exact-variant positive
  control, qualified-scrutinee non-confirmation, foreign same-name import
  defeat with its no-import/own-crate-import/aliased-import controls,
  cross-package same-name-function defeat with its same-package and
  unscopable-path positive controls, and the Ok-arm observation gate for
  success-payload return-value probes (routing-form and
  payload-ignoring-arm non-confirmation, observing-arm confirmation,
  ErrorPath and Err-construction independence controls).
- Repo-grading tests: guarded-match kind matching for error and
  return-value seams, exact-pin discrimination with sibling/type-only
  rejection, wrong-callee non-discrimination with its own-callee and
  qualified-scrutinee controls, exemplar nomination for the new kind, and
  the observing-Ok-arm requirement for success-payload return-value seams
  (routing-form and payload-ignoring rejection, missing-decision fail
  closed, observing-arm and Err-construction controls).
- Parser-path test: exactly one guarded-match oracle for the routing
  form and no mock-expectation duplicate through
  `extract_parser_oracles`.
- #3727 Slice A scanner-equivalence battery: for fixture bodies covering
  the #3722/#3728 pin families (string/comment/char-literal/multiline-
  string shapes, initializer-less `;` bound, positional `let` rule,
  same-line conservative defeat, `let mut`/`ref`/typed/destructuring/
  let-else patterns, nested fns and nested blocks, prefix-coincidence
  names), the parser fact decisions and the lexical scanner decisions on
  the same masked body agree for every probed callee, through both the
  descent helper and the real `summarize_file_with_parser` test facts;
  the guarded-match scan driven with parser facts emits exactly what the
  lexical entry point emits on the same bodies.
- #3727 Slice A flag-law and removal controls: a parser-backed file's
  facts defeat the shadowed seam-callee admit; deleting the facts (file
  absent from the index) or marking the file `used_lexical_fallback`
  (fact fields empty) falls back to the lexical scanners and still
  defeats — no over-credit; a parser-backed file with genuinely empty
  fact sets keeps a clean callee call related (emptiness is a real
  "no shadow", never a discriminator); a producer control requires
  non-empty facts for a shadow-bearing body; and the parser path defeats
  a fact-shadowed guarded match after attribute lines (line-index
  alignment), while the lexical fallback path keeps its existing
  shadow-defeat pins.

## Acceptance Examples

- Accept: the `expect_response` shape above credits `exposed` for an
  error-path/return-value seam of `expect_response` when reach, infection,
  and propagation hold.
- Accept: the guarded-routing form — `Err(error) if error.kind() ==
  io::ErrorKind::InvalidData => {}` routed by `result => bail!(..)` with
  no `Ok` arm — credits through the guard's equality pin for an
  error-path seam (and for a return-value seam on an exact Err
  construction, whose pin names the changed variant).
- Accept: a return-value probe whose changed value is the success
  payload confirms only when the Ok arm observes the unwrapped value
  (`Ok(v) => assert_eq!(v, 3)`); a routing form with no Ok arm, or a
  payload-ignoring `Ok(_) => {}` arm, never confirms it — the finding
  keeps its `observation_unverified` weakness.
- Accept: a downcast-only Err guard credits observation and names the type
  pin, but the seam stays below `exposed`.
- Reject: the same match naming a different callee; a `let`-bound result
  scrutinee; a local `fn expect_response` shadow; an `Err(_)` arm; an arm
  that only logs; a silent catch-all routing target; a diagnostic string
  mentioning the variant; a guarded match pinning a sibling variant of
  the changed error; a qualified scrutinee sharing the owner's bare name;
  an Err arm whose panic fires only under an unrelated condition; an Err
  arm whose only failure action is an unrelated `.unwrap()`; an Err arm
  that ends in a bare `return`, `return Ok(())`, or `exit(0)`; an Err arm
  whose only pin is a computed-but-unconsumed `let` pin; a body-predicate
  if-form whose branches contain a successful return; a guard
  whose downcast pin is discarded or observed only through another
  value's `.is_ok()` or a `.map_err` conversion; a related test file that
  imports the owner's bare name from a foreign crate; a related test
  whose own package defines the owner callee's bare name while the owner
  lives in another package; a guarded match whose Ok arm ignores the
  payload (or has no Ok arm at all) offered as the discriminator for a
  changed success value.

## Test Mapping

- `crates/ripr/src/analysis/extract/oracles/scan.rs::guarded_result_match_tests`
- `crates/ripr/src/analysis/syntax/ra.rs::guard_pipeline_debug_tests::parser_path_credits_guarded_routing_match_in_test_facts`
- `crates/ripr/src/analysis/syntax/ra.rs::guard_pipeline_debug_tests::parser_path_shadow_defeats_guarded_match_through_facts`
- `crates/ripr/src/analysis/syntax/ra.rs::shadow_fact_equivalence_tests`
- `crates/ripr/src/analysis/extract/shadow.rs::fact_authority_tests`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_parser_backed_file_when_facts_shadow_callee_then_no_seam_callee_call`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_parser_facts_deleted_when_wrapper_probe_then_lexical_fallback_still_defeats`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_lexical_fallback_file_when_wrapper_probe_then_lexical_scanners_still_defeats`
- `crates/ripr/src/analysis/classify/related_tests.rs::tests::given_parser_backed_file_with_no_shadow_facts_then_seam_callee_call_stays_related`
- `crates/ripr/src/analysis/classify/reveal.rs::tests::guarded_result_match_*`
- `crates/ripr/src/analysis/test_grip_evidence/tests.rs::guarded_result_match_*`
- `crates/ripr/src/output/agent_seam_packets.rs::tests::kind_gate_error_variant_seam_with_guarded_result_match_strong_test_is_nominated`
- `fixtures/guarded_result_match_owner_observation`
- `fixtures/guarded_result_match_positive`
- `fixtures/guarded_result_match_fail_closed`
- `fixtures/guarded_result_match_swallowed`
- `fixtures/guarded_result_match_sibling_variant`
- `fixtures/guarded_result_match_conditional_failure`
- `fixtures/evidence-promotion-honesty-corpus/corpus.json`
  (`rust_guarded_result_match_routing_positive_control`,
  `rust_guarded_result_match_swallowed_no_promotion`,
  `rust_guarded_result_match_fail_closed_shapes`,
  `rust_guarded_result_match_sibling_variant_no_promotion`,
  `rust_guarded_result_match_conditional_failure_no_promotion`)

## Implementation Mapping

- `crates/ripr/src/analysis/extract/oracles/scan.rs` —
  `guarded_result_match_scan` and the arm/discriminator grammar, plus the
  Ok-arm body slicing and the `ok_arms_observe_value` containment rule
  behind the fact's `ok_value_observed` decision; #3727 Slice A adds
  `guarded_result_match_scan_with_shadow_authority` (the lexical entry
  point delegates with `ShadowAuthority::LexicalMaskedBody`).
- `crates/ripr/src/analysis/extract/shadow.rs` — the single shadow
  authority (moved from `classify/related_tests.rs`, then
  `extract/calls.rs`): the byte-level lexical scanners plus the #3727
  Slice A fact twins (`fact_body_defines_callee_fn`,
  `fact_body_let_shadow_line`, `fact_body_shadows_callee_at_line`,
  `extract_pattern_words`) and the `ShadowAuthority` flag-law switch both
  consumers call.
- `crates/ripr/src/analysis/extract/oracles/scan.rs` —
  `guarded_result_match_scan` and the arm/discriminator grammar, plus the
  Ok-arm body slicing and the `ok_arms_observe_value` containment rule
  behind the fact's `ok_value_observed` decision; #3727 Slice A adds
  `guarded_result_match_scan_with_shadow_authority` (the lexical entry
  point delegates with `ShadowAuthority::LexicalMaskedBody`).
- `crates/ripr/src/analysis/extract/calls.rs` — call-fact extraction
  feeding the seam-callee relation.
- `crates/ripr/src/analysis/syntax/ra.rs` — parser-path ingestion and
  statement-joiner suppression; #3727 Slice A adds the
  `collect_body_shadow_facts` descent (nested `ast::Fn` names plus
  initialized `ast::LetStmt` pattern facts, body-relative through the
  `LineIndex`), wires the parser-backed authority into
  `extract_parser_oracles`, and mirrors the fields into the
  `FunctionFact`/`TestFact` summaries.
- `crates/ripr/src/analysis/syntax/lexical.rs` — the fallback producer
  leaves the #3727 shadow-fact fields empty and sets
  `used_lexical_fallback` (#2698 flag discipline).
- `crates/ripr/src/analysis/facts/model.rs` — the `OracleFact` fact gains
  `ok_value_observed` (the guarded-match Ok-arm observation decision;
  `None` for every other oracle kind); #3727 Slice A adds
  `LetBindingFact` and the `nested_fn_names`/`let_bindings` fields on
  `FunctionFact` and `TestFact` (`#[serde(default)]`).
- `crates/ripr/src/analysis/classify/related_tests.rs` — the
  `SeamCalleeCall` relation routes its shadow decision through the
  #3727 `ShadowAuthority` flag law (facts on parser-backed files,
  lexical scanners under fallback or absent index entries).
- `crates/ripr/src/analysis/facts/build.rs` — `package_names` from the
  root manifest (the `[package] name` plus the `[lib] name` target, each
  in raw and crate-identifier form), feeding the own-crate side of the
  import gate.
- `crates/ripr/src/analysis/classify/reveal.rs` — `owner_callee` context,
  the producer-owned confirmation (including the Ok-arm observation gate
  for success-payload return-value probes), and
  `file_imports_foreign_callee_name` (the F11/F22 import-defeat scanner,
  a bounded lexical `use` scan over the masked file source at any brace
  depth).
- `crates/ripr/src/analysis/classifier/evidence.rs` — the caller-supplied
  per-test defeat closures threading `FileFacts.source` (the F11/F22
  same-name-import defeat) and `RustIndex.functions` with the shared
  package-scope authority (the cross-package same-name defeat, #3731
  review).
- `crates/ripr/src/analysis/test_grip_evidence.rs` —
  `guarded_result_oracle_matches_seam_variant` (variant comparison plus
  the scrutinee/owner callee-identity gate plus the observing-Ok-arm
  requirement for success-payload return-value seams).
- `crates/ripr/src/domain/evidence.rs` — `OracleKind::GuardedResultMatch`.
- `crates/ripr/src/analysis/seam_cache.rs` — cache generation bumps:
  file-fact 1.0 -> 1.1 for the oracle kind and statement suppression;
  1.1 -> 1.2 for the guarded-routing grammar, after a warm 1.1 cache
  demonstrably replayed the pre-routing classification on a real
  fixture; 1.2 -> 1.3 for the #3731 review grammar fixes (bounded
  depth-0 terminal arms, first-comma `matches!` slices, bare-only shadow
  scoping); 1.3 -> 1.4 for the #3731 review round-4 fixes (successful
  exits, observed-cast pins); 1.4 -> 1.5 for the #3731 review round-5
  fixes (binding-rooted guard-equality pins, first-control-transfer
  termination, every-arm pin collection); 1.5 -> 1.6 for the #3731
  review round-6 fixes (divergence-participating body pins, escape-free
  if-form termination, per-pin truncation with an untruncated join);
  1.6 -> 1.7 for the Ok-arm observation decision (Ok-arm bodies sliced
  at extraction and the fact carries whether they observe the unwrapped
  success value); 1.7 -> 1.8 for the #3727 Slice A parser-backed shadow
  facts (`nested_fn_names`/`let_bindings` plus the flag-law authority
  switch in both consumers — a warm pre-bump hit would serve parser-backed
  files without the fact fields, and the flag law reads empty facts on
  those files as real "no shadow"); classified `CACHE_SCHEMA_VERSION` 1.6 -> 1.7 -> 1.8
  -> 1.9 -> 1.10 -> 1.11, sharded 0.12 -> 0.13 -> 0.14 -> 0.15 -> 0.16
  -> 0.17, and compact 0.13 -> 0.14 -> 0.15 -> 0.16 -> 0.17 -> 0.18
  (the second-to-last steps for the round-4 through round-6 fixes,
  including the bare-only repo scrutinee gate, the nested-import defeat,
  the lib-target own-crate names, and the cross-package same-name
  defeat, and the last steps for the Ok-arm observation decision and its
  reveal/repo confirmation gates) so warm classified envelopes derived
  from pre-fix oracle facts cannot serve stale discrimination.

## Metrics

- unit_test_pass_rate
- golden_fixture_pass_rate
- oracle_kind histograms now count the guarded-match form under
  `guarded_result_match`.
