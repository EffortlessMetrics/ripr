def test_discount_above_threshold():
    assert apply_discount(100, 50) == 90
