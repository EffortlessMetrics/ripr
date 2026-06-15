from src.pack import pack


def test_pack_returns_list():
    buffered_output = pack([1, 2], 5)
    assert buffered_output == [1, 2]
