# RIPR-SPEC-0197: Owner-return pins through calls that name the owner

Status: proposed

Owner:

Created: 2026-09-29

Linked proposal:

Linked ADRs:

Linked plan:

Linked issues:

- #4478 (confirm a return value pinned by `assert_eq!` on the owner's own call)
- #5027 (share bounded equality execution admission with ErrorPath and Predicate)
- #5040 (typed async-harness execution provenance; explicit unsupported boundary)
- #3727 (parser-backed call identity; this spec adds the owner's item
  container fact, not parser-derived `CallFact`)

Linked PRs:

Support-tier impact:

- No tier change. A Rust `return_value` finding can now read `exposed`
  when a related test pins the owner's whole return value through a call
  that names the owner. No new language or surface is claimed.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new CI gate or production process surface. The matched runtime integration
  test has one allowlisted test-only process-launch site for compiling/running the
  fixed fixture subjects.

## Problem

A Rust `return_value` probe confirms its discriminator only when an
assertion shares an identifier token with the changed expression. A test
that pins the owner's exact return value through a call of the owner shares
no such token. The OSS replay of tokio-rs/bytes 7930d93 changed
`Buf::try_get_int`'s tail to `Ok(sign_extend(self.try_get_uint(nbytes)?, nbytes))`.
The commit's own test `assert_eq!(a.try_get_int(3), Ok(-1))` kills every
mutant of that line, and ripr reported `weakly_exposed` with "Discriminator
unconfirmed".

A lexical "the assertion calls something named like the owner" rule is
wrong in three shapes, each of which an earlier candidate credited:

1. An associated function `T::name` next to a same-named free function
   `name`: a bare `name(..)` in a test names the free function.
2. A trait default method whose receiver's impl overrides it: the call runs
   the override, not the default.
3. An input that leaves the owner through another return path: with
   `Ok(g(x)? * 2)` changed, `assert_eq!(f(-1), Err(E))` observes the `?`
   exit and never evaluates the changed `Ok(..)`.

## Behavior

An `assert_eq!` confirms a `return_value` probe's discriminator when all of
the following hold. Each rule fails closed: when ripr cannot establish it,
the owner-specific pin is refused. Before either the pin or token matching
can supply any return-value oracle credit, bare `assert_eq!` assertions pass
the same execution/macro-context admission below. A refused context contributes
no observation, oracle kind, strength, or discriminator; its related-test row
remains visible with no oracle. If no admitted oracle remains, existing stage
combination produces `reachable_unrevealed` with Observe `no` and Discriminate
`no`. Here `no` means no statically established oracle, not proof that a test
can never execute. Other owner-specific pin failures retain the existing token
rule only for an assertion whose context was admitted.

1. Oracle shape. The assertion is one plain `assert_eq!` (never
   `assert_ne!`, `debug_assert_eq!`, a path-qualified or crate-specific
   `*_assert_eq!`) with an exact-value or whole-object oracle kind, on a line
   inside the test's own body, in a test without `#[should_panic]`. The
   parser must match the exact indexed function body and the assertion's
   line/text identity uniquely. Identical same-line invocations fail closed.
   The test itself must have item ancestry through modules/item lists to the
   source file, never an enclosing function, block, closure or impl. Module
   outer attributes and module/source-file inner attributes must establish
   availability in a test build. The existing `cfg_predicates` owner evaluates
   `test = true`, `all`/`any`/`not` and nested `cfg_attr`; `all()` is true and
   `any()` false. Feature/target/custom atoms remain unknown and cannot grant
   credit. Ordinary lint/doc attributes and definitely enabled forms such as
   `cfg(test)` or `any(test, feature = "unknown")` remain supported. Raw attribute
   heads (including raw introduced attributes) remain unestablished; refusing
   them does not alter the cached source-role classifier. Malformed
   operands are not silently discarded. Existing module-provenance edges bind
   out-of-line parents to their indexed source and declaration coordinate;
   missing, unresolved or include-only provenance cannot establish that context.
   The assertion must lie on an ordinary statement/block/initializer path,
   without conditional, async, const, labeled, nested-item, attributed-node
   or prior root-return context. Root returns after the actual assertion execution
   point do not defeat it. For a bound closure, use the invocation position, not
   its earlier definition; any closure return remains conservatively refused.
   A non-async zero-argument closure is supported
   only when immediately invoked, or when its immutable simple binding has
   exactly one reference in the entire function: a subsequent zero-argument
   call in the binding's same live statement block. Both the binding and call
   must have an ordinary statement path to the test. Aliases, mutation, rebinding,
   conditional calls, deferred calls and nested closure chains are unknown.
   `?` in a root test remains supported (an error fails an ordinary Result
   test); `?` in a closure is refused because its result could be discarded.
   Exactly
   one compared operand is a complete call of the owner's name with nothing
   chained after it, and the other operand does not mention the owner's
   name (`assert_eq!(f(4), f(2) + f(2))` compares the owner with itself).
