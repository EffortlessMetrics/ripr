from unittest.mock import Mock

from src.notify import send_receipt


def test_send_receipt_notifies_callback():
    callback = Mock()
    send_receipt(callback, "ord-123")
    callback.assert_called_once_with("receipt.sent", "ord-123")
