def loyalty_discount(subtotal, loyalty_threshold):
    if subtotal >= loyalty_threshold:
        return subtotal - 5
    return subtotal