2. Call identity, from the parser's item-container fact on the owner
   (`FunctionFact.item`: free, local, inherent, trait impl, or trait, with
   the `self`-receiver and body flags; the lexical fallback leaves it
   unknown):
   - A bare `name(..)` names only a module-level function without a `self`
     receiver. No other module-level function of that name may exist in the
     workspace, the test must not bind the name (`let`, nested `fn`, the
     test's parameters, a `for`, closure or match-arm pattern, or a macro
     such as `let_assert!` that mentions it), and the test's file must not
     rename an item to it (`use a::b as name`).
   - A method call `recv.name(..)` names only a function with a `self`
     receiver in an `impl` or `trait` block. `recv` must be a plain local
     binding, and every `let` that binds it must be a simple
     `let [mut] recv [: T] = ..;` whose type ripr can read: an annotation,
     a struct or tuple literal (`T { .. }`, `T(..)`), or a constructor call
     whose signature returns the type: `T::default()` and `T::from(..)`
     directly, `T::try_from(..)` only after `?`, `.unwrap()` or
     `.expect(..)`, and any other `T::f(..)` only when `f` is `T`'s one
     inherent associated function of that name and declares `-> Self` (or
     `-> T`), or `Result`/`Option` of it followed by `?`, `.unwrap()` or
     `.expect(..)`; or a byte-slice expression (`&[..][..]`, `&b".."[..]`).
     A name bound by any other pattern (a closure parameter, a `for` or
     match-arm pattern, a destructuring `let`, a nested `fn`, the test's
     parameters, a macro that mentions it) leaves the type unestablished. A
     named type must be a struct, enum or union declared in the workspace,
     and the test's file must not import it from outside the workspace,
     rename another item to it, or declare a `type` alias of it.
   - The receiver type must dispatch to the owner: the inherent `impl`'s
     self type, the trait impl's self type, or, for a trait default method,
     a type with an `impl .. Trait for <type>` in the workspace. A trait
     method also needs its trait in scope: the test's file imports it by
     name from a workspace path (`crate`, `self`, `super`, a workspace
     package, or its `::`-rooted form), or declares it. A byte-slice
     receiver never credits a name `&[u8]` itself resolves (slice methods,
     prelude and `std::io` trait methods). A named receiver never credits
     a by-value prelude trait method name (`count`, `map`, `into`, ...):
     method lookup tries `T` before `&T`, so `Iterator::count(self)` takes
     `c.count()` before an inherent `count(&self)` whenever the type is an
     iterator, and ripr cannot see which std traits a type implements.
   - No other workspace definition of the name with a `self` receiver and a
     body may exist (an override, another type's inherent method, another
     trait's method), except a pure `(**self).name(..)` / `(*self).name(..)`
     forward. A definition the parser did not index (a `macro_rules!` body)
     counts when its `fn name` text is visible. A definition in a
     lexical-fallback file always counts.
3. Return path. The changed expression must be the owner body's tail (or
   its final `return <expr>;`), and it must evaluate all of its parts on
   every input: no closure or `|` operator, no `&&`/`||`, no `if`, `match`,
   loop or `break`, and no combinator that skips its argument on some
   inputs (`map_or`, `unwrap_or`, `and_then`, `then`, ...). With
   `x.map_or(0, |v| v * 3)` changed, `assert_eq!(f(None), 0)` never runs
   the changed closure. When the owner has no `?` and no other
   `return`, any pinned value came through it. Otherwise the changed
   expression must be one `Ok(..)` (or `Some(..)`) constructor, the only
   one in the body, every other `return` must build `Err(..)` (or `None`),
   and the pinned value must itself be `Ok(..)` (or `Some(..)`). An owner
   body that invokes any macro outside a fixed non-returning set
   (`assert!`, `format!`, `panic!`, `vec!`, ...), however it is spaced
   (`ensure !(..)`), leaves the return paths unestablished.
