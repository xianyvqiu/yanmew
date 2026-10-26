import { useEffect, useRef, useState } from "react";
import { errorMessage, reminderApi } from "../../api";
import useReminders from "../../hooks/useReminders";
import type { AppError, Reminder, ReminderInput, ReminderStatus } from "../../types";
import ReminderForm from "./ReminderForm";
import CardBrowserControls from "../CardBrowserControls";
import { useCardBrowser } from "../../hooks/useCardBrowser";
import "./ReminderPanel.css";

const statusLabels: Record<ReminderStatus, string> = { pending: "未完成", done: "完成", expired: "已过期" };
const kindLabels = { once: "一次性", daily: "每天", weekly: "每周", monthly: "每月" };
const displayTime = (value: string | null) => value ? new Date(value).toLocaleString() : "暂无后续安排";
function scheduleText(rule: Reminder) {
  const schedule = rule.schedule;
  if (!schedule) return "一次性";
  const days = schedule.repeat === "weekly" ? schedule.weekdays?.map(day => ["", "周一", "周二", "周三", "周四", "周五", "周六", "周日"][day]).join("、")
    : schedule.repeat === "monthly" ? schedule.days?.map(day => `${day} 日`).join("、") : "";
  return `${kindLabels[rule.kind]} ${days ?? ""} ${schedule.time}`;
}

