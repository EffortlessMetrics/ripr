from src.routes import route_order


def test_first():
    assert route_order()[0] == "index"
