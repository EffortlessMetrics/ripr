import unittest


class ServiceTest(unittest.TestCase):
    def test_normalize(self):
        self.assertEqual(normalize(" ABC "), "abc")
