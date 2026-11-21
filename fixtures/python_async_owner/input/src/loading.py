async def load_total(client):
    return await client.total() + 2
