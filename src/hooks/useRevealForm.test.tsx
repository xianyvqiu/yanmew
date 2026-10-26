import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {useRevealForm} from "./useRevealForm";
let frame:FrameRequestCallback|undefined;
const scroll=vi.fn(),cancel=vi.fn();
function Form({open=true,id="1"}:{open?:boolean;id?:string}) {
  const ref=useRevealForm(open,id);
  return open?<form ref={ref}><input type="hidden"/><input disabled/><input aria-label="名称" defaultValue={id}/></form>:null;
}
beforeEach(()=>{
  frame=undefined;scroll.mockReset();cancel.mockReset();
  vi.stubGlobal("requestAnimationFrame",vi.fn((callback:FrameRequestCallback)=>{frame=callback;return 7;}));
  vi.stubGlobal("cancelAnimationFrame",cancel);
  vi.stubGlobal("matchMedia",vi.fn(()=>({matches:false})));
  Object.defineProperty(HTMLElement.prototype,"scrollIntoView",{configurable:true,value:scroll});
});
afterEach(()=>{cleanup();vi.unstubAllGlobals();delete (HTMLElement.prototype as Partial<HTMLElement>).scrollIntoView;});
it("打开后等待布局完成再滚动并聚焦首个可用输入框",()=>{
  render(<Form/>);expect(scroll).not.toHaveBeenCalled();frame?.(0);
  expect(scroll).toHaveBeenCalledWith({block:"start",behavior:"smooth"});expect(document.activeElement).toBe(screen.getByLabelText("名称"));
});
it("输入或同一个编辑对象重渲染不会再次定位；更换对象会定位",()=>{
  const {rerender}=render(<Form/>);frame?.(0);scroll.mockClear();vi.mocked(requestAnimationFrame).mockClear();
  fireEvent.change(screen.getByLabelText("名称"),{target:{value:"正在输入"}});rerender(<Form/>);
  expect(requestAnimationFrame).not.toHaveBeenCalled();expect(scroll).not.toHaveBeenCalled();
  rerender(<Form id="2"/>);expect(requestAnimationFrame).toHaveBeenCalledTimes(1);frame?.(0);expect(scroll).toHaveBeenCalledTimes(1);
});
it("关闭时不定位，重新打开会定位，减少动效时不平滑滚动",()=>{
  const {rerender}=render(<Form open={false}/>);expect(requestAnimationFrame).not.toHaveBeenCalled();
  vi.mocked(matchMedia).mockReturnValue({matches:true} as MediaQueryList);rerender(<Form/>);frame?.(0);
  expect(scroll).toHaveBeenCalledWith({block:"start",behavior:"auto"});
});
it("表单在定位前被关闭时取消待执行的滚动",()=>{
  const {unmount}=render(<Form/>);unmount();expect(cancel).toHaveBeenCalledWith(7);expect(scroll).not.toHaveBeenCalled();
});
