async def test_load_price(client):
    assert await load_price(client) > 0
