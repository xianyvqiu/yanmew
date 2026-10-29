import {cleanup,render} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {useRef} from "react";
import {usePetPhysics} from "./usePetPhysics";
import type {Toss} from "./usePetDesktop";
let reduced=false;
const animations:{cancel:ReturnType<typeof vi.fn>}[]=[];
const animate=vi.fn((_frames:Keyframe[]|PropertyIndexedKeyframes|null,_options?:number|KeyframeAnimationOptions)=>{const value={cancel:vi.fn()};animations.push(value);return value as unknown as Animation;});
beforeEach(()=>{animations.length=0;animate.mockClear();reduced=false;vi.stubGlobal("matchMedia",()=>({matches:reduced}));HTMLElement.prototype.animate=animate;});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
const flight:Toss={phase:"up",personaId:"1",sessionId:"1",contextRevision:0,controlsBelow:false,body:{x:60,y:100,width:196,height:196},velocity:{x:900,y:-1200}};
function Fixture({drag=false,x=0,toss=null,enabled=true}:{drag?:boolean;x?:number;toss?:Toss|null;enabled?:boolean}){const root=useRef<HTMLDivElement>(null);usePetPhysics(root,drag,{x,y:-100},toss,enabled);return <div ref={root}><span className="pet-expression">角色</span><button>按钮</button></div>;}
it("倾斜只影响立绘，反向拖动衔接旧动画并保持按钮和布局",()=>{
 const result=render(<Fixture drag x={1200}/>);expect(animate.mock.instances[0]).toBe(result.container.querySelector(".pet-expression"));
 expect(animate.mock.calls[0][0]).toEqual(expect.arrayContaining([expect.objectContaining({rotate:expect.stringMatching(/^18/)} )]));
 result.rerender(<Fixture drag x={-1200}/>);expect(animations[0].cancel).toHaveBeenCalledOnce();
 expect(result.container.querySelector("button")?.getAttribute("style")).toBeNull();expect(result.container.firstElementChild?.getAttribute("style")).toBeNull();
 result.unmount();expect(animations[1].cancel).toHaveBeenCalledOnce();
});
it("落地回弹只播一次，结束和不可互动状态归位",()=>{
 const result=render(<Fixture toss={flight}/>);result.rerender(<Fixture toss={{...flight,phase:"landed",velocity:{x:0,y:0}}}/>);
 expect(animate.mock.calls[1][0]).toHaveLength(5);expect(animate.mock.calls[1][1]).toMatchObject({duration:620});
 result.rerender(<Fixture toss={{...flight,phase:"landed",velocity:{x:0,y:0}}}/>);expect(animate).toHaveBeenCalledTimes(2);
 result.rerender(<Fixture toss={flight} enabled={false}/>);expect(animate.mock.calls[2][0]).toEqual(expect.arrayContaining([{rotate:"0deg",scale:"1 1"}]));
});
it("减少动态效果时不旋转、不拉伸，也不播放落地挤压",()=>{
 reduced=true;render(<Fixture drag x={1800}/>);expect(animate.mock.calls[0][0]).toEqual(expect.arrayContaining([{rotate:"0deg",scale:"1 1"}]));expect(animate.mock.calls[0][1]).toMatchObject({duration:1});
});
