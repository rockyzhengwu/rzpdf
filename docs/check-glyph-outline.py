"""Read-only, deterministic comparison with FontTools for TrueType glyph samples.

Build inspect_glyph_outline first. This does not validate PDF encoding/visibility.
"""
import argparse
import ast
import math
import re
import subprocess

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.pointInsidePen import PointInsidePen
from fontTools.ttLib import TTFont


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("font")
    parser.add_argument("--example", default="target/debug/examples/inspect_glyph_outline")
    parser.add_argument("--characters", default="OAB中一")
    parser.add_argument("--require-composite", action="store_true", help="Require at least one composite sample")
    args = parser.parse_args()
    if not args.characters:
        parser.error("--characters must not be empty")
    with TTFont(args.font) as font:
        glyphs = font.getGlyphSet()
        cmap = font.getBestCmap()
        units = font["head"].unitsPerEm
        checked = boundaries = composites = 0
        for character in args.characters:
            name = cmap[ord(character)]
            if font["glyf"][name].numberOfContours == 0:
                raise ValueError(f"Sample {character!r} must not be empty")
            composite = font["glyf"][name].isComposite()
            composites += int(composite)
            pen = BoundsPen(glyphs)
            glyphs[name].draw(pen)
            if pen.bounds is None:
                raise ValueError(f"Sample {character!r} has no outline")
            expected_bounds = [value / units for value in pen.bounds]
            output = subprocess.run(
                [args.example, args.font, character, "--grid"],
                check=True, capture_output=True, text=True,
            ).stdout.splitlines()
            gid = re.search(r"\bgid=(\d+)\b", output[0])
            if gid is None or int(gid.group(1)) != font.getGlyphID(name):
                raise AssertionError(f"{character!r} GID differs")
            actual_bounds = ast.literal_eval(next(line.split("=", 1)[1] for line in output if line.startswith("bounds_em=")))
            if len(actual_bounds) != 4 or not all(math.isclose(a, b, rel_tol=0, abs_tol=1e-12) for a, b in zip(actual_bounds, expected_bounds)):
                raise AssertionError(f"{character!r} bounds differ: {actual_bounds} vs {expected_bounds}")
            grid = next(line.split("=", 1)[1] for line in output if line.startswith("grid17="))
            if len(grid) != 289 or any(value not in "012" for value in grid):
                raise AssertionError("Invalid grid format")
            sample_checked = sample_boundaries = 0
            for index, value in enumerate(grid):
                # Boundary classification is numerical; FontTools returns bool
                # and provides no equivalent three-way boundary predicate.
                if value == "2":
                    sample_boundaries += 1
                    continue
                y, x = divmod(index, 17)
                b = pen.bounds
                point = (b[0] + (b[2] - b[0]) * (x + 0.5) / 17, b[1] + (b[3] - b[1]) * (y + 0.5) / 17)
                hit = PointInsidePen(glyphs, point, evenOdd=False)
                glyphs[name].draw(hit)
                if (value == "1") != hit.getResult():
                    raise AssertionError(f"{character!r} grid({x},{y}) differs at {point}")
                sample_checked += 1
            checked += sample_checked
            boundaries += sample_boundaries
            print(f"{character}: {'composite' if composite else 'simple'}; bounds match; {sample_checked} points match; {sample_boundaries} boundary points excluded")
        if args.require_composite and not composites:
            raise AssertionError("No composite glyph samples were checked")
        print(f"PASS: {checked} classified points; {boundaries} numerical boundary points excluded; {composites} composite samples")


if __name__ == "__main__":
    main()
