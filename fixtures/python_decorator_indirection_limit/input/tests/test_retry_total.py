from src.retry_total import fetch_total


def test_fetch_total_uses_retry_path(client):
    assert fetch_total(client) == 10
