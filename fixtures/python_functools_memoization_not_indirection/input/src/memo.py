from functools import lru_cache


@lru_cache
def format_year(year):
    return "%03d" % (year % 100)


@lru_cache(maxsize=None)
def century_index(year):
    return year % 1000
