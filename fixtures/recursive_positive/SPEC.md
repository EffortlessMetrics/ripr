# Fixture: recursive_positive

Spec: RIPR-SPEC-0165

## Given

A self-recursive match helper (`label_of`) whose `"word"` arm's value
is a nested direct call to itself with a different literal argument
(`label_of("text")`), the `"text"` arm returning `"beta"`, and the
wildcard returning `"alpha"`. The only caller (`classify`) binds the
return into `final_label` through the #3296 operand jump and compares
it at an equality boundary. The diff changes the comparison constant
(`"alpha"` -> `"beta"`). The tests call only `classify` with exact
literals coherent with the pre-diff behavior.

## When

```bash
cargo xtask fixtures recursive_positive
```

## Then

The operand jumps to the helper authority; the evaluator enters
`label_of("word")` (state 1), the nested `label_of("text")` (state 2,
distinct inputs — not a cycle), and resolves `"beta"` within the hop
bound. The boundary equality is observed exactly on the word row: the
finding is `exposed`, with the helper hop (`label_of = "beta" via
helper return ... (1 hop)`) in the observed-value provenance.

## Must Not

- Resolve a repeated `(helper, bound inputs)` state or a chain beyond
  the hop bound (see `recursive_controls`).
- Credit the boundary without an exact evaluation of the operand on a
  related-test row.
