import argparse


def main(argv=None):
    parser = argparse.ArgumentParser(prog="ship")
    parser.add_argument("--dry-run", action="store_true")
    parser.parse_args(argv)
    print("shipment queued")
