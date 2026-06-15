from src.checker import validate


def test_checker_validate():
    # Strong exact-value oracle (observes the changed value "ok") so the guard is
    # genuinely exercised — a weak/truthy oracle would early-return before it.
    assert validate("ok") == "ok"
