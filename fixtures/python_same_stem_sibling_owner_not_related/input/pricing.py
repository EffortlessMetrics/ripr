DISCOUNT_THRESHOLD = 10000


def discounted_total(amount):
    if amount >= DISCOUNT_THRESHOLD:
        return amount * 9 // 10
    return amount


def loyalty_price(amount, years):
    if years >= 5:
        return amount * 95 // 100
    return amount
