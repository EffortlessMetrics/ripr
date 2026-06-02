from src.b_observed import calculate_fee


def test_calculate_fee_exact():
    assert calculate_fee(100) == 105
