from src.calc import compute


def test_base_unchanged():
    base = 10
    compute(base, 3)
    assert base == 10
