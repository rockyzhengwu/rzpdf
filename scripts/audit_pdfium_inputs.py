#!/usr/bin/env python3
"""Collect independent, bounded input facts for corpus failures; never auto-allowlist.

PyMuPDF probes run in separate address-space/time-limited processes. A probe
failure remains evidence requiring review, not a conformance result.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import zlib


def digest(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        while block:=f.read(1024*1024):h.update(block)
    return h.hexdigest()


def probe(path):
    import fitz
    raw=Path(path).read_bytes()
    anchors=list(re.finditer(rb'startxref\s+(\d+)',raw))
    facts={'decoder':'PyMuPDF '+fitz.VersionBind,'size':len(raw),'header':raw[:96].hex(),
           'startxref':[],'has_eof':b'%%EOF' in raw[-1024:]}
    for match in anchors[-4:]:
        offset=int(match[1]);facts['startxref'].append({'declared':offset,'target':raw[offset:offset+48].hex()})
    try:d=fitz.open(path)
    except Exception as error:
        facts['open_error']=str(error);return facts
    try:facts.update(needs_password=bool(d.needs_pass),pages=d.page_count,xref_length=d.xref_length())
    except Exception as error:
        facts['metadata_error']=str(error);return facts
    if d.needs_pass:return facts
    try:facts['catalog']=d.xref_object(d.pdf_catalog())[:2048]
    except Exception as error:facts['catalog_error']=str(error)
    objects=[]
    facts['null_objects']=[]
    for number in range(1,min(d.xref_length(),8192)):
        try:
            body=d.xref_object(number)
            if not body or body.strip()=='null':
                facts['null_objects'].append(number);continue
            kind=d.xref_get_key(number,'Type')[1];subtype=d.xref_get_key(number,'Subtype')[1]
            if kind not in ('/Font','/Page','/Pages') and subtype not in ('/Image','/Type3','/Form'):continue
            item={'id':number,'dictionary':body[:4096]}
            if subtype=='/Type3':
                refs=re.findall(r'(\d+)\s+(\d+)\s+R',d.xref_get_key(number,'CharProcs')[1])
                item['charproc_prefixes']={obj:d.xref_stream(int(obj))[:96].hex() for obj,_ in refs[:32]}
            if subtype=='/Image':
                encoded=d.xref_stream_raw(number)
                item['encoded_bytes']=len(encoded)
                filters=re.findall(r'/([^\s/\[\]]+)',d.xref_get_key(number,'Filter')[1])
                for name in filters:
                    if name in ('ASCIIHexDecode','AHx'):
                        digits=re.sub(rb'\s',b'',encoded.split(b'>')[0]);digits+=b'0'*(len(digits)%2)
                        encoded=bytes.fromhex(digits.decode('ascii'))
                    elif name in ('ASCII85Decode','A85'):
                        data=encoded.strip();data=data if data.startswith(b'<~') else b'<~'+data
                        encoded=base64.a85decode(data,adobe=True)
                    elif name in ('FlateDecode','Fl'):
                        decoder=zlib.decompressobj();encoded=decoder.decompress(encoded,16*1024*1024+1)
                        if len(encoded)>16*1024*1024:
                            item['probe_decoded_limit']=True;break
                    else:break
                item.update(byte_filter_output_length=len(encoded),payload_prefix=encoded[:80].hex())
                soc=encoded.find(b'\xff\x4f\xff\x51')
                if soc>=0 and len(encoded)>=soc+42:
                    siz=encoded[soc:];item['jpx_siz']={'grid':[int.from_bytes(siz[i:i+4],'big') for i in range(8,40,4)],'components':int.from_bytes(siz[40:42],'big'),'component_bytes':siz[42:42+3*int.from_bytes(siz[40:42],'big')].hex()}
                if 'JBIG2Decode' in filters:
                    segments=[];offset=0
                    while offset+11<=len(encoded) and len(segments)<1024:
                        start=offset;num=int.from_bytes(encoded[offset:offset+4],'big');flags=encoded[offset+4];refs=encoded[offset+5]>>5
                        if refs>4:break
                        refsize=1 if num<=256 else 2 if num<=65536 else 4
                        offset+=6+refs*refsize+(4 if flags&64 else 1)
                        length=int.from_bytes(encoded[offset:offset+4],'big');offset+=4
                        segment={'type':flags&63,'declared_bytes':length,'available_bytes':len(encoded)-offset}
                        if flags&63==48 and offset+8<=len(encoded):segment['page_dimensions']=[int.from_bytes(encoded[offset:offset+4],'big'),int.from_bytes(encoded[offset+4:offset+8],'big')]
                        segments.append(segment)
                        if length==0xffffffff or length>len(encoded)-offset:break
                        offset+=length
                        if offset<=start:break
                    item['jbig2_segments']=segments
            objects.append(item)
        except Exception as error:objects.append({'id':number,'probe_error':str(error)})
    facts['objects']=objects
    facts['native_warnings']=fitz.TOOLS.mupdf_warnings()
    d.close();return facts


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--probe',type=Path)
    parser.add_argument('--run',type=Path)
    parser.add_argument('--output',type=Path)
    args=parser.parse_args()
    if args.probe:
        print(json.dumps(probe(str(args.probe))));return
    args.output.mkdir(parents=True,exist_ok=False)
    rows=list(map(json.loads,(args.run/'results.jsonl').read_text().splitlines()))
    records=[]
    for row in rows:
        failed=row['parsing']['status']=='error' or any(p['rzpdf']['status']!='ok' for p in row['renders'])
        if not failed:continue
        if digest(row['path'])!=row['sha256']:raise ValueError('input identity changed')
        record={k:row[k] for k in ('id','source','sha256')}
        try:
            result=subprocess.run(['prlimit','--as=1073741824','--',sys.executable,__file__,'--probe',row['path']],capture_output=True,text=True,timeout=10)
            record.update(returncode=result.returncode,stderr=result.stderr[-4096:])
            if result.returncode==0:
                try:record['facts']=json.loads(result.stdout.strip().splitlines()[-1])
                except (ValueError,IndexError):record.update(probe_status='invalid_probe_output',stdout=result.stdout[-4096:])
            else:record['probe_status']='failed'
        except subprocess.TimeoutExpired:record['probe_status']='timeout'
        record['interpretation_error']=row['parsing']['details']
        record['render_errors']=[p['rzpdf']['stderr'] for p in row['renders'] if p['rzpdf']['status']!='ok']
        record['adjudication']='needs_review'
        records.append(record)
        (args.output/f"{row['id']}.json").write_text(json.dumps(record,indent=2)+'\n')
    summary={'inputs':len(records),'probe_successes':sum('facts' in r for r in records),'probe_failures':sum('facts' not in r for r in records),'script_sha256':digest(__file__),'run':str(args.run),'complete':False}
    (args.output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary))


if __name__=='__main__':main()
