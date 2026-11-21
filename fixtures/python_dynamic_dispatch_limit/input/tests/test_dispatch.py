from src.dispatch import call_named


def test_call_named_dispatches_total(client):
    assert call_named(client, "total") == 10
