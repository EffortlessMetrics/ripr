def send_receipt(callback, order_id):
    callback("receipt.sent", order_id)
    return True
