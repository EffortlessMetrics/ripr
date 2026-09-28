from src.pricing import discount


def test_discount_passthrough():
    assert discount(100) == 100
