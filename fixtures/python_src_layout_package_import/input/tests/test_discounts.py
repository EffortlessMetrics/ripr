from pricing.discounts import bulk_discount


def test_bulk_discount_large_order():
    assert bulk_discount(101) == 0.15
