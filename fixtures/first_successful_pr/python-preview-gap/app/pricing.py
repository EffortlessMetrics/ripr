def calculate_discount(amount, threshold):
    if amount >= threshold:
        return amount * 0.9
    return amount
