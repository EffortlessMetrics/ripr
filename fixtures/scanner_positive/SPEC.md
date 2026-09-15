# Fixture: scanner_positive

Spec: RIPR-SPEC-0163

## Given

A string-state scanner helper (`scan_state`) whose only caller
(`classify`) binds its return into `final_state` and compares it at an
equality boundary. The diff changes the comparison constant
(`"word"` -> `"text"`). The tests call only `classify` with exact
literals; neither test names the scanner.

## When

```bash
cargo xtask fixtures scanner_positive
```

## Then

The local-with-call-initializer operand jumps to the helper authority,
the scanner evaluator resolves `scan_state` per test row over the
bound inputs (`"ab"` -> `"word"`, `"ab "` -> `"text"`), and the
boundary equality is observed on the trailing-space row: the finding
is `exposed`, with the scanner hop (`scan_state = "text" via helper
return ... (1 hop)`) carried in the observed-value provenance.

## Must Not

- Resolve the scanner through a computed argument or a non-literal
  transition arm (see `scanner_controls`).
- Credit the boundary without an exact evaluation of the operand on a
  related-test row.
