def send_receipt(notifier, order_id):
    notifier("receipt.sent", order_id)
    return True
