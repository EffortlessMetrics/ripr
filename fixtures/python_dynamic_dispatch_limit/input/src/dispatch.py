def apply_dynamic(strategy, amount):
    return getattr(strategy, "apply")(amount)
