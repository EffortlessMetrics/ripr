def normalize_quantity(value):
    if value <= 0:
        raise ValueError("quantity must be positive")
    return value
