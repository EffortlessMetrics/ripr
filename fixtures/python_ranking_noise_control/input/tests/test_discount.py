from src.c_discount import calculate_discount


def test_calculate_discount_smoke():
    result = calculate_discount(100, 50)
    assert result
