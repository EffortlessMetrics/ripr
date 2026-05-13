def verify_callback(callback):
    callback.assert_called_once_with("sent")
