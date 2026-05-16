import pytest

from src.quantity import normalize_quantity


def test_rejects_zero_quantity():
    with pytest.raises(ValueError):
        normalize_quantity(0)
