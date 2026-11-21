import pytest

from src.formatter import Formatter


def test_rejects_space_in_key():
    with pytest.raises(ValueError, match='Invalid key'):
        Formatter()({"bad key": "value"})
