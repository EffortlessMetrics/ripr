async def load_price(client):
    return await client.price_with_tax()
