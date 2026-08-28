from src.pricing import discount


def test_discount():
    assert discount(100) == 80
