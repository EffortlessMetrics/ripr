import pytest

from src.tax import tax_band


@pytest.mark.parametrize("amount", [100, 125])
def test_tax_band(amount):
    tax_band(amount)
