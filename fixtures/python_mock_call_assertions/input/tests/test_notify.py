from src.notify import notify_customer


def test_notify_customer_interaction(gateway):
    notify_customer(gateway)
    gateway.send.assert_called_once_with("paid")
