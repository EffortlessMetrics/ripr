import unittest

from src.validation import require_positive


class TestValidation(unittest.TestCase):
    def test_rejects_zero_value(self):
        with self.assertRaises(ValueError):
            require_positive(0)
