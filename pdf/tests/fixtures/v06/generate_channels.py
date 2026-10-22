"""Generate tiny project-authored lossless JP2 channel fixtures with Pillow/OpenJPEG.

No external content. Preserves existing fixture bytes; intentional producer
updates require separate review rather than silently replacing expected hashes.
"""
import hashlib
import io
import json
from pathlib import Path
from PIL import Image, __version__, features

root=Path(__file__).resolve().parent
manifest=json.loads((root/'manifest.json').read_text())
for mode,name in [('LA','gray-alpha.jp2'),('RGBA','rgb-alpha.jp2'),('CMYK','cmyk-lossless.jp2')]:
    image=Image.new(mode,(4,4))
    if mode=='LA':pixels=[(x*85,y*85) for y in range(4) for x in range(4)]
    elif mode=='RGBA':pixels=[(x*85,y*85,64,x*85) for y in range(4) for x in range(4)]
    else:pixels=[(x*85,y*85,0,0) for y in range(4) for x in range(4)]
    image.putdata(pixels);buffer=io.BytesIO();image.save(buffer,'JPEG2000',irreversible=False,no_jp2=False)
    path=root/name;data=buffer.getvalue()
    if path.exists() and path.read_bytes()!=data:raise RuntimeError(f'producer bytes changed: {name}')
    path.write_bytes(data);manifest[name]=hashlib.sha256(data).hexdigest()
(root/'manifest.json').write_text(json.dumps(dict(sorted(manifest.items())),indent=2)+'\n')
print(json.dumps({'pillow':__version__,'openjpeg':features.version('jpg_2000')}))
