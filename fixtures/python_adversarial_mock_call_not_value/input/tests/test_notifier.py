from unittest.mock import MagicMock

from src.notifier import send_alert


def test_send_alert_calls_post():
    client = MagicMock()
    send_alert(client, "error")
    client.post.assert_called_once()
