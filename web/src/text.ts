import type { TextCharacter, TextPage } from './client.js';
export interface TextMatch { start: number; end: number; }
export type Quad = [number,number,number,number,number,number,number,number];
export function validateQuery(query: string): void {
  if (typeof query !== 'string' || !query.length || query.length > 1024 || query.includes('\0')) throw new RangeError('Search requires 1–1024 characters without NUL');
}
/** Unicode lowercase matching in paint order. Unknown mappings break a match. */
export function searchPage(page: TextPage, query: string, caseSensitive = false, limit = 1000): TextMatch[] {
  validateQuery(query);
  if (typeof caseSensitive !== 'boolean') throw new RangeError('Invalid case sensitivity');
  if (!Number.isInteger(limit) || limit < 1 || limit > 10001) throw new RangeError('Invalid search limit');
  const owners: number[] = [];
  const pieces = page.characters.map((c, index) => {
    const value = c.unicode === null ? '\0' : caseSensitive ? c.unicode : c.unicode.toLowerCase();
    for (let i=0;i<value.length;i++) owners.push(index);
    return value;
  });
  const text=pieces.join(''), needle=caseSensitive ? query : query.toLowerCase();
  const results: TextMatch[]=[];
  for(let offset=0;offset<=text.length-needle.length && results.length<limit;) {
    const start=text.indexOf(needle,offset);
    if(start<0)break;
    results.push({start:owners[start],end:owners[start+needle.length-1]+1});
    offset=start+needle.length;
  }
  return results;
}
/** Logical em/advance selection quad, not a glyph ink outline or clipping test. */
export function characterQuad(c: TextCharacter): Quad | null {
  if(!c.emToDisplay)return null;
  const [a,b,cc,d]=c.emToDisplay;
  const [nx,ny]=c.writingMode==='Vertical' ? [a,b] : [cc,d];
  const [x,y]=c.position,[ax,ay]=c.advance;
  const quad:Quad=[x+nx*.8,y+ny*.8,x+ax+nx*.8,y+ay+ny*.8,x+ax-nx*.2,y+ay-ny*.2,x-nx*.2,y-ny*.2];
  return quad.every(Number.isFinite)?quad:null;
}
