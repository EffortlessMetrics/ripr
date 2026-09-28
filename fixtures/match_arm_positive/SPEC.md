# Fixture: match_arm_positive

Spec: RIPR-SPEC-0164

## Given

A literal match helper (`label`) whose whole body is one `match` over
its `kind` parameter with string-literal arms and a `_` wildcard. The
only caller (`classify`) binds its return into `final_label` through
the #3296 operand jump and compares it at an equality boundary. The
diff changes the comparison constant (`"alpha"` -> `"beta"`). The
tests call only `classify` with exact literals, one per non-wildcard
arm; neither test names the helper.

## When

```bash
cargo xtask fixtures match_arm_positive
```

## Then

The local-with-call-initializer operand jumps to the helper authority,
the match evaluator resolves `label` per test row over the bound
inputs (`"word"` -> `"alpha"`, `"text"` -> `"beta"`), and the boundary
equality is observed exactly: the finding is `exposed`, with the
helper hop (`label = "beta" via helper return ... (1 hop)`) carried in
the observed-value provenance. Each supported arm connects to an exact
assertion (the #3215 literal-match acceptance row).

## Must Not

- Resolve a guard, an alternative pattern, a bare binding, a computed
  arm value, an escaped literal, or a char scrutinee (see
  `match_arm_controls`).
- Credit the boundary without an exact evaluation of the operand on a
  related-test row.
