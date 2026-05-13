import pytest

from src.validation import reject_negative


def test_reject_negative_broad_error():
    with pytest.raises(Exception):
        reject_negative(-1)
