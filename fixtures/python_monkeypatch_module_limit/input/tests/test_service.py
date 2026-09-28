from src.service import fetch_total


def test_fetch_total_monkeypatches_remote_total(monkeypatch):
    monkeypatch.setattr("src.service.remote_total", lambda: 10)
    assert fetch_total() == 10
