import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import OnboardingGuide, { OnboardingDialog, onboardingApi } from "./OnboardingGuide";

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
  vi.spyOn(onboardingApi, "pending").mockResolvedValue(false);
  vi.spyOn(onboardingApi, "finish").mockResolvedValue();
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it("新用户自动引导，提供商步骤只定位、不提供配置输入", async () => {
  vi.mocked(onboardingApi.pending).mockResolvedValue(true);
  const navigate = vi.fn();
  render(<OnboardingGuide replay={0} onNavigate={navigate}/>);
  await screen.findByRole("dialog");
  fireEvent.click(screen.getByText("下一步"));
  expect(navigate).toHaveBeenLastCalledWith("提供商");
  expect(screen.getByText(/暂不填写或测试连接/)).toBeTruthy();
  expect(screen.queryByRole("textbox")).toBeNull();
  fireEvent.click(screen.getByText("跳过教程"));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(onboardingApi.finish).toHaveBeenCalledTimes(1);
});
it("老用户不自动出现，手动重开仍可从欢迎开始并完成", async () => {
  const navigate=vi.fn();
  const view=render(<OnboardingGuide replay={0} onNavigate={navigate}/>);
  await act(async()=>{});
  expect(screen.queryByRole("dialog")).toBeNull();
  view.rerender(<OnboardingGuide replay={1} onNavigate={navigate}/>);
  await screen.findByRole("dialog");
  for(let i=0;i<3;i++)fireEvent.click(screen.getByText("下一步"));
  fireEvent.click(screen.getByText("上一步"));
  expect(navigate).toHaveBeenLastCalledWith("人设");
  fireEvent.click(screen.getByText("下一步"));
  fireEvent.click(screen.getByText("完成教程"));
  await waitFor(()=>expect(screen.queryByRole("dialog")).toBeNull());
  view.rerender(<OnboardingGuide replay={2} onNavigate={navigate}/>);
  expect(await screen.findByText("初次见面，欢迎来到颜喵")).toBeTruthy();
});
it("保存失败保留教程，Esc可重试跳过", async () => {
  vi.mocked(onboardingApi.finish).mockRejectedValueOnce(new Error("磁盘忙"));
  const close=vi.fn();
  render(<OnboardingDialog onNavigate={()=>{}} onClose={close}/>);
  fireEvent.click(screen.getByText("跳过教程"));
  expect(await screen.findByRole("alert")).toHaveProperty("textContent",expect.stringContaining("磁盘忙"));
  expect(close).not.toHaveBeenCalled();
  fireEvent(screen.getByRole("dialog"),new Event("cancel",{bubbles:true,cancelable:true}));
  await waitFor(()=>expect(close).toHaveBeenCalledTimes(1));
});
