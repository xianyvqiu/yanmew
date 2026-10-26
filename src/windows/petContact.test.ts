import {afterEach,expect,it,vi} from "vitest";
import {petContact,visibleContactBounds,petHintPosition} from "./petContact";
afterEach(()=>vi.restoreAllMocks());
it("只报告不透明像素，保留图片实际contain尺寸",()=>{
 const face=document.createElement("div"),img=document.createElement("img");img.className="pet-image-current";face.append(img);
 Object.defineProperties(img,{complete:{value:true},naturalWidth:{value:100},naturalHeight:{value:200}});
 vi.spyOn(img,"getBoundingClientRect").mockReturnValue({left:100,top:120,width:200,height:200} as DOMRect);
 const rgba=new Uint8ClampedArray(32*32*4);for(let y=8;y<24;y++)for(let x=12;x<20;x++)rgba[(y*32+x)*4+3]=255;
 vi.spyOn(HTMLCanvasElement.prototype,"getContext").mockReturnValue({drawImage:vi.fn(),getImageData:()=>({data:rgba})} as unknown as CanvasRenderingContext2D);
 const contact=petContact(face,{left:0,top:0} as DOMRect);expect(contact.bounds).toEqual({x:150,y:120,width:100,height:200});expect(contact.rows[0]).toBe(0);expect(contact.rows[10]).toBe(0x000ff000);
 expect(visibleContactBounds(contact)).toEqual({x:187.5,y:170,width:25,height:100});
});
it("图片未就绪时不把空白当作接触",()=>{
 const face=document.createElement("div"),img=document.createElement("img");img.className="pet-image-current";face.append(img);vi.spyOn(img,"getBoundingClientRect").mockReturnValue({left:0,top:0,width:100,height:100} as DOMRect);
 expect(petContact(face,{left:0,top:0} as DOMRect).rows.every(row=>row===0)).toBe(true);
});

it("宽立绘旁边放不下时，提示放到下方留白",()=>{
 const pos=petHintPosition({x:16,y:60,width:288,height:200},{width:320,height:320});expect(pos.y).toBe(266);expect(pos.x).toBeGreaterThan(16);expect(pos.x+58).toBeLessThan(304);
});

it("下方居中有聊天按钮时，改在下方侧边留白显示",()=>{
 const p=petHintPosition({x:16,y:60,width:288,height:200},{width:320,height:320},[{x:144,y:270,width:32,height:32}]);expect(p.x+58<=141||p.x>=179).toBe(true);expect(p.y).toBe(266);
});
