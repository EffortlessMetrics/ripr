from src.parseint import parse


def test_parse_ok():
    assert parse("42") == 42
