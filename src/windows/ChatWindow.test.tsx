import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import ChatWindow from "./ChatWindow";
import type { WorkspaceContext } from "../hooks/useCurrentContext";
const nav=vi.hoisted(() => ({openSettingsDetail:vi.fn().mockResolvedValue(undefined)}));
const current:WorkspaceContext={
  context:{personaId:"1",sessionId:null,contextRevision:1},sessions:[],userProfiles:[],
  persona:{id:"1",name:"电子小猫",prompt:"设定",initialPrompt:"设定",selfEvolveEnabled:false,lastEvolvedAt:null,color:"#123456",chatBgFile:null,chatBgOpacity:85,chatBgBlur:0,isActive:true,isBuiltin:true,expressions:null},
};
vi.mock("../hooks/useCurrentContext",()=>({useCurrentContext:()=>({snapshot:current,loading:false,error:"",refresh:vi.fn()})}));
vi.mock("./usePetAppearance",()=>({usePetAppearance:()=>({ready:true,error:""})}));
vi.mock("../platform/navigation",()=>({navigation:nav}));
vi.mock("../api",()=>({errorMessage:(cause:Error)=>cause.message}));
vi.mock("../pages/ChatPage",()=>({default:()=>null}));
vi.mock("../components/ChatBackground",()=>({default:()=>null}));
afterEach(cleanup);
it("点击当前人设请求设置中的对应详情，失败可见且不更改会话",async()=>{
  nav.openSettingsDetail.mockRejectedValueOnce(new Error("窗口打开失败"));
  render(<ChatWindow />);
  fireEvent.click(screen.getByRole("button",{name:"打开人设详情：电子小猫"}));
  expect(nav.openSettingsDetail).toHaveBeenCalledWith("persona","1");
  await waitFor(()=>expect(screen.getByRole("alert").textContent).toContain("窗口打开失败"));
});
