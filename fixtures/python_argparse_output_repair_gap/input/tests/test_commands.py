from src.commands import main


def test_main_smoke(capsys):
    main([])
    captured = capsys.readouterr()
    assert captured.out