4. Macro binding. The existing non-returning standard-macro vocabulary is
   reused for test bodies; any other invoked macro may hide an early exit
   and refuses the pin. Only bare standard names qualify; qualified paths stay unknown because
   their roots can be rebound.
   Escape keywords or nested/ambiguous invocation syntax in opaque macro operands
   also refuse it; nested expansion is not interpreted. Any visible
   workspace definition/import binding of a trusted macro, a foreign glob,
   `macro_use`, `no_implicit_prelude`, or macro-generation arguments naming a
   trusted macro used by that test refuses the pin. This scan
   includes other indexed files because textual macro scope reaches child
   modules. Workspace-owned globs remain supported only when that scan finds
   no competing binding. The scan conservatively ignores visibility/cfg
   refinements, so unrelated same-name definitions can cause under-credit.
   Other trusted names unused by the test do not defeat its pin (for example,
   an unrelated `std::fs::write` import does not defeat a plain assertion). It is not
   macro expansion or full name resolution.
5. The existing owner-binding defeats still apply: a foreign same-name
   import, a same-named function in the test's own package when the owner
   lives in another package, and the exact variant when the changed
   expression constructs an error variant.

## Required Evidence

- The bytes 7930d93 replay moves both `return_value` findings
  (`try_get_int`, `try_get_int_le`) from `weakly_exposed` to `exposed`, and
  the commit's `Err(TryGetError { .. })` assertion is not the confirming
  oracle.
- A fixture pins the bytes shape (`fixtures/owner_return_pin_trait_method`).
- A fixture pins each trap as non-exposed
  (`fixtures/owner_return_pin_identity_traps`): the associated-versus-free
  bare call, the overridden trait default, the early-exit input, and the
  test-local binding of the owner's name.
- Unit tests pin every gate with a positive and a discriminating negative.
- Twenty matched fixtures keep effective and ineffective tests separate:
  - `owner_return_pin_direct`, `_called_closure`, `_token_direct`, and
    `_token_called_closure` retain `exposed`; correct libraries pass one test,
    and deliberately wrong `input * 2` libraries fail with actual 8 versus
    expected 12.
  - `_uncalled_closure`, `_shadowed_assertion`, `_expired_closure_binding`,
    `_deferred_closure_binding`, `_macro_operand_return`, `_macro_return`,
    `_if_false`, `_unpolled_async`, and `_token_overlap` pass even the wrong
    library. They retain related-test provenance without an oracle and read
    `reachable_unrevealed`, Observe `no`, Discriminate `no`, strength `none`.
  - `_unknown_singleton`, `_nested_test`, `_cfg_false_module`, `_cfg_false_file`,
    `_cfg_attr_module`, and `_out_of_line_cfg` pin review findings from #5020:
    admission cannot manufacture a singleton match or use an uncollected test.
    Each keeps one collected runtime test and passes either library, while
    the static finding is No/No/None `reachable_unrevealed`.
  - `_no_assertion` is the removal control and keeps that same class/stage
    outcome without the context-refusal diagnostic.
