from src.render import render


def test_render_explicit_verbose_false():
    assert render("Sam", verbose=False) == "Sam"
