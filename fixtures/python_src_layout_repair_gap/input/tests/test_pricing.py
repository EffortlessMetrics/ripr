from shop_service.pricing import loyalty_discount


def test_loyalty_discount_high_value_smoke():
    result = loyalty_discount(250, 100)
    assert result
