def require_positive(amount):
    if amount < 0:
        raise ValueError("amount must be positive")
    return amount
