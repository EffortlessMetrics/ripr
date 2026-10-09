from pricing import bulk_discount_rate


def test_below_boundary_stays_standard():
    assert bulk_discount_rate(19) == 0.0


def test_above_boundary_gets_discount():
    assert bulk_discount_rate(21) == 0.15
