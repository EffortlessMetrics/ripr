from src.totals import total


def test_total_includes_tax(client):
    assert total(client) == 12
