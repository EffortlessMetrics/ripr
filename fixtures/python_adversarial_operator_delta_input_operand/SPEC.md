# Fixture: python_adversarial_operator_delta_input_operand (false-exposed guard — empty-delta operator change must not credit an input operand)

Spec: RIPR-SPEC-0028

## Given

An adversarial **operator-only** change with an empty token delta. The owner
`next_value` flips `+` to `-`; the only strong related oracle observes the
**unchanged input parameter** `count`, never the changed return value:

```python
# changed owner: next_value  (delta: + -> -, no identifier/literal token delta)
def next_value(count):
    return count - 1
```

```python
# the only related test — the strong oracle reads the UNCHANGED input `count`
from src.counter import next_value

def test_next():
    count = 5
    result = next_value(count)
    assert count == 5        # strong, but observes an unchanged INPUT operand
    assert result > 0        # weak — passes for both 6 (old) and 4 (new)
```

The operator change produces no identifier/literal token delta, so the #1277
empty-delta fallback would otherwise credit `changed_sink_token` on any operand
on the line — including `count`, an unchanged input. The strong `== 5` oracle
observes `count`, and the only output assertion (`result > 0`) is weak and passes
under both the old and new operator, so the test cannot notice if the change were
wrong.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr does **not** classify the change `exposed`. For an empty-token-delta change
in a **value-producing** family (a `return` / assignment operator edit), the
changed sink is the produced value — observed via the owner call, not via an
unchanged input operand — so the empty-delta operand fallback is withheld. The
fallback is preserved only for **control-flow** changes (predicate / error-path),
where an outcome oracle can discriminate the changed branch (see
`python_cross_file_construct_call`). The companion unit test
`empty_delta_operator_change_stays_exposed_when_oracle_observes_owner_output`
pins the inverse: the same operator change IS `exposed` when a strong oracle calls
the owner and observes the result (`assert next_value(5) == 4`).

## Must Not

- Credit `exposed` from a `changed_sink_token` match on an unchanged input operand
  of an empty-delta value-family change.
- Regress `python_cross_file_construct_call` (#1228) or `python_field_assignment_shape`.
- Run any Python runtime; static preview evidence only.
