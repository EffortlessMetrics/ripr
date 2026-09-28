from src.config import get_ttl


def test_ttl():
    assert get_ttl() == 30
