from unittest.mock import patch

from src.service import fetch_total


@patch("src.service.remote_total")
def test_fetch_total_uses_client_total(mock_remote, client):
    assert fetch_total(client) == 10
