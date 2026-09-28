from hypothesis import given, strategies as st
from src.pricing import apply_discount


@given(st.integers(min_value=0), st.integers(min_value=0))
def test_apply_discount_property(amount, threshold):
    assert apply_discount(amount, threshold) <= amount
