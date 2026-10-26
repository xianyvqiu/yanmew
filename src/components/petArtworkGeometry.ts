export type ArtworkGeometry={top:number;bottom:number;width:number;height:number;visible:number};
/** 所有比例按原图最长边计算，显示尺寸改变不会反向改变透明边界。 */
export function artworkGeometry(width:number,height:number,pixels:Uint8ClampedArray):ArtworkGeometry|null {
  let top=height,bottom=-1;
  for(let y=0;y<height;y++)for(let x=0;x<width;x++)if(pixels[(y*width+x)*4+3]>8){top=Math.min(top,y);bottom=Math.max(bottom,y);break;}
  if(bottom<0)return null;const longest=Math.max(width,height);
  return {top:-top/longest,bottom:(height-bottom-1)/longest,width:width/longest,height:height/longest,visible:(bottom-top+1)/longest};
}
export function readArtworkGeometry(image:HTMLImageElement):ArtworkGeometry|null {
  if(!image.naturalWidth||!image.naturalHeight)return null;
  try{const canvas=document.createElement("canvas");canvas.width=image.naturalWidth;canvas.height=image.naturalHeight;
    const context=canvas.getContext("2d",{willReadFrequently:true});if(!context)return null;
    context.drawImage(image,0,0);return artworkGeometry(canvas.width,canvas.height,context.getImageData(0,0,canvas.width,canvas.height).data);
  }catch{return null;}
}
