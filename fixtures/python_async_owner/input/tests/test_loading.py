from src.loading import load_total


async def test_load_total_includes_fee(client):
    assert await load_total(client) == 12
