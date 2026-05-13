import pytest

from src.validation import reject_negative


def test_reject_negative_error_kind():
    with pytest.raises(ValueError):
        reject_negative(-1)
