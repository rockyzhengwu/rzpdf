/** Device pixels after the PDF page's own rotation/CropBox/UserUnit. */
export interface RasterRegion {x:number;y:number;width:number;height:number;}
export const MAX_REGION_SIDE=1024;
export const MAX_RASTER_SIDE=32768;
export const MAX_FULL_PAGE_DPI=288;
export const MAX_REGION_DPI=1152;
export function normalizeRasterRegion(value:RasterRegion):RasterRegion {
  if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(k=>!['x','y','width','height'].includes(k)))throw new RangeError('Invalid raster region');
  const {x,y,width,height}=value;
  if(![x,y,width,height].every(Number.isSafeInteger)||x<0||y<0||x>MAX_RASTER_SIDE||y>MAX_RASTER_SIDE||width<1||height<1||width>MAX_REGION_SIDE||height>MAX_REGION_SIDE)throw new RangeError('Invalid raster region');
  return {x,y,width,height};
}
/** Cover the display extent; match native f32 inputs and f64 multiply/divide. */
export function rasterSize(width:number,height:number,dpi=72):{width:number;height:number} {
  if(![width,height,dpi].every(Number.isFinite)||width<=0||height<=0||dpi<1||dpi>MAX_REGION_DPI)throw new RangeError('Invalid raster dimensions');
  const w=Math.ceil(Math.fround(width)*Math.fround(dpi)/72),h=Math.ceil(Math.fround(height)*Math.fround(dpi)/72);
  if(w<1||h<1||w>MAX_RASTER_SIDE||h>MAX_RASTER_SIDE)throw new RangeError('Raster dimensions exceed region limits');
  return {width:w,height:h};
}
