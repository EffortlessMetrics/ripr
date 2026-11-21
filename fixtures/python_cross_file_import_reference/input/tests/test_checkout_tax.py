from src.tax import apply_tax as taxed


def test_checkout_tax_alias_import():
    assert taxed(10) == 12
