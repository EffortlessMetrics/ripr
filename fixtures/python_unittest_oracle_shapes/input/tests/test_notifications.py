import unittest

from src.notifications import warn_coupon


class NotificationTests(unittest.TestCase):
    def test_warn_coupon_output_message(self):
        result = type("Result", (), {"output": warn_coupon("expired")})()
        self.assertIn("coupon expired", result.output)
