from src.discount import apply_discount


def test_apply_discount_boundary():
    assert apply_discount(100, 100) == 90
