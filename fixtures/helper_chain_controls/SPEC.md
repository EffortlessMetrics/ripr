# Fixture: helper_chain_controls

Spec: RIPR-SPEC-0159

## Given

Three fail-closed controls, each with its changed line under the diff:
the computed-argument control (`classify` calls its helper with
`input.trim()`), and two same-name `is_word_start` helpers (root and
`other::`) so the callee name is non-unique and path-qualified call
sites are refused — neither helper chain may relate or transfer. The
tests call only the entry functions with exact values.

## When

```bash
cargo xtask fixtures helper_chain_controls
```

## Then

No helper-owned probe relates through a chain: both changed helpers
stay `no_static_path` with no related tests, keeping the pre-transfer
output, and no transferred input row or call-operand evaluation
appears for these shapes.

## Must Not

- Transfer through a computed argument or a non-unique callee name.
- Promote any control past its fail-closed class on call reach alone.