export default function ReminderPanel() {
  const data = useReminders({ complete: true });
  const [ruleKind, setRuleKind] = useState<Reminder["kind"] | "all">("all");
  const [ruleStatus, setRuleStatus] = useState<ReminderStatus | "all">("all");
  const [recordKind, setRecordKind] = useState<Reminder["kind"] | "all">("all");
  const [recordStatus, setRecordStatus] = useState<ReminderStatus | "all">("all");
  const ruleBrowser = useCardBrowser(data.rules.filter(item => (ruleKind === "all" || item.kind === ruleKind) && (ruleStatus === "all" || item.status === ruleStatus)),
    item => `${item.name} ${item.content}`, item => item.createdAt, item => item.id, 6);
  const kinds = new Map(data.rules.map(item => [item.id, item.kind]));
  const recordBrowser = useCardBrowser(data.occurrences.filter(item => (recordKind === "all" || kinds.get(item.reminderId) === recordKind) && (recordStatus === "all" || item.status === recordStatus)),
    item => `${item.name} ${item.content}`, item => item.scheduledAt, item => item.id, 6);
  useEffect(() => { recordBrowser.setPage(1); }, [data.selectedId, recordBrowser.setPage]);
  const [editing, setEditing] = useState<Reminder | null | undefined>(undefined);
  const [deleting, setDeleting] = useState<Reminder | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [notice, setNotice] = useState("");
  const alive = useRef(true);
  const locked = useRef(false);
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => { if (deleting) dialog.current?.showModal(); else dialog.current?.close(); }, [deleting]);
  const run = async (action: () => Promise<unknown>, message: string) => {
    if (locked.current) return false;
    locked.current = true; setBusy(true); setError(null); setNotice("");
    try {
      await action();
      if (alive.current) { setNotice(message); await data.refresh(); }
      return true;
    } catch (cause) {
      const details = cause && typeof cause === "object" ? cause as Partial<AppError> : {};
      if (alive.current) setError({ code: details.code ?? "UNKNOWN", message: errorMessage(cause), field: details.field });
      return false;
    } finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  const save = async (input: ReminderInput) => {
    const target = editing;
    if (await run(() => target ? reminderApi.update(target.id, input) : reminderApi.create(input), "提醒已保存")) {
      if (alive.current) setEditing(undefined);
    }
  };
  const remove = async () => {
    if (!deleting) return;
    const id = deleting.id;
    if (await run(() => reminderApi.delete(id), "提醒规则及其触发记录已删除")) {
      if (alive.current) { setDeleting(null); if (data.selectedId === id) data.select(""); }
    }
  };
  const acknowledgeMissed = async () => {
    // 固定用户本次看到的 ID；新到记录不能混入确认，分页确认每批不超过 200。
    const ids = data.missed.map(item => item.id);
    await run(async () => {
      for (let start = 0; start < ids.length; start += 200) await reminderApi.acknowledgeMissed(ids.slice(start, start + 200));
    }, "已记录汇总展示，提醒状态未改变");
  };
  const disabled = busy || data.loading;
  const names = [...new Map(data.missed.map(item => [item.reminderId, item.name])).values()];
  return <section className="ym-page-card ym-reminder-panel" aria-label="提醒管理">
    <div className="ym-page-heading"><h2>提醒</h2><button className="ym-button" disabled={disabled || editing !== undefined} onClick={() => { setEditing(null); setError(null); }}>新增提醒</button></div>
    <p className="ym-page-note">提醒属于全局。循环时间跟随电脑当前时区；不存在的日期或时刻会跳过，重复时刻只提醒一次。</p>
    <p className="ym-page-note">在线提醒到点自动完成本次触发；宠窗“知道了”仅收起提示，循环规则继续生效。永久停止需删除规则。</p>
    {data.error && <div className="ym-page-error" role="alert">{data.error}<button className="ym-button" disabled={disabled} onClick={() => { void data.refresh(); }}>重新读取</button></div>}
    {error && !deleting && (!error.field || editing === undefined) && <p role="alert" className="ym-page-error">{error.message}</p>}
    {notice && <p role="status">{notice}</p>}
    {data.loading && <p role="status">正在读取提醒…</p>}
    {data.missed.length > 0 && <aside className="ym-reminder-summary" aria-label="离线过期汇总">
      <h3>离开期间的提醒</h3><p>你离开这段时间，{names.join("、")}过期了！</p>
      <p className="ym-page-note">下方触发记录保留处理状态，确认看过不会将提醒改为完成。</p>
      <button className="ym-button" disabled={disabled} onClick={() => { void acknowledgeMissed(); }}>知道了</button>
    </aside>}
    {editing !== undefined && <ReminderForm key={editing?.id ?? "new"} reminder={editing} busy={busy} error={error} onSave={save} onCancel={() => { setEditing(undefined); setError(null); }} />}
    <section className="ym-reminder-section" aria-label="提醒规则列表"><h3>提醒规则</h3>
    <div className="ym-reminder-filters">
      <label className="ym-field">提醒类型<select value={ruleKind} disabled={disabled} onChange={event => { setRuleKind(event.target.value as typeof ruleKind); ruleBrowser.setPage(1); }}><option value="all">全部类型</option>{Object.entries(kindLabels).map(([value,label]) => <option key={value} value={value}>{label}</option>)}</select></label>
      <label className="ym-field">规则状态<select value={ruleStatus} disabled={disabled} onChange={event => { setRuleStatus(event.target.value as typeof ruleStatus); ruleBrowser.setPage(1); }}><option value="all">全部状态</option>{Object.entries(statusLabels).map(([value,label]) => <option key={value} value={value}>{label}</option>)}</select></label>
      <button className="ym-button" disabled={disabled || (ruleKind === "all" && ruleStatus === "all" && !ruleBrowser.search)} onClick={() => { setRuleKind("all"); setRuleStatus("all"); ruleBrowser.setSearch(""); }}>清除规则筛选</button>
    </div>
    <CardBrowserControls browser={{...ruleBrowser, loading:data.loading, error:data.error}} label="提醒规则" timeLabel="创建时间" disabled={disabled}/>
    {!data.loading && !data.error && data.rules.length > 0 && ruleBrowser.total === 0 && !ruleBrowser.search && <p className="ym-page-empty">没有符合筛选条件的提醒规则。</p>}
    {!data.loading && data.rules.length === 0 && <p className="ym-page-note">暂无提醒，可以新增一次性或循环提醒。</p>}
    <div className="ym-reminder-rules">{ruleBrowser.items.map(rule => <article className="ym-reminder-rule" key={rule.id} aria-label={rule.name}>
      <div className="ym-page-heading"><h4>{rule.name}</h4><span className="ym-reminder-state" data-state={rule.status}>{rule.kind!=="once"?"循环中":statusLabels[rule.status]}</span></div>
      <p className="ym-reminder-content">{rule.content || "未填写详情"}</p>
      <p>{scheduleText(rule)}</p><p className="ym-page-note">下一次：{displayTime(rule.fireAt)} · 来源：{rule.source === "chat" ? "聊天" : "手动创建"}</p>
      <div className="ym-inline-form">
        <button className="ym-button" disabled={disabled || editing !== undefined} aria-label={`编辑 ${rule.name}`} onClick={() => { setEditing(rule); setError(null); }}>编辑</button>
        <button className="ym-button" disabled={disabled} aria-label={`查看 ${rule.name} 触发记录`} onClick={() => data.select(rule.id)}>触发记录</button>
        <button className="ym-button ym-reminder-danger" disabled={disabled || editing !== undefined} aria-label={`删除 ${rule.name}`} onClick={() => { setDeleting(rule); setError(null); }}>删除</button>
      </div>
    </article>)}</div>
    </section>
    <section className="ym-reminder-section" aria-label="提醒触发记录列表"><div className="ym-page-heading"><h3>触发记录</h3>{data.selectedId && <button className="ym-button" disabled={disabled} onClick={() => data.select("")}>查看全部触发记录</button>}</div>
    {data.selectedId && <p className="ym-page-note">当前筛选：{data.rules.find(rule => rule.id === data.selectedId)?.name ?? "所选提醒"}</p>}
    <div className="ym-reminder-filters">
      <label className="ym-field">记录类型<select value={recordKind} disabled={disabled} onChange={event => { setRecordKind(event.target.value as typeof recordKind); recordBrowser.setPage(1); }}><option value="all">全部类型</option>{Object.entries(kindLabels).map(([value,label]) => <option key={value} value={value}>{label}</option>)}</select></label>
      <label className="ym-field">记录状态<select value={recordStatus} disabled={disabled} onChange={event => { setRecordStatus(event.target.value as typeof recordStatus); recordBrowser.setPage(1); }}><option value="all">全部状态</option>{Object.entries(statusLabels).map(([value,label]) => <option key={value} value={value}>{label}</option>)}</select></label>
      <button className="ym-button" disabled={disabled || (recordKind === "all" && recordStatus === "all" && !recordBrowser.search)} onClick={() => { setRecordKind("all"); setRecordStatus("all"); recordBrowser.setSearch(""); }}>清除记录筛选</button>
    </div>
    <CardBrowserControls browser={{...recordBrowser, loading:data.loading, error:data.error}} label="触发记录" timeLabel="原定时间" disabled={disabled}/>
    {!data.loading && !data.error && data.occurrences.length > 0 && recordBrowser.total === 0 && !recordBrowser.search && <p className="ym-page-empty">没有符合筛选条件的触发记录。</p>}
    {!data.loading && data.occurrences.length === 0 && <p className="ym-page-note">暂无触发记录。</p>}
    <div className="ym-reminder-occurrences">{recordBrowser.items.map(item => <article className="ym-reminder-occurrence" key={item.id} aria-label={`${item.name} 本次提醒`}>
      <h4>{item.name}</h4><p className="ym-reminder-content">{item.content || "未填写详情"}</p>
      <p>本次原定时间：{displayTime(item.scheduledAt)}</p>
      <p className="ym-page-note ym-reminder-state" data-state={item.deliveryState}>{item.deliveryState === "missed" ? "离线错过，汇总待确认" : item.deliveryState === "queued" ? "待确认已看到" : "已确认展示"}</p>
      <div className="ym-inline-form"><label>本次状态<select aria-label={`${item.name} ${item.id} 本次状态`} value={item.status} className="ym-reminder-state" data-state={item.status} disabled={disabled}
        onChange={event => { const value = event.target.value as ReminderStatus; void run(() => reminderApi.updateStatus(item.id, value), "本次提醒状态已保存"); }}>
        {Object.entries(statusLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
      </select></label>
      {item.status !== "done" && <button className="ym-button" disabled={disabled} aria-label={`完成本次 ${item.id}`} onClick={() => { void run(() => reminderApi.complete(item.id), "已完成本次提醒，循环规则继续生效"); }}>完成本次</button>}
      {item.deliveryState === "queued" && <button className="ym-button" disabled={disabled} onClick={() => { void run(() => reminderApi.acknowledge([item.id], "queued"), "已确认看到本次提醒，完成状态未改变"); }}>确认已看到</button>}
      </div>
    </article>)}</div>
    </section>
    <dialog ref={dialog} className="ym-reminder-dialog" aria-labelledby="reminder-delete-title" onCancel={event => { if (busy) event.preventDefault(); else setDeleting(null); }}>
      {deleting && <><h2 id="reminder-delete-title">删除 {deleting.name}</h2><p>这会永久停止此提醒，并删除它的全部触发记录。其他提醒不受影响。</p>
        {error && <p className="ym-page-error" role="alert">{error.message}</p>}
        <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => setDeleting(null)}>取消</button><button className="ym-button" disabled={busy} onClick={() => { void remove(); }}>{busy ? "正在删除…" : "确认删除"}</button></div></>}
    </dialog>
  </section>;
}
