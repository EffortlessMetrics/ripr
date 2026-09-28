def test_expired_coupon_response_smoke(client):
    response = client.post("/checkout", json={"expired": True})
    assert response
