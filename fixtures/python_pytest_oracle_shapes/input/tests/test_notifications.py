from src.notifications import warn_coupon


def test_warn_coupon_logs_expired_message(caplog):
    warn_coupon("expired")
    assert "coupon expired" in caplog.text
