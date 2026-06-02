from app.pricing import calculate_discount


def test_calculate_discount_above_threshold():
    assert calculate_discount(100, 50)
