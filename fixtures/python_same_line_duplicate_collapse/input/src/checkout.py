def checkout_payload(order_id):
    return {"status": "paid", "event": "receipt.sent", "id": order_id}

