from src.discount import apply_discount


def test_apply_discount_smoke():
    result = apply_discount(100, 50)
    assert result
