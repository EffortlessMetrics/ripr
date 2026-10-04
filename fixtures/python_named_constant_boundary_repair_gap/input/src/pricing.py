DISCOUNT_THRESHOLD = 10_000


def discounted_total(amount):
    if amount >= DISCOUNT_THRESHOLD:
        return amount - amount // 10
    return amount
