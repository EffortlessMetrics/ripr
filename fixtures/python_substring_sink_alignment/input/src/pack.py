def pack(items, limit):
    buffer = list(items)
    if len(buffer) <= limit:
        return buffer
    raise ValueError("too many items")
