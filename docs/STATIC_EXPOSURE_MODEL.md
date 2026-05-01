# Static Exposure Model

`ripr` uses mutation-testing concepts without running mutants.

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
| `exposed` | Static evidence suggests a complete RIPR path to a strong oracle. |
| `weakly_exposed` | A path exists, but infection or discrimination appears weak. |
| `reachable_unrevealed` | Related tests appear reachable, but no meaningful oracle was found. |
| `no_static_path` | No static test path was found for the changed owner. |
| `infection_unknown` | Reachability exists, but input or fixture evidence is opaque. |
| `propagation_unknown` | The changed behavior crosses an opaque propagation boundary. |
| `static_unknown` | Syntax-first analysis cannot make a credible judgment. |

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
