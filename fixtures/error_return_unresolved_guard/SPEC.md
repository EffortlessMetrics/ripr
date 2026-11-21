# Fixture: error_return_unresolved_guard

Spec: RIPR-SPEC-0158

## Given

Production code returns an error through a boolean guard. The error value
is built by a real constructor (`cancelled_error()`), and an exact test
observes the returned error (`expect_err` plus full equality against the
constructor value plus message equality):

```rust
pub fn run_slow(cancelled: bool) -> Result<&'static str, OpError> {
    if cancelled {
        return Err(OpError { code: -32800, message: "operation cancelled".to_string() });
    }
    Ok("completed")
}
```

## When

The `return Err(...)` expression changes from the constructor call to an
inline struct literal with the identical value:

```diff
-        return Err(cancelled_error());
+        return Err(OpError { code: -32800, message: "operation cancelled".to_string() });
```

The runtime value is unchanged; only the producing expression moves behind
the `cancelled` guard parameter.

## Then

ripr must report the changed return with a precise typed limitation naming
the unresolved producer edge (the boolean guard the bounded evaluator
cannot carry into the return), and withhold any impossible actionable
assignment. The classification stays conservative (`infection_unknown` at
most — never a fabricated `exposed`, never a prescription to add an
observer the suite already contains).

## Must Not

- No `exposed` finding for the changed return: the exact observer cannot
  discriminate which producing expression built the identical value.
- No actionable assignment (add an error assertion, cover the return):
  the exact observer already exists.
- No generic changed-syntax limitation: the limitation must name the
  unresolved guard/producer edge.

## Evidence and qualification boundary

Production-subject half of #1579. The historical test-plumbing subject is
recorded `resolved_by_source_role` on the issue and is not this fixture.
Scoping probes on current source show the constructor-message change is
correctly credited `exposed` while this expression change reports bare
`infection_unknown` without naming the edge — the defect this lane repairs.
Expected outputs are recorded only after the production correction; no
observed imprecise output is accepted as a golden.
