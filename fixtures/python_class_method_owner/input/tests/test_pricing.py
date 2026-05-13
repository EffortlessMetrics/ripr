def test_from_amount_accepts_positive():
    assert Pricing.from_amount(10) is not None
