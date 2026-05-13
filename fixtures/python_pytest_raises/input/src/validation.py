def reject_negative(amount):
    if amount < 0:
        raise ValueError("negative")
    return amount
