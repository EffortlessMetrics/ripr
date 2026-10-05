<!-- section: Changed -->
- `ripr check` human output now says when a Rust change has no static test path
  in a repo that also has Python test files (`test_*.py`, `*_test.py`, or any
  `.py` under `tests/` or `test/`): the closing note names how many there are and
  that ripr does not link Python tests to Rust changes, so a change they alone
  cover reads as no static path. Verdicts, classes, JSON, SARIF and gate output
  are unchanged. Virtualenvs and tool caches (`.venv`, `venv`, `site-packages`,
  `.tox`, `__pycache__`) are not counted (#6340).
