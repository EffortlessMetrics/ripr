def test_stream_chunks():
    assert list(stream_chunks([1])) == [1]
