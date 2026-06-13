# Static Exposure Model

`ripr` is **static mutation-exposure analysis**. It catches the same class
of signal mutation testing catches — weak test/oracle exposure on changed
behavior — but earlier and cheaper, by reading the diff at draft time
instead of running mutants. It does not find or run actual mutants;
mutation testing remains the slower runtime backstop for what static
analysis cannot predict.

It creates mutation-shaped probes from changed code and asks whether existing
tests appear to provide the RIPR chain needed to expose the changed behavior:

```text
Reach -> Infect -> Propagate -> Observe -> Discriminate
```

## Probe

A probe is an unexecuted, mutation-shaped hypothesis attached to changed code.

Examples:

| Change shape | Probe family | Expected discriminator |
| --- | --- | --- |
| `>` changed to `>=` | `predicate` | boundary test at equality |
| error variant changed | `error_path` | exact error variant assertion |
| returned field changed | `field_construction` or `return_value` | field, whole-object, or snapshot assertion |
| side effect added | `side_effect` | mock, event, state, persistence, or metric oracle |
| match arm changed | `match_arm` | input selecting arm plus exact assertion |

The current MVP probe families are:

- `predicate`
- `return_value`
- `error_path`
- `call_deletion`
- `field_construction`
- `side_effect`
- `match_arm`
- `static_unknown`

## RIPR Stages

`Reach` asks whether a related test appears to reach the changed owner.

`Infect` asks whether test inputs appear capable of activating the changed
behavior, such as a boundary value for a predicate.

`Propagate` asks whether the changed state appears able to flow to an observable
value, error, field, side effect, event, state change, or persistence boundary.

`Observe` asks whether a related test has an oracle near the propagated effect.

`Discriminate` asks whether that oracle is strong enough to distinguish intended
behavior from a plausible wrong behavior.

## Stage States

Stage states are intentionally conservative:

- `yes`
- `weak`
- `no`
- `unknown`
- `opaque`
- `not_applicable`

`unknown` and `opaque` are not failures of the tool. They are honest signals that
static analysis should stop or escalate.

## Exposure Classes

| Class | Meaning |
| --- | --- |
| `exposed` | Static evidence suggests a complete RIPR path to a strong oracle that observes the changed sink (see Discrimination vs Coverage). |
| `weakly_exposed` | A path exists, but infection or discrimination appears weak. |
| `reachable_unrevealed` | Related tests appear reachable, but no meaningful oracle was found. |
| `no_static_path` | No static test path was found for the changed owner. |
| `infection_unknown` | Reachability exists, but input or fixture evidence is opaque. |
| `propagation_unknown` | The changed behavior crosses an opaque propagation boundary. |
| `static_unknown` | Syntax-first analysis cannot make a credible judgment. |

## Public Badge Projection

Public `ripr` badges are not raw exposure-class totals, seam-native inventory,
coverage, mutation adequacy, all behavior seams, or all untested code. They
project unresolved actionable static repair gaps: canonical gaps with a repair
route, verification path, receipt path, and public projection eligibility.
`ripr+` adds only actionable test-efficiency repairs lifted into the same
repair / verify / receipt model. Detailed seam-native inventory remains an
internal pressure report for evidence quality and static limitations.

## Analysis Modes

Modes define how much static evidence `ripr` is allowed to gather before it
classifies probes. They change scope and cost; they do not change the meaning of
the exposure classes.

| Mode | Scope in the current alpha | Intended use |
| --- | --- | --- |
| `instant` | Changed Rust files only. | Editor-safe, cheapest feedback. |
| `draft` | Rust files in packages touched by the diff. | Default local scan. |
| `fast` | Same package-local scope as `draft` for now. | Draft PR scan; future bounded graph work lands here. |
| `deep` | All Rust files in the workspace. | Manual or CI scan when wider static evidence is acceptable. |
| `ready` | All Rust files in the workspace. | Static preflight before real mutation confirmation. |

`ready` mode still does not run mutants or report mutation outcomes. It remains
static exposure analysis unless a future calibration or mutation adapter is
explicitly invoked.

## Oracle Strength

Strong oracle examples:

- `assert_eq!`
- `assert_ne!`
- `assert_matches!`
- exact enum or error variant assertion
- whole-object equality
- configured snapshot or mock oracle

Weak or smoke oracle examples:

- `assert!(result.is_ok())`
- `assert!(result.is_err())`
- `unwrap()`
- `expect()`
- `assert!(x > 0)`
- `assert!(!items.is_empty())`

The MVP favors high-signal distinctions over completeness. A weak oracle is not
bad by itself; it is weak when the changed behavior needs a stronger
discriminator.

## Discrimination vs Coverage

`ripr` is not a coverage tool, and the difference is the entire point. Coverage
asks whether a line *executed*. `ripr` asks whether a test would *notice* the
changed behavior being wrong — whether some oracle actually observes the value
the change moves.