- `cargo test -p ripr --test owner_pin_execution` compiles both library
  variants and each actual fixture test before comparing runtime outcomes.
  All twenty cells are in the RIPR-SPEC-0108 honesty corpus. Additional
  public-API mixed-oracle tests reject confirmation/strength borrowed from
  refused assertions and preserve valid direct evidence in either order.
- The initial narrow repair left refused assertions `weakly_exposed` with
  Observe `yes`, confidence 0.92 and advice to replace broad assertions. The
  `_token_overlap` control additionally remained falsely `exposed` through
  generic token matching. This is why admission precedes all oracle credit.
  The corrected negative cells use the unchanged confidence algorithm:
  three `yes` stages plus two `no` stages and the existing unrevealed-class
  adjustment produce 0.79: `3 * 0.20 + 2 * 0.02 + 0.15`. Medium/High stage
  confidence leaves the RIPR-SPEC-0109 ceiling at 1.0. This is an advisory
  confidence signal for the static result, not a
  probability, protection percentage or runtime-adequacy claim. Reach,
  infection and propagation keep their existing static authorities.
- Human/JSON/agent findings disclose `rust_assertion_context_unestablished`
  and ask to establish execution and macro binding, rather than prescribe
  another equality assertion. No runtime outcome is imported into the
  production classifier. Agent context has no top-level scalar confidence
  field; its gap witness carries `confidence.value: 0.79` with
  `confidence.basis: static_only`, alongside the same No/No stages,
  zero-oracle relation and guidance. Exposed controls have no gap witness.

### Matched before/after observations

The initial fourteen fixture inputs were replayed on retained analyzer `e729ca15`,
the rejected narrow owner-pin-only candidate, and the shared-admission candidate.
Every run produced one complete `return_value` finding. Each exact test also ran
against the correct and deliberately wrong library; every correct-library test
passed. Runtime outcomes remain independent of the static verdict.

| Cases | Count | Old analyzer | Narrow pin-only candidate | Shared admission | Wrong-library runtime |
|---|---:|---|---|---|---|
| Direct and called closure, with/without token overlap | 4 | exposed, 1.00 | exposed, 1.00 | exposed, 1.00 | one failure per case |
| Deferred/conditional/binding/macro negative controls without overlap | 8 | exposed, 1.00 | weakly_exposed, 0.92 | reachable_unrevealed, 0.79 | one pass per case |
| Deferred closure with named-input token overlap | 1 | exposed, 1.00 | exposed, 1.00 | reachable_unrevealed, 0.79 | one pass |
| No-assertion removal control | 1 | reachable_unrevealed, 0.79 | reachable_unrevealed, 0.79 | reachable_unrevealed, 0.79 | one pass |

Six later review controls extend the matrix to twenty cases (forty compiled
library/test subjects). Against the first published #5020 head, the real
Unknown helper singleton reads `weakly_exposed` at 0.92, while the five
non-collectable-test shapes read `exposed` at 1.00. With the repaired admission,
all six read No/No/None `reachable_unrevealed` at 0.79. The singleton heuristic
uses the original test assertion count: removing credit must never create a
new matching signal for an unrelated survivor. The bot's initial `assert!(ready)`
example is a `RelationalCheck` and already family-matches; the separate-line
`assert_ready(true)` helper is the discriminating Unknown control.

The initial return-only slice changed five older guarded-result fixtures: conditional
bare equality rows lose standalone return-value oracle credit. The dedicated
`guarded_result_match` authority remains intact, including its positive control.
Four fixture class outcomes are unchanged; `guarded_result_match_swallowed`'s
return-value probe became unrevealed while its error-path probe initially stayed
weak. The #5027 extension now refuses the same conditional bare equalities as
standalone ErrorPath evidence, so both selected families are unrevealed.
Separate family-filtered honesty cases pin exactly one return-value finding
and exactly one error-path finding, preserving both distinct class contracts.
The original-cardinality repair also prevents an existing lexical
`let result = expect_response(..)` mock-expectation row from gaining a new
singleton match in `guarded_result_match_fail_closed`; other retained evidence
keeps its stages and class unchanged. This is bounded admission, not a claim
to resolve match-arm execution or every other oracle family's provenance.

