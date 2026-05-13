def test_send_alert_calls_notifier():
    notifier = Mock()
    send_alert(notifier, "sent")
    notifier.send.assert_called_once_with("sent")
