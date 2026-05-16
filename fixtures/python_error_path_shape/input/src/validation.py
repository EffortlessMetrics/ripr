def require_positive(value):
    if value <= 0:
        raise ValueError("positive required")
    return value
