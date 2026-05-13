from src.pricing import apply_discount


def test_apply_discount_exact_value():
    assert apply_discount(100, 50) == 90
