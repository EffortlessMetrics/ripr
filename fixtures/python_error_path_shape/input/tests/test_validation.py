import pytest


def test_require_positive_rejects_negative():
    with pytest.raises(ValueError):
        require_positive(-1)
