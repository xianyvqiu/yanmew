import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { errorMessage } from "../../api";
import "./OnboardingGuide.css";

export const onboardingApi = {
  pending: () => invoke<boolean>("get_onboarding_pending"),
  finish: () => invoke<void>("finish_onboarding"),
};
const steps = [
  { title: "初次见面，欢迎来到颜喵", page: "帮助与使用手册", location: "先认识你的桌面伙伴", face: "(・ω・)ノ", text: "聊天、陪伴、出行和小礼物，都可以从桌面上的伙伴开始。接下来用四个小步骤认识常用入口。", note: "教程不会调用模型，也不会替你修改配置。随时可以跳过，以后在使用手册重新查看。" },
  { title: "连接模型，从这里开始", page: "提供商", location: "全局设置 → 基础服务 → 提供商", face: "( •̀ ω •́ )✧", text: "这里管理模型服务地址、模型名称和 API Key。聊天与生图可以分别选择服务，使用时可能产生服务商费用。", note: "已带你定位提供商页面。本步骤只介绍入口，暂不填写或测试连接；完成教程后再自行配置。" },
  { title: "选择你喜欢的伙伴", page: "人设", location: "全局设置 → 基础服务 → 人设", face: "(｡･ω･｡)", text: "这里可以选择内置人设，也可以创建自己的角色。人设名称不限定聊天模型；角色外观和表情库由同一人设的会话共用。", note: "教程中先认识入口。结束后再选择伙伴，从宠窗打开聊天详情开始对话。" },
  { title: "设置与帮助，随时找得到", page: "帮助与使用手册", location: "设置侧栏底部 → 帮助与使用手册", face: "(๑•̀ㅂ•́)و✧", text: "全局设置管理所有伙伴共用的服务；人设与会话设置管理当前伙伴的形象、状态和经历。每个页面顶部都会说明资料归属。", note: "更详细的说明就在使用手册中。完成或跳过后，以后启动不会再自动显示本教程。" },
];

export function OnboardingDialog({ onNavigate, onClose }: { onNavigate: (page: string) => void; onClose: () => void }) {
  const [step, setStep] = useState(0), [busy, setBusy] = useState(false), [error, setError] = useState("");
  const dialog = useRef<HTMLDialogElement>(null), heading = useRef<HTMLHeadingElement>(null);
  const saving = useRef(false), navigate = useRef(onNavigate); navigate.current = onNavigate;
  useEffect(() => { dialog.current?.showModal(); return () => dialog.current?.close(); }, []);
  useEffect(() => { navigate.current(steps[step].page); heading.current?.focus(); }, [step]);
  const finish = async () => {
    if (saving.current) return;
    saving.current = true; setBusy(true); setError("");
    try { await onboardingApi.finish(); onClose(); }
    catch (cause) { setError(`暂时未能保存教程状态：${errorMessage(cause)}。请重试。`); }
    finally { saving.current = false; setBusy(false); }
  };
  const item = steps[step];
  return createPortal(<dialog ref={dialog} className="ym-onboarding" aria-labelledby="onboarding-heading" onCancel={event => { event.preventDefault(); void finish(); }}>
    <header><span>新手教程 · {step + 1} / {steps.length}</span><button className="ym-button" disabled={busy} onClick={() => void finish()}>跳过教程</button></header>
    <div className="ym-onboarding-progress" aria-hidden="true">{steps.map((s, i) => <span key={s.title} data-active={i <= step} />)}</div>
    <div key={step} className="ym-onboarding-body">
      <div className="ym-onboarding-face" aria-hidden="true">{item.face}</div>
      <h2 ref={heading} tabIndex={-1} id="onboarding-heading">{item.title}</h2>
      <p className="ym-onboarding-location">{item.location}</p><p>{item.text}</p>
      <aside>{item.note}</aside>
    </div>
    {error && <p role="alert" className="ym-page-error">{error}</p>}
    <footer><button className="ym-button" disabled={busy || step === 0} onClick={() => setStep(s => s - 1)}>上一步</button>
      <button className="ym-button" disabled={busy} onClick={() => step === steps.length - 1 ? void finish() : setStep(s => s + 1)}>{busy ? "正在保存…" : step === steps.length - 1 ? "完成教程" : "下一步"}</button></footer>
  </dialog>, document.body);
}

export default function OnboardingGuide({ replay, onNavigate }: { replay: number; onNavigate: (page: string) => void }) {
  const [open, setOpen] = useState(false);
  const manuallyOpened = useRef(false);
  useEffect(() => {
    let disposed = false;
    void onboardingApi.pending().then(pending => { if (!disposed && !manuallyOpened.current) setOpen(pending); }).catch(() => {});
    return () => { disposed = true; };
  }, []);
  useEffect(() => { if (replay > 0) { manuallyOpened.current = true; setOpen(true); } }, [replay]);
  return open ? <OnboardingDialog onNavigate={onNavigate} onClose={() => setOpen(false)} /> : null;
}
