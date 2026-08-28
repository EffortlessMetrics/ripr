from src.conf import build_config


def test_host():
    assert build_config()["host"] == "localhost"
