# Fixture: error_path_diagnostic_neutral

Spec: RIPR-SPEC-0108

## Given

A reader now propagates an I/O error through `?` instead of returning zero.
The only test reads a successful byte slice, checks its successful result and
its unchanged length, with the `neutral` diagnostic variant:

```rust
assert_eq!(rdr.len(), 10, "read mismatch");
```

## When

```bash
cargo xtask fixtures error_path_diagnostic_neutral
```

## Then

Exactly one `error_path` finding remains `weakly_exposed`, with
`observation_unverified`. The independent honesty corpus selects that family;
the separate base-deleted `static_unknown` finding is not this fixture's claim.

## Must Not

- Promote the ErrorPath because of diagnostic words, raw or escaped strings,
  formatting expressions, or an error kind inferred only from those arguments.
- Drop the finding or remove its reachable test to make the result look clean.

The absent and neutral variants are differential controls. Genuine typed error
and guarded Result observers remain covered by their existing positive fixtures.
Refs #4748 and the diagnostic-operand residual after #4771.
