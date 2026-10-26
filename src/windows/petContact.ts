/** 当前立绘的32×32透明度命中图；不把外层padding或透明图片留白算作接触。 */
export function petContact(face:HTMLElement,canvas:DOMRect) {
  const image=face.querySelector<HTMLImageElement>("img.pet-image-current");
  const expression=face.querySelector<HTMLElement>(".pet-expression")??face;
  const r=(image??expression).getBoundingClientRect();
  let width=r.width,height=r.height,left=r.left,top=r.top;
  let rows=Array<number>(32).fill(0xffffffff);
  if(image){
    rows=Array<number>(32).fill(0);
    if(image.complete&&image.naturalWidth&&image.naturalHeight){
      const scale=Math.min(width/image.naturalWidth,height/image.naturalHeight);
      const w=image.naturalWidth*scale,h=image.naturalHeight*scale;
      left+=(width-w)/2;top+=(height-h)/2;width=w;height=h;
      try{
        const sample=document.createElement("canvas");sample.width=32;sample.height=32;
        const context=sample.getContext("2d",{willReadFrequently:true});
        if(context){context.drawImage(image,0,0,32,32);const rgba=context.getImageData(0,0,32,32).data;
          rows=rows.map((_,y)=>{let bits=0;for(let x=0;x<32;x++)if(rgba[(y*32+x)*4+3]>32)bits|=1<<x;return bits>>>0;});}
      }catch{/* 未就绪或无法读取时不提前宣告追到；下一次load会重新测量。 */}
    }
  }
  return {bounds:{x:left-canvas.left,y:top-canvas.top,width:Math.max(1,width),height:Math.max(1,height)},rows};
}
export function visibleContactBounds(contact:ReturnType<typeof petContact>){
  let left=32,right=-1,top=32,bottom=-1;
  contact.rows.forEach((bits,y)=>{for(let x=0;x<32;x++)if((bits>>>x)&1){left=Math.min(left,x);right=Math.max(right,x);top=Math.min(top,y);bottom=Math.max(bottom,y);}});
  const b=contact.bounds;
  return right<0?b:{x:b.x+b.width*left/32,y:b.y+b.height*top/32,width:b.width*(right-left+1)/32,height:b.height*(bottom-top+1)/32};
}

export function petHintPosition(visible:{x:number;y:number;width:number;height:number},canvas:{width:number;height:number},obstacles:{x:number;y:number;width:number;height:number}[]=[]){
  const right=visible.x+visible.width,bottom=visible.y+visible.height,center=visible.x+(visible.width-58)/2;
  const choices=[{x:right+6,y:visible.y+visible.height*.3},{x:visible.x-64,y:visible.y+visible.height*.3},
    ...[center,visible.x,right-58].map(x=>({x,y:bottom+6})),...[center,visible.x,right-58].map(x=>({x,y:visible.y-36}))];
  const overlaps=(p:{x:number;y:number},b:{x:number;y:number;width:number;height:number})=>p.x<b.x+b.width+3&&p.x+58>b.x-3&&p.y<b.y+b.height+3&&p.y+30>b.y-3;
  const fits=(p:{x:number;y:number})=>p.x>=6&&p.y>=6&&p.x+58<=canvas.width-6&&p.y+30<=canvas.height-2&&!obstacles.some(b=>overlaps(p,b));
  return choices.find(fits)??{x:Math.max(6,Math.min(canvas.width-64,center)),y:Math.max(6,Math.min(canvas.height-30,bottom+6))};
}
