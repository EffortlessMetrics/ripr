from src.catalog import calculate_total


def test_checkout_total_includes_fee():
    assert calculate_total(10) == 17
