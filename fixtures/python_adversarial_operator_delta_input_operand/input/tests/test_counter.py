from src.counter import next_value


def test_next():
    count = 5
    result = next_value(count)
    assert count == 5
    assert result > 0
