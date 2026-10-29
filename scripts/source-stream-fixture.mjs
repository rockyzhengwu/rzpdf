// Deterministic, project-owned 7-page PDF: >20 MiB of real RGB pixels, <=1024² per image.
export function largeImagePages(){
  const side=1024,count=7;
  const objects=[null,null,Buffer.from('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>')];
  const kids=[];
  for(let page=0;page<count;page++){
    const pixels=Buffer.alloc(side*side*3);
    for(let y=0;y<side;y++)for(let x=0;x<side;x++){
      const offset=(y*side+x)*3;pixels[offset]=(x+page*31)%256;pixels[offset+1]=(y+page*47)%256;pixels[offset+2]=80+page*20;
    }
    const imageId=objects.length+1;
    objects.push(Buffer.concat([Buffer.from(`<< /Type /XObject /Subtype /Image /Width ${side} /Height ${side} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length ${pixels.length} >>\nstream\n`),pixels,Buffer.from('\nendstream')]));
    const pageId=objects.length+1,contentId=pageId+1;kids.push(`${pageId} 0 R`);
    objects.push(Buffer.from(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 400] /Resources << /Font << /F1 3 0 R >> /XObject << /Im1 ${imageId} 0 R >> >> /Contents ${contentId} 0 R >>`));
    const content=Buffer.from(`BT /F1 16 Tf 40 370 Td (Image page ${page+1} of ${count}) Tj ET\nq 320 0 0 280 40 60 cm /Im1 Do Q`);
    objects.push(Buffer.concat([Buffer.from(`<< /Length ${content.length} >>\nstream\n`),content,Buffer.from('\nendstream')]));
  }
  objects[0]=Buffer.from('<< /Type /Catalog /Pages 2 0 R >>');
  objects[1]=Buffer.from(`<< /Type /Pages /Count ${count} /Kids [${kids.join(' ')}] >>`);
  const parts=[Buffer.from('%PDF-1.7\n%\xe2\xe3\xcf\xd3\n','latin1')],offsets=[0];let size=parts[0].length;
  for(const[index,object]of objects.entries()){
    offsets.push(size);const bytes=Buffer.concat([Buffer.from(`${index+1} 0 obj\n`),object,Buffer.from('\nendobj\n')]);parts.push(bytes);size+=bytes.length;
  }
  const xref=size;
  parts.push(Buffer.from(`xref\n0 ${offsets.length}\n0000000000 65535 f \n`+offsets.slice(1).map(n=>`${String(n).padStart(10,'0')} 00000 n \n`).join('')+`trailer\n<< /Size ${offsets.length} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`));
  return Buffer.concat(parts);
}

// CLI: node scripts/source-stream-fixture.mjs /path/to/large.pdf
if (process.argv[1] && (await import('node:url')).pathToFileURL(process.argv[1]).href === import.meta.url) {
  const {writeFile} = await import('node:fs/promises');
  const {createHash} = await import('node:crypto');
  const bytes = largeImagePages();
  const sha256 = createHash('sha256').update(bytes).digest('hex');
  if (bytes.length !== 22023982 || sha256 !== '03be863a438eb2b44709947a5a211a9b94d120e231c92bfaae26be521f1f2551') throw Error('fixture identity changed');
  await writeFile(process.argv[2], bytes);
  console.log(JSON.stringify({bytes: bytes.length, sha256}));
}
