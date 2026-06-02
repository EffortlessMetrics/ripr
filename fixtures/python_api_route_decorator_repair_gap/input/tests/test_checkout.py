from app.checkout import checkout


def test_expired_coupon_response_smoke():
    response = checkout(True)
    assert response
