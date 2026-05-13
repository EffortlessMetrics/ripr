import unittest

from src.service import normalize, reject_negative


class ServiceTests(unittest.TestCase):
    def test_normalize_exact_value(self):
        self.assertEqual(normalize(" PAID "), "paid")

    def test_reject_negative_error_kind(self):
        self.assertRaises(ValueError, reject_negative, -1)
