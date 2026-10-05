from src.memo import century_index, format_year


def test_format_year_width():
    assert format_year(1999) == "099"


def test_century_index_smoke():
    century_index(1999)
