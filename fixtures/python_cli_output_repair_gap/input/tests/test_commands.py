from src.commands import ship


def test_ship_smoke(capsys):
    ship()
    captured = capsys.readouterr()
    assert captured.out
