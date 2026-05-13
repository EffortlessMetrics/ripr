from unittest.mock import MagicMock


def recording_callback():
    callback = MagicMock(name="receipt.sent")
    return callback
