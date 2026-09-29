from src.pricing import is_large


def test_large_order():
    assert is_large(500) == True
