from src.pricing import discounted_total


def test_no_discount_below_threshold():
    assert discounted_total(5_000) == 5_000


def test_discount_far_above_threshold():
    assert discounted_total(20_000) == 18_000
