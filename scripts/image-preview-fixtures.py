"""Two-page, 5,106,140-pixel image previews; no external assets."""
import io
import re
import sys
import zlib
from pathlib import Path
from PIL import Image, ImageDraw

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
image = Image.new('RGB', (1916, 2665), (34, 96, 180))
draw = ImageDraw.Draw(image)
draw.rectangle((150, 200, 1750, 900), fill=(230, 180, 60))
for index in range(8):
    draw.rectangle((160, 1100 + index * 160, 1750, 1160 + index * 160), fill=(240, 245, 250))

for codec in ('jpeg', 'progressive', 'flate'):
    if codec == 'flate':
        payload = zlib.compress(image.tobytes())
        filter_name = 'FlateDecode'
    else:
        buffer = io.BytesIO()
        image.save(buffer, format='JPEG', quality=85, progressive=codec == 'progressive')
        payload = buffer.getvalue()
        filter_name = 'DCTDecode'
    content = b'q 595 0 0 842 0 0 cm /Im1 Do Q'
    first = b'0.1 0.4 0.7 rg 50 600 400 100 re f'
    objects = [b'<< /Type /Catalog /Pages 2 0 R >>', b'<< /Type /Pages /Count 2 /Kids [3 0 R 5 0 R] >>',
               b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << >> /Contents 4 0 R >>',
               b'<< /Length ' + str(len(first)).encode() + b' >>\nstream\n' + first + b'\nendstream',
               b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /XObject << /Im1 7 0 R >> >> /Contents 6 0 R >>',
               b'<< /Length ' + str(len(content)).encode() + b' >>\nstream\n' + content + b'\nendstream',
               f'<< /Type /XObject /Subtype /Image /Width 1916 /Height 2665 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /{filter_name} /Length {len(payload)} >>\nstream\n'.encode() + payload + b'\nendstream']
    pdf = bytearray(b'%PDF-1.7\n')
    offsets = [0]
    for index, obj in enumerate(objects):
        offsets.append(len(pdf))
        pdf.extend(f'{index + 1} 0 obj\n'.encode() + obj + b'\nendobj\n')
    xref = len(pdf)
    pdf.extend(b'xref\n0 8\n0000000000 65535 f \n')
    for offset in offsets[1:]:
        pdf.extend(f'{offset:010} 00000 n \n'.encode())
    pdf.extend(f'trailer\n<< /Size 8 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
    (out / f'preview-{codec}.pdf').write_bytes(pdf)

# Structurally identical negative fixture: fail the final zlib checksum after
# producing all image samples, before a preview may be published.
source = bytearray((out / 'preview-flate.pdf').read_bytes())
image_start = source.index(b'7 0 obj')
payload_start = source.index(b'stream\n', image_start) + len(b'stream\n')
length = int(re.search(rb'/Length (\d+)', source[image_start:payload_start]).group(1))
source[payload_start + length - 1] ^= 1
(out / 'preview-flate-corrupt.pdf').write_bytes(source)
