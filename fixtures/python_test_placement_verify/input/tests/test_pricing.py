from src.pricing import calculate_discount


def test_calculate_discount_smoke():
    result = calculate_discount(150, 100)
    assert result
