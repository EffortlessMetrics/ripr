from src.pack import pack


def test_pack_returns_buffered_output():
    buffered_output = pack([1, 2], 5)
    assert buffered_output == [1, 2]
