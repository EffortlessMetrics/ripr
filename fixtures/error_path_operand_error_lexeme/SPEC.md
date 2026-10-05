# Fixture: error_path_operand_error_lexeme

Spec: RIPR-SPEC-0108

## Given

A reader now propagates an I/O error through `?` instead of returning zero.
The only test reads a successful byte slice and checks its successful result
and unchanged length together with a test-local `error_count` of `0`:

```rust
let error_count = 0;
assert_eq!((rdr.len(), error_count), (10, 0));
```

## When

```bash
cargo xtask fixtures error_path_operand_error_lexeme
```

## Then

Exactly one `error_path` finding remains `weakly_exposed`, with
`observation_unverified`. The independent honesty corpus selects that family;
the separate base-deleted `static_unknown` finding is not this fixture's claim.

## Must Not

- Promote the ErrorPath because a test-local identifier contains the lexeme
  `error` in operand position.
- Drop the finding or remove its reachable test to make the result look clean.

The message-twin control is `error_path_diagnostic_error` (`"read error"` in
the diagnostic). Genuine typed error and guarded Result observers remain
covered by their existing positive fixtures.
Refs #5255 (residual of #4748).
