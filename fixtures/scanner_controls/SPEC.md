# Fixture: scanner_controls

Spec: RIPR-SPEC-0163

## Given

Four fail-closed scanner controls, each with its changed comparison
line under the diff: the step-bound control (`classify_bound` calls
the plain scanner with 36/37-symbol inputs, beyond the 32-step
bound), the computed-argument control (`classify_trim` calls its
scanner with `input.trim()`), the computed next-state control
(`scan_computed` derives one arm's next state from `next_for(input)`),
and the bare-identifier control (`scan_bare` uses the parameter
`input` as an arm's next state — the token-coincidence guard). The
tests call only the entry functions with exact values.

## When

```bash
cargo xtask fixtures scanner_controls
```

## Then

Every control stays `weakly_exposed`: the operand evaluation stops at
its named edge (bound exceeded, computed argument, non-literal arm,
bare identifier) and no scanner hop or evaluated state value appears
in the observed values — the missing-discriminator reason keeps
`unknown` operand values rather than invented states.

## Must Not

- Promote any control past its fail-closed class on call reach alone.
- Parse a bare identifier arm state as a literal state.
- Truncate a beyond-bound scan and present the partial state as exact.
