# Fixture: python_adversarial_local_assignment_operator_input (false-exposed guard — local-assignment operator change must not credit an input operand)

Spec: RIPR-SPEC-0028

## Given

An adversarial **local-assignment operator** change with an empty token delta. The
owner `compute` flips `+` to `-` in a plain local assignment; the only strong
related oracle observes the **unchanged input parameter** `base`, never the changed
result:

```python
# changed owner: compute  (delta: + -> -, no identifier/literal token delta)
def compute(base, bonus):
    total = base - bonus
    return total
```

```python
# the only related test — the strong oracle reads the UNCHANGED input `base`
from src.calc import compute

def test_base_unchanged():
    base = 10
    compute(base, 3)
    assert base == 10        # observes an unchanged INPUT operand, not the result
```

A plain local assignment (`total = base - bonus`) is not recognized by
`classify_probe_shape`, so it previously fell to that function's **default**
`(Predicate, Control)` arm. The empty-delta operand fallback (#1278) is keyed on a
control-flow change, so a default-`Control` assignment wrongly kept the fallback and
credited `changed_sink_token` on the unchanged operand `base`. The test never
observes `total`, so it cannot notice if the `+`/`-` change were wrong.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr does **not** classify the change `exposed`. The empty-token-delta operand
fallback is gated on a **precise control-flow line check** (`if`/`while`/`for`/
`match`/ternary/`raise`/`except`), not the default-polluted delta kind. A plain
local or augmented assignment is not a control-flow line, so the fallback is
withheld and the unchanged input operand is not credited. A discriminating test that
observes the owner's call result (`assert compute(10, 3) == 7`) still classifies
`exposed` via the `direct` path (a companion unit test pins this).

## Must Not

- Credit `exposed` from a `changed_sink_token` match on an unchanged input operand
  of a local/augmented assignment operator change.
- Regress `python_cross_file_construct_call` (#1228), `python_field_assignment_shape`,
  or `python_adversarial_operator_delta_input_operand` (#1278).
- Run any Python runtime; static preview evidence only.
