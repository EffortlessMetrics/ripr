def parse(text):
    if not text:
        raise KeyError("empty")
    return int(text)
