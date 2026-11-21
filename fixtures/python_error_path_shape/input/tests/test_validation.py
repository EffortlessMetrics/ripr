import pytest

from src.validation import require_positive


def test_rejects_zero_value():
    with pytest.raises(ValueError):
        require_positive(0)
