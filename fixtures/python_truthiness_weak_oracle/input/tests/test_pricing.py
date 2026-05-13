from src.pricing import apply_discount


def test_apply_discount_truthy():
    assert apply_discount(100)
