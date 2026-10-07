"""Read-only pixel check for the fixed create_transparency example's MuPDF PPM."""
import sys

if len(sys.argv) != 2:
    raise SystemExit("Usage: python3 docs/check-transparency-pixels.py example.ppm")

header = b"P6\n300 100\n255\n"
size = 300 * 100 * 3
with open(sys.argv[1], "rb") as source:
    data = source.read(len(header) + size + 1)
if not data.startswith(header) or len(data) != len(header) + size:
    raise SystemExit("Expected exact MuPDF P6 300x100 RGB output")
pixels = data[len(header):]
for x, expected in [(50, (255, 255, 255)), (150, (227, 147, 137)), (250, (200, 40, 20))]:
    offset = (50 * 300 + x) * 3
    actual = tuple(pixels[offset:offset + 3])
    if any(abs(a - e) > 1 for a, e in zip(actual, expected)):
        raise SystemExit(f"Pixel ({x},50): {actual}, expected {expected}, tolerance 1")
    print(f"Pixel ({x},50): {actual}, expected {expected}, passed")
