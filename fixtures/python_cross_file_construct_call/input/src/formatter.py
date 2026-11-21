class Formatter:
    def __call__(self, event):
        for key in event:
            if any(c < " " for c in key):
                raise ValueError(f'Invalid key: "{key}"')
        return ",".join(f"{k}={event[k]}" for k in event)
