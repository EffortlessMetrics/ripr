# Fixture: python_adversarial_buffer_token (adversarial false-exposed guard)

Spec: RIPR-SPEC-0028

## Given

An adversarial **token-coincidence** over-credit trap — the confirmed
false-exposed vector from #1224, pinned end-to-end. A module function `pack`
changes a boundary inside its length guard (`<` → `<=`), and the only related
test reaches `pack(...)` under a *strong* exact-value oracle whose variable name
**embeds a changed-sink token as a substring**:

```python
# changed sink tokens include `buffer`
if len(buffer) <= limit:

# oracle observes `buffered_output`, which merely CONTAINS `buffer`
buffered_output = pack([1, 2], 5)
assert buffered_output == [1, 2]
```

The test genuinely reaches the owner (a real `SyntacticCall` relation) and carries
a genuinely *strong* `ExactValue` oracle — so the only thing standing between
ripr and a false `exposed` is sink-token alignment using **identifier
boundaries**, not substring containment.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr classifies the change `weakly_exposed` with `oracle_alignment: orthogonal`:
the strong oracle reaches the owner but observes `buffered_output`, not the
changed sink. `buffer` matches `buffered_output` only as a substring (`buffer`
followed by `e`, an identifier char), so `oracle_text_observes_token` correctly
declines the match. This is the contract-honoring conservative direction
(under-credit, never over-credit).

**This fixture must NEVER read `exposed`.** A regression that reverts
`changed_sink_token` alignment to substring containment (`text.contains`) would
flip this to `exposed` — the exact #1224 false-exposed. Pinning `weakly_exposed`
here makes that regression fail CI end-to-end, not only at the
`oracle_text_observes_token` unit level.

## Must Not

- Credit `exposed` on the `buffer` ⊂ `buffered_output` substring coincidence.
- Run any Python runtime; static preview evidence only.
