from src.mapper import make_mapper


def test_make_mapper_adds_two():
    assert make_mapper()(1) == 3
