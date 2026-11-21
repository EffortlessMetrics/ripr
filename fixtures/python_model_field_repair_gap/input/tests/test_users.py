from src.users import build_user


def test_build_user_smoke():
    user = build_user()
    assert user
