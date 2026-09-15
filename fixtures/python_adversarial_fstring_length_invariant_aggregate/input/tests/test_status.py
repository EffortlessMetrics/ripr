from src.status import status_label


def test_len():
    assert len(status_label(7)) == 4
