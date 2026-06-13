import unittest

from src.stop import stop_after_attempt


class StopTest(unittest.TestCase):
    def test_stop_after_attempt(self):
        stop = stop_after_attempt(3)
        self.assertTrue(stop(3))