The two come apart constantly. A test can execute every branch of a function and
assert almost nothing (`assert result is not None`, a smoke check, a mock that
never inspects the value). Every line is covered; nothing is discriminated. So
`ripr 0` (no exposure gaps) is already a higher bar than 100% coverage, and
`ripr+ 0` (no exposure gaps *and* no weak oracles — every changed behavior under
a strong, aligned discriminator) is higher still. Most repositories at 100%
coverage sit well below `ripr+ 0`.

The bars form a ladder, not a synonym set:

```text
100% coverage     every changed line executed under test
   <  ripr 0      every changed behavior reached by *some* discriminator
   <  ripr+ 0     every changed behavior under a *strong, aligned* discriminator
                  (no exposure gaps and no weak oracles)
```

A repository can stand on the bottom rung and fail the top two: full coverage,
yet changed behavior that no test would notice breaking. That gap between
"executed" and "discriminated" is the whole surface `ripr` reports on.

### The alignment invariant

The standing danger for a static analyzer is to drift back into coverage by
crediting proximity as discrimination: "a strong oracle reaches the owner,
therefore the behavior is checked." That is false. A strong oracle that observes
a *different* value than the changed sink does not discriminate the change — a
test asserting a wrapped function's return value does not pin a boundary deep
inside the wrapper, even though the assertion is strong and exact.

So `exposed` requires **sink alignment**: the strong oracle must observe the
changed behavior's output — its assertion references the changed owner (by name
or import alias) or the changed sink (the attribute, field, or value the change
touches). A strong-but-orthogonal oracle downgrades to `weakly_exposed` with a
typed reason and routes a repair, rather than being silently called covered.

### Two error rates

Trust in `ripr` rests on two error rates, not one:

- **false-actionable** — it routed a repair for a behavior that is actually
  discriminated. Visible: inspect the emitted finding.
- **false-`exposed` / over-credit** — it called a behavior covered when no oracle
  discriminates it. *Silent*: `ripr` emits nothing, so it cannot be found by
  inspecting output, only against ground truth on the cases where `ripr` stayed
  quiet.

The second is the more dangerous: it is the failure that makes a discriminator
indistinguishable from coverage, and the one a cheap robustness sweep
structurally cannot see.

## Finding Shape

A useful finding should include:

- changed behavior
- probe family
- RIPR stage evidence
- related tests
- observed oracle strength
- missing discriminator
- recommended next step

The recommended next step should be specific enough for a human or coding agent
to write the targeted test.

## Escalation

Escalate to real mutation testing when:

- propagation stops at an opaque fixture or macro
- dynamic dispatch hides the call path
- async causality is unclear
- external state is involved
- static evidence and human intuition disagree
- the finding would block a release decision

## The discriminator test, turned inward

`ripr` exists to catch one failure shape: a test that *reaches* the changed
behavior but does not *discriminate* it — a green signal that says nothing about
whether the behavior is actually checked. Building `ripr` against real external
code surfaced that same shape at every other layer of the system, which is the
sharpest thing the project has learned about itself:

- A CI gate that *runs* but is not *required* reaches the regression without
  blocking the merge — a green-looking gate that discriminates nothing.
- A consumer that reads the wrong output key (a parser keyed on `class` when the
  artifact emits `classification`) stays green against its own fixture while
  blind to the live output.
- A sweep that analyzed zero repositories reports a vacuous pass — a success
  state with no denominator.
- A validated implementation plan can *assume* a strong oracle that running the
  binary shows does not exist — a plan that reaches the right files and
  discriminates nothing.
- And `ripr` itself under-credits a discriminating test when the oracle reaches
  the owner only through an indirect call (a local binding, an inline
  construct-call, framework dispatch) that syntax-first analysis cannot trace.

These are one failure: **a pass without both a denominator and a discriminator.**
The discipline that counters it is identical everywhere — verify the *artifact*,
not the proxy for it. Run the binary; do not trust the code-reading. Read the
required check's result; do not trust the merge button. Diff the golden; do not
trust "it passed." The report, the plan, the gate, and the green check are all
proxies; the artifact is ground truth.

### Two error rates, measured on real code

The Tier A sweep and Tier B judging across eight real external Python
repositories put numbers on the two error directions from
[Two error rates](#two-error-rates):

- **false-`exposed`** (silent over-credit — calling a behavior covered when no
  oracle discriminates it): **zero** across the corpus. `ripr` does not hand out
  false confidence; the conservative `exposed` rule held on code it did not
  author.
- **false-actionable** (over-suggestion — routing a repair for a behavior that a
  test already discriminates): common, and traced to one cause — `ripr` cannot
  follow a discriminating oracle back to its owner through an indirect call. The
  discriminating test exists; the syntax-first analysis cannot see it.

The *shape* of the error is load-bearing: `ripr` errs toward the visible,
conservative side (over-suggest) and away from the silent, dangerous side
(over-credit). It is **safe** — and currently **imprecise** on idiomatic code, a
precision ceiling set by indirect-call blindness in relation and oracle
extraction, not by the exposure model. Safety is the harder half and it is in
hand; precision is the tractable, bounded next step.
