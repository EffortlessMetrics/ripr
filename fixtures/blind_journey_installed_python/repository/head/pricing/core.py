"""Bulk discount policy owned by the fixture."""


def bulk_discount_rate(units):
    """Return the bulk discount rate for one order size.

    The boundary is inclusive: an order of exactly 20 units now qualifies for
    the bulk discount.
    """
    if units >= 20:
        return 0.15
    return 0.0
