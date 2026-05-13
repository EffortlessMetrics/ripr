def test_verify_callback():
    callback = Mock()
    callback("sent")
    verify_callback(callback)
    callback.assert_called_once_with("sent")
