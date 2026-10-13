"""Rename a reduced Lato test font so it does not use the reserved font name.

Usage: python3 build-v03-lato-fixture.py INPUT.ttf OUTPUT.ttf
Requires FontTools 4.39.4. Run after the fixed `fonttools subset` command in
pdf/tests/fixtures/v03/README.md.
"""

import argparse
from fontTools.ttLib import TTFont


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input")
    parser.add_argument("output")
    args = parser.parse_args()
    font = TTFont(args.input)
    font.recalcTimestamp = False
    names = {
        1: "Rzpdf Test Sans",
        3: "rzpdf-v03;RzpdfTestSans-Regular",
        4: "Rzpdf Test Sans Regular",
        6: "RzpdfTestSans-Regular",
    }
    table = font["name"]
    for record in list(table.names):
        replacement = names.get(record.nameID)
        if replacement is not None:
            table.setName(
                replacement,
                record.nameID,
                record.platformID,
                record.platEncID,
                record.langID,
            )
    font.save(args.output)


if __name__ == "__main__":
    main()
