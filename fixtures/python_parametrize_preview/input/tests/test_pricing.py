import pytest


@pytest.mark.parametrize("amount", [10, 20])
def test_discount_amount(amount):
    assert apply_discount(amount, 10) >= 0
