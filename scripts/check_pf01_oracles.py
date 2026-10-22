#!/usr/bin/env python3
"""Verify PF-01 stripe sampling and radial geometry without a reference rasterizer.

Uses the pinned check_render worker and input identities from a corpus run.
Requires Pillow and PyMuPDF for independent source inspection, not rendering.
"""
import argparse
from decimal import Decimal, localcontext
import hashlib
import json
from pathlib import Path
import struct
import subprocess

from PIL import Image
import fitz


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def stripe_differences(image, dpi):
    """Exact nearest-center oracle, allowing either neighbor only at exact ties."""
    width, height = image.size
    expected_size = round(64*dpi/72)
    if image.size != (expected_size, expected_size):
        raise ValueError('unexpected rendered dimensions')
    pixels = image.convert('RGBA')
    differences = 0
    ties = 0
    for y in range(height):
        source = (y+0.5)*72/dpi
        values = {255 if int(source) % 2 == 0 else 0}
        if source == int(source) and source > 0:
            values.add(255 if (int(source)-1) % 2 == 0 else 0)
            ties += width
        for x in range(width):
            pixel = pixels.getpixel((x,y))
            if pixel[3]!=255 or pixel[0] not in values or pixel[:3]!=(pixel[0],)*3:
                differences += 1
    return dict(width=width,height=height,differing_pixels=differences,tie_pixels=ties)


def radial_geometry():
    # Input coordinates are stored as f32 by the core. Decimal arithmetic
    # preserves those exact values; geometric ray intersections avoid the
    # implementation's quadratic-root formula entirely.
    coordinates = [struct.unpack('f',struct.pack('f',v))[0] for v in (-.223151,-.974784)]
    with localcontext() as ctx:
        ctx.prec=70
        x,y=map(Decimal.from_float,coordinates)
        norm2=x*x+y*y
        norm=norm2.sqrt()
        if not norm>1:raise ValueError('expected slightly external starting point')
        # For P=k*S, k>1: |P-C(t)|-r(t)=(k-1)*|S|+t*(|S|-1)>0
        # for every t>=0. At P=0 the larger solution is |S|/(|S|-1)>1.
        return dict(stored_start=[str(x),str(y)],distance_squared_minus_one=str(norm2-1),
                    outward_ray_coverage=False,end_center_largest_parameter=str(norm/(norm-1)),
                    end_center_uses_extended_end_color=True,
                    proof='For k>1 and t>=0, (k-1)*norm+t*(norm-1)>0; center intersections follow directly from circle distance.')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    settings=json.loads((args.run/'settings.json').read_text())
    rows=[json.loads(l) for l in (args.run/'results.jsonl').read_text().splitlines()]
    source=next(row for row in rows if row['id']=='0420')
    binary=(args.run/'workers/check_render').resolve()
    if sha(binary)!=settings['workers']['check_render'] or sha(source['path'])!=source['sha256']:
        raise ValueError('corpus input or pinned renderer changed')
    with fitz.open(source['path']) as doc:
        images=doc[0].get_images()
        if len(images)!=1:raise ValueError('expected one stripe image')
        image=images[0]
        if image[2:5]!=(64,64,1):raise ValueError('stripe geometry changed')
        data=doc.xref_stream(image[0])
        expected=b''.join(bytes([255 if row%2==0 else 0])*8 for row in range(64))
        if data!=expected:raise ValueError('independently decoded stripe samples changed')
        interpolate=doc.xref_get_key(image[0],'Interpolate')
        if interpolate not in [('null','null'),('bool','false')]:raise ValueError('nearest oracle requires Interpolate false')
    args.output.mkdir(parents=True,exist_ok=False)
    report=dict(input_sha256=source['sha256'],renderer_sha256=sha(binary),
                oracle_script_sha256=sha(__file__),source_samples_sha256=hashlib.sha256(data).hexdigest(),
                independent_source_decoder=fitz.VersionBind,radial=radial_geometry(),renders=[])
    for dpi in [48,72,144]:
        output=(args.output/f'stripes-{dpi}.png').resolve()
        command=['prlimit','--as=2147483648','--',str(binary),source['path'],str(output),'1','compatible',str(dpi)]
        result=subprocess.run(command,capture_output=True,text=True,timeout=30)
        if result.returncode:raise RuntimeError(result.stderr)
        with Image.open(output) as image: metrics=stripe_differences(image,dpi)
        report['renders'].append(dict(dpi=dpi,png_sha256=sha(output),**metrics))
    report['status']='passed' if all(r['differing_pixels']==0 for r in report['renders']) else 'failed'
    (args.output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(status=report['status'],renders=report['renders'],report=str(args.output/'report.json'))))
    if report['status']!='passed':raise SystemExit(1)


if __name__=='__main__':main()
