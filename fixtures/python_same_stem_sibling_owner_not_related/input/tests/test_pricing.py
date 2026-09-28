from pricing import discounted_total


def test_no_discount_below_threshold():
    assert discounted_total(5000) == 5000


def test_discounts_far_above_threshold():
    assert discounted_total(20000) == 18000
