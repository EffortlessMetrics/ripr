from src.notify import skip_notify


def test_skip_notify_no_interaction(gateway):
    skip_notify(gateway, True)
    gateway.send.assert_not_called()