### Shared error and predicate equality admission

The private execution/collectability/macro-binding witness also gates bare
`assert_eq!` evidence for `error_path` and `predicate` probes (#5027). This is
independent of the owner-return pin: error operands still use RIPR-SPEC-0106,
and predicate activation/boundary pairing retain their own semantic authorities.
Neither a valid error operand nor a boundary-valued call proves that an assertion
executes. Admission precedes matching, observation and oracle-strength selection
for all three named families; refusal preserves the original assertion count so
an unrelated Unknown helper cannot gain singleton credit.

An independent ordinary equality also remains eligible after a uniquely bound,
local empty macro. The syntax witness recognizes only one unannotated
`macro_rules!` declaration with the sole catch-all `($($name:tt)*) => {}` rule,
declared before the invocation in the same or an enclosing lexical module.
The existing workspace binding authority rejects competing definitions,
imports and opaque binding-producing calls. This is not a property-macro name
allowlist or a macro evaluator: other matchers, nonempty/returning expansions,
qualified, imported, shadowed and ambiguous bindings remain unsupported.
The same parser-backed local resolver also removes these empty invocations
from the shared call-fact view. Both call discovery and retained mixed-line
text exclude their arguments; a discarded 100/100 call cannot lend boundary
activation to a real far equality on the same line. Original function/file
source and AST coordinates remain intact. The far equality retains strong
oracle strength and Observe/Discriminate=yes, with weak infection and
`weakly_exposed`, just as when the empty invocation is removed. The real
boundary positive remains exposed and rejects the wrong implementation.
This does not govern arbitrary non-equality raw-oracle consumers or resolve
general macro expansion. The same closure, CFG, collection and statement-prefix
requirements still govern the independent equality. Calls after an assertion
receive no new prefix exception.

The repair changes extracted call facts, so predecessor facts are not
interchangeable. Persisted file-fact/classified keys retain their existing
build identity: an e290 predecessor uses a distinct producer from the repaired
commit even when generations remain file facts 1.18/full 1.29/compact and
sharded 0.35. Favorable predecessor self-hit, repaired-producer refusal and
current reuse are separate controls; no cross-build cache bypass is inferred.

Fourteen family fixtures pair direct and invoked-closure positives with uncalled,
false-branch, macro-shadowed and no-assertion controls. Each family also has an
unconditional owner call before an uncalled assertion: real reach alone cannot
create an observer. Public-API tests compile the correct and deliberately wrong
version of each fixture and require exactly one executed test per subject.
The four effective tests fail against the wrong implementation; the ten
ineffective/removal tests pass it. These independent runtime outcomes calibrate
the fixtures only and are not imported into production classification.

The eight ineffective assertion cells and two removal cells require
`reachable_unrevealed`, Observe `no`, Discriminate `no` and oracle strength `none`.
Four positive cells retain `exposed` and strong exact-value evidence. Mixed
strong/weak/refused assertions are checked in both orders; a refused exact
assertion cannot lend strength to an admitted weak oracle or turn an Unknown
helper into a singleton. Ten related tests exercise the eight-row JSON projection
cap without crowding out admitted evidence. All cells are independently guarded
by family-selected RIPR-SPEC-0108 corpus assertions.

A separate ninety-subject compiled matrix covers all three admitted families:
a direct assertion before/after a root return, a bound closure invoked before/after
a root return, a closure that returns before its assertion, and unrelated ordinary
or CFG-disabled nested helpers before direct/invoked assertions, and unrelated
`async`/`async move` return contexts. Effective direct
and invoked positives are admitted; prior outer returns and closure escapes are
refused. The statement-prefix query retains
all macro, CFG, collection and closure-escape gates; it does not evaluate arbitrary
branch conditions. The existing Result harness retains its earlier strong equality
and its class/confidence; a possible return still refuses a later equality.
Only returns owned by the collected function enter its statement-prefix boundary.
A return in an unrelated async block belongs to that future, even before it is
polled; it cannot hide a later executed outer assertion. Assertions inside an
unpolled future remain refused, and an actual outer return still defeats a later
assertion. This is return-scope information, not an async-harness execution claim.
An ordinary or CFG-disabled nested helper's return cannot escape the outer test;
direct and invoked-closure positives remain admitted with either helper present.
This does not admit an assertion inside an uncalled nested helper, and the earlier
closure-return refusal remains unchanged.
The ReturnValue helper controls use the existing token-direct assertion shape;
the separate owner-return pin still conservatively refuses a test body containing
multiple function declarations. Execution admission does not broaden that binding
proof or turn it into an exact owner-return pin.

The full scan also exposes a deliberate usefulness tradeoff: the existing real
`#[tokio::test]` fixture catches an inverted predicate at runtime, but the bounded
query cannot establish its macro binding and async polling path. It retains owner
and test discovery and `propagation_unknown`, while oracle strength becomes None
and advisory confidence moves 0.66 to 0.43. Its 100/50 input still cannot distinguish
`>` from `>=`; that activation limitation is separate. #5040 owns producer-backed
async-harness execution provenance and the real positive/removal controls. No
Tokio-name exception or claim that the actual test is ineffective is made here.

The same existing human/JSON/context projections explain a refused invocation
and keep confidence advisory and `static_only`. This change does not assert
that all oracle families, assertion macros or arbitrary Rust control flow have
execution provenance.

### Admission consumers and cross-test pairing

Predicate boundary pairing consumes the exact same admission callback as reveal.
A false-branch, uncalled-closure or unpolled-future boundary equality cannot regain
credit after reveal refuses it by borrowing a strong far-input oracle. Matched
same-test and separate-test layouts retain that far oracle's strong evidence and
Observe=yes, while Discriminate stays weak with `same_test_pairing_missing`.
Direct and invoked boundary positives remain exposed. Twelve paired layouts
compile correct/wrong implementations and execute exactly one or two tests per
program, rather than treating a zero-subject or compile failure as a control.
The summary describes an absent admitted boundary-call discriminator without
falsely claiming that every failure involves different tests.

The covered route is Rust diff classification. Its consumer inventory is:

| Consumer | Authority and boundary |
| --- | --- |
| `OwnerPinSyntax` / `OwnerReturnPin` | One parser-backed admission decision; owner-return binding remains a separate narrower proof. |
| `classify/reveal.rs` | Filters before kind, strength, token matching, observation and owner-pin credit. Original assertion cardinality remains separate and unchanged. |
| `classify/boundary_pairing.rs` | Uses the same callback before strength or boundary-subject pairing; raw refused assertions cannot restore exposure. |
| `classifier/evidence.rs` / `classify/decision.rs` | Compose admitted stages; the tuple-specific witness is MatchArm-only and outside these covered families. |
| `classifier/finding.rs` | Strong sink guidance starts from the already-filtered `evidence.related_tests`, not raw assertions. |
| `classify/activation.rs` / `classify/related_tests.rs` | Raw source value/missing-fact and relationship scans remain their own static authorities. They are not oracle admission or runtime execution proof and cannot bypass the final admitted pairing requirement. |
| `test_grip_evidence.rs` | Repository grip has a separate raw-oracle consumer. This diff-path repair does not claim repository-grip execution parity; #4793 owns the shared-witness migration. |

The `observed_values` field retains lexical and bounded statically derived
source facts independently of oracle admission, including enum tokens in
refused assertions. Its historical name and structured fields remain stable;
it does not establish execution or observation. Human and JSON evidence paths
share neutral `source ... value` wording, including source-value cap disclosure.
Direct/invoked error oracles retain strong observation, while uncalled, disabled
or shadowed assertions retain source facts without contradicting Observe=no.
This presentation repair changes no class, stage, score or admission decision.

The two executed bypasses, family dispatch before reveal and raw boundary pairing
after reveal, motivate an admitted-oracle iteration contract under #4793. That
follow-up must carry original cardinality separately and preserve source facts
for diagnosis; it must not introduce another execution checker or globally erase
assertions. This repair shares the existing callback without that larger migration.

## Non-Goals

- Shared admission covers Rust `return_value`, `error_path` and `predicate`
  evidence from bare `assert_eq!` invocations. Qualified assertion macros, other
  oracle kinds/families and general control-flow or macro resolution retain
  their existing authorities; this is not a general execution-proof system.

- Name resolution or type inference. The receiver typing reads a binding's
  syntax only; a receiver returned by an arbitrary function call stays
  unestablished.
- Methods generated by derive or attribute macros, and `macro_rules!`
  bodies that build the name from fragments (`paste!`), are invisible; they
  are documented residuals.
- A trait from outside the workspace that is also in scope and also names
  the method is not detected.
- Qualified calls (`Type::name(..)`, `Trait::name(&mut recv, ..)`) do not
  pin yet.
- A helper function the changed tail calls may itself ignore an argument
  on some inputs; the tail gate reads the tail's own syntax only.
- Opaque macro expansion can synthesize a binding from fragments without a
  visible `assert_eq` token. Such generated identities are not resolved by
  this bounded source query; no compiler-equivalent macro identity is claimed.
- No runtime claim: `exposed` stays a static reading. The matched compiled
  controls are independent evidence for the fixture contract only.

## Acceptance Examples

- Given a trait default `try_get_int` whose changed tail is
  `Ok(sign_extend(self.try_get_uint(nbytes)?, nbytes))`, an
  `impl Buf for &[u8]`, and a test importing `Buf` that binds
  `let mut a = &[0xff, 0xff, 0xff][..];` and asserts
  `assert_eq!(a.try_get_int(3), Ok(-1))`, when `ripr check` runs, then the
  finding reads `exposed`.
- Given the same owner and a test asserting only
  `assert_eq!(short.try_get_int(4), Err(TryGetError { .. }))`, the finding
  stays below `exposed`.
- Given `impl Codec { fn decode(input: u32) -> u32 }` and a free
  `fn decode`, a test asserting `assert_eq!(decode(8), 9)` does not confirm
  a change to `Codec::decode`.
- Given a trait default `next_word` and `impl Reader for Fixed` overriding
  it, a test asserting `assert_eq!(reader.next_word(), 7)` on a `Fixed`
  receiver does not confirm a change to the default.

## Test Mapping

- Unit (`crates/ripr/src/analysis/classify/owner_pin/tests.rs`): the bytes
  shape; trait scope; competing definitions (override, inherent twin,
  `macro_rules!` body, forward, free twin); receiver typing; slice-method
  names; inherent receivers; bare calls (associated twin, free twin, local
  binding, `for`/closure/parameter/macro bindings, `use .. as` renames);
  the return-path gate, including conditionally evaluated tails and spaced
  macros; plain `assert_eq!` against an owner-free value, `#[should_panic]`
  and assertions outside the test body; by-value prelude method names;
  constructor signatures; macro-bound, aliased and parameter receivers;
  lexical fallback; the item-container fact.
- Unit execution and macro context controls: `owner_pin_requires_an_executed_assertion_context`,
  `owner_pin_requires_unambiguous_standard_assert_eq`, `owner_pin_refuses_ambiguous_oracle_coordinates`,
  `owner_pin_macro_ambiguity_in_other_files_and_run_memo`,
  `owner_pin_closure_call_must_share_the_bindings_live_scope`, and
  `shared_return_admission_uses_the_outer_invocation_identity`, and
  `owner_pin_requires_test_item_ancestry_and_enabled_cfg` in the same test module.
- CFG authority (`analysis/facts/cfg_predicates/tests.rs`):
  `test_build_availability_preserves_unknown_and_boolean_identity` and
  `test_build_availability_refuses_raw_attribute_heads` distinguish
  enabled, disabled, unknown and malformed inputs without changing role classification.
- Integration (`crates/ripr/tests/owner_pin_execution.rs`):
  `owner_pin_matched_static_and_runtime_controls` (twenty fixtures, two library variants),
  `owner_pin_token_overlap_cannot_bypass_oracle_admission`,
  `owner_pin_shared_admission_keeps_credit_on_one_admitted_oracle` (six mixed cases), and
  `owner_pin_refused_rows_do_not_crowd_out_admitted_oracles` (eight related tests),
  and `owner_pin_review_admission_controls` (six public-API review regressions).
  `local_empty_macro_preserves_independent_equality_execution` compares static
  admission with compiled correct/wrong subjects for local empty, returning,
  imported, ambiguous, shadowed and disabled declarations. The property
  quarantine integration retains its named/direct/helper mixed positives.
  The execution fixtures and their JSON/human outputs are mapped in `.ripr/traceability.toml`.
- Fixtures: `fixtures/owner_return_pin_trait_method`,
  `fixtures/owner_return_pin_identity_traps`; re-blessed
  `fixtures/infect_value_returned`, `fixtures/infect_wildcard_discard`,
  `fixtures/tail_comparison_boundary`, whose tests pin a free owner's
  unconditional tail.

## Implementation Mapping

- `crates/ripr/src/analysis/syntax/ra.rs`: `function_item_fact` fills
  `FunctionFact.item`.
- `crates/ripr/src/analysis/facts/model.rs`: `FunctionContainer`,
  `FunctionItemFact`.
- `crates/ripr/src/analysis/classify/owner_pin.rs`: `OwnerReturnPin`
  (`establish` for the owner-side gates, `admits` for the test-side gates;
  `OwnerPinSyntax::admits_equality_assertion` applies shared context independently
  of whether an owner-return pin can be established).
- `crates/ripr/src/analysis/classify/reveal.rs`: the pin joins the
  confirmation signals behind the family, oracle-kind and owner-binding
  defeats. `analyze_related_assertions` applies shared context before matching
  or credit and preserves refused assertions only as zero-oracle relations; `file_imports_own_item`; `::`-rooted `use` paths.
- `crates/ripr/src/analysis/classifier/evidence.rs`: establishes the pin
  once per probe and supplies the shared context-admission callback.
- `crates/ripr/src/analysis/classifier/finding.rs` and `classify/decision.rs`:
  project the existing stage verdict into execution/binding guidance and
  missing evidence, without changing classification or confidence arithmetic.
- `crates/ripr/src/analysis/syntax/owner_pin.rs`: private bounded assertion
  execution-context query. `OwnerPinSyntax` in the classifier memoizes the
  workspace macro scan and per-file context across probes in one immutable
  index; no output or serialized `OracleFact` field changes.
- `crates/ripr/src/analysis/facts/cfg_predicates.rs`: bounded test-build
  availability reuses the canonical lexer; the existing test-only role query
  retains its separate contract. Out-of-line resolution remains owned by
  existing `FileFacts::role_provenance`, not by the admission consumer.
- `crates/ripr/src/analysis/seam_cache.rs`: classified `1.25`, sharded `0.31`,
  compact `0.31` invalidate stale false credit. File-fact `1.15` from #4748 is preserved;
  the query reads existing indexed source, so no file-fact migration is needed.
  The new classified generation also rejects favorable results from published
  candidate 740098f5, whose post-reveal pairing could re-use refused assertions.
  The statement-prefix refinement changes no serialized fact shape.

## Metrics

- `owner_return_pin_false_exposed` — zero: no fixture or replay finding
  gains `exposed` through the pin unless its confirming assertion is a
  complete call that names the owner on the changed return path.
