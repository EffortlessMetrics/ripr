from src.a_dynamic import call_named


class Client:
    def total(self):
        return 5


def test_call_named_exact():
    assert call_named(Client(), "total") == 5
