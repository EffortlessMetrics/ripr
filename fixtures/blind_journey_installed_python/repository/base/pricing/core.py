"""Bulk discount policy owned by the fixture."""


def bulk_discount_rate(units):
    """Return the bulk discount rate for one order size.

    The boundary is exclusive: an order of exactly 20 units stays at the
    standard rate.
    """
    if units > 20:
        return 0.15
    return 0.0
