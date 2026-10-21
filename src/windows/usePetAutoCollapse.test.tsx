import{useRef}from"react";
import{act,cleanup,fireEvent,render,screen}from"@testing-library/react";
import{afterEach,beforeEach,expect,it,vi}from"vitest";
import{usePetAutoCollapse}from"./usePetAutoCollapse";
const close=vi.fn();
function Example({draft=false,busy=false,minutes=3}:{draft?:boolean;busy?:boolean;minutes?:number}){const root=useRef<HTMLDivElement>(null);usePetAutoCollapse(root,{minutes,visible:true,hidden:false,busy,hasDraft:draft,onCollapse:close});return <div ref={root} data-testid="pet"/>;}
beforeEach(()=>{close.mockReset();vi.useFakeTimers({toFake:["setInterval","clearInterval","performance"]});});afterEach(()=>{cleanup();vi.useRealTimers();});
it("鼠标离开满三分钟才收起",()=>{render(<Example/>);fireEvent.pointerLeave(screen.getByTestId("pet"));act(()=>vi.advanceTimersByTime(179000));expect(close).not.toHaveBeenCalled();act(()=>vi.advanceTimersByTime(1000));expect(close).toHaveBeenCalledTimes(1);});
it("鼠标留在宠窗不会收起，离开重新计时",()=>{render(<Example/>);fireEvent.pointerEnter(screen.getByTestId("pet"));act(()=>vi.advanceTimersByTime(300000));expect(close).not.toHaveBeenCalled();fireEvent.pointerLeave(screen.getByTestId("pet"));act(()=>vi.advanceTimersByTime(180000));expect(close).toHaveBeenCalledTimes(1);});
it("有草稿或忙碌时保留，结束后重新等三分钟",()=>{const view=render(<Example draft/>);act(()=>vi.advanceTimersByTime(300000));expect(close).not.toHaveBeenCalled();view.rerender(<Example busy/>);act(()=>vi.advanceTimersByTime(300000));expect(close).not.toHaveBeenCalled();view.rerender(<Example/>);act(()=>vi.advanceTimersByTime(180000));expect(close).toHaveBeenCalledTimes(1);});
it("零分钟关闭自动收起",()=>{render(<Example minutes={0}/>);act(()=>vi.advanceTimersByTime(1000000));expect(close).not.toHaveBeenCalled();});
