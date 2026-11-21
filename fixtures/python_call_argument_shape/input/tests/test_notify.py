from unittest.mock import Mock

from src.notify import send_receipt


def test_send_receipt_notifies_callback():
    notifier = Mock()
    send_receipt(notifier, "ord-123")
    notifier.assert_called_once_with("receipt.sent", "ord-123")
