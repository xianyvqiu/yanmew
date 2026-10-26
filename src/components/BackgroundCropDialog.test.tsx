import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import BackgroundCropDialog from "./BackgroundCropDialog";
const draw=vi.fn();
beforeEach(()=>{
 HTMLDialogElement.prototype.showModal=function(){this.setAttribute("open","");};
 vi.spyOn(HTMLElement.prototype,"clientWidth","get").mockReturnValue(640);vi.spyOn(HTMLElement.prototype,"clientHeight","get").mockReturnValue(360);
 vi.spyOn(HTMLCanvasElement.prototype,"getContext").mockReturnValue({drawImage:draw} as unknown as CanvasRenderingContext2D);
 vi.spyOn(HTMLCanvasElement.prototype,"toDataURL").mockReturnValue("data:image/png;base64,Y3JvcA==");
 vi.stubGlobal("ResizeObserver",class {observe(){} disconnect(){}});draw.mockClear();
});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
it("横图缩放后保存正确的16:9裁剪区域，取消不输出",()=>{
 const save=vi.fn(),cancel=vi.fn();render(<BackgroundCropDialog source="data:image/png;base64,aQ==" viewport={{width:1600,height:900}} onSave={save} onCancel={cancel}/>);
 const image=screen.getByAltText("待裁剪背景");Object.defineProperties(image,{naturalWidth:{value:2000},naturalHeight:{value:1000}});fireEvent.load(image);
 fireEvent.change(screen.getByLabelText("背景裁剪缩放"),{target:{value:"2"}});expect(save).not.toHaveBeenCalled();fireEvent.click(screen.getByRole("button",{name:"保存裁剪"}));
 const args=draw.mock.calls[0];expect(args[1]).toBeCloseTo(555.5555,3);expect(args[2]).toBe(250);expect(args[3]).toBeCloseTo(888.8888,3);expect(args[4]).toBe(500);expect(args.slice(5)).toEqual([0,0,1600,900]);expect(save).toHaveBeenCalledWith("Y3JvcA==");
 fireEvent.click(screen.getByRole("button",{name:"取消"}));expect(cancel).toHaveBeenCalledOnce();expect(save).toHaveBeenCalledOnce();
});
it("无法解码的图片不可保存",()=>{const save=vi.fn();render(<BackgroundCropDialog source="bad" onSave={save} onCancel={()=>{}}/>);fireEvent.error(screen.getByAltText("待裁剪背景"));expect((screen.getByRole("button",{name:"保存裁剪"}) as HTMLButtonElement).disabled).toBe(true);expect(screen.getByRole("alert").textContent).toContain("无法解码");expect(save).not.toHaveBeenCalled();});
it("按实际聊天区域比例导出，取景框与输出保留同一构图",()=>{
 vi.spyOn(HTMLElement.prototype,"clientWidth","get").mockReturnValue(600);vi.spyOn(HTMLElement.prototype,"clientHeight","get").mockReturnValue(450);
 const save=vi.fn();render(<BackgroundCropDialog source="data:image/png;base64,aQ==" viewport={{width:1200,height:900}} onSave={save} onCancel={()=>{}}/>);const image=screen.getByAltText("待裁剪背景");Object.defineProperties(image,{naturalWidth:{value:2000},naturalHeight:{value:1000}});fireEvent.load(image);fireEvent.click(screen.getByRole("button",{name:"保存裁剪"}));const args=draw.mock.calls[0];expect(args[1]).toBeCloseTo(333.333,2);expect(args[2]).toBeCloseTo(0);expect(args[3]/args[4]).toBeCloseTo(4/3);expect(args.slice(5)).toEqual([0,0,1600,1200]);
});
