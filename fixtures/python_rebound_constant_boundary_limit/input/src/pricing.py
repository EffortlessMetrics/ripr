DISCOUNT_THRESHOLD = 10_000


def configure(threshold):
    global DISCOUNT_THRESHOLD
    DISCOUNT_THRESHOLD = threshold


def discounted_total(amount):
    if amount >= DISCOUNT_THRESHOLD:
        return amount - amount // 10
    return amount
