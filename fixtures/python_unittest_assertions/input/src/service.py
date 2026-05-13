def normalize(value):
    return value.strip().lower()

def reject_negative(amount):
    if amount < 0:
        raise ValueError("negative")
