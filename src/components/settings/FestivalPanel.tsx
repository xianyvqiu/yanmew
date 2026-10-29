import CardBrowserControls, {CardBrowserPagination} from "../CardBrowserControls";
import {useRemoteCardBrowser} from "../../hooks/useRemoteCardBrowser";
import { useEffect, useRef, useState } from "react";
import { errorMessage, festivalsApi } from "../../api";
import type { CurrentContext, CustomFestivalInput, CustomFestivalRecord } from "../../types";
import "./FestivalPanel.css";
import CalendarView from "./RealMoneyCalendar";
import "./RealMoneyPanel.css";

const kinds = { birthday: "生日", anniversary: "纪念日", festival: "固定节日", custom: "自定义日期" };
const emptyDraft = (): CustomFestivalInput => ({ kind: "birthday", name: "", calendarType: "solar", month: 1, day: 1, isLeapMonth: false, blessing: "", note: null });
function draftOf(record: CustomFestivalRecord): CustomFestivalInput {
  return { kind: record.kind, name: record.name, calendarType: record.calendarType,
    month: (record.calendarType === "solar" ? record.solarMonth : record.lunarMonth) ?? 1,
    day: (record.calendarType === "solar" ? record.solarDay : record.lunarDay) ?? 1,
    isLeapMonth: record.isLeapMonth, blessing: record.blessing, note: record.note };
}
function definition(record: CustomFestivalRecord) {
  if (record.calendarType === "lunar") return `农历 ${record.isLeapMonth ? "闰" : ""}${record.lunarMonth ?? "—"} 月 ${record.lunarDay ?? "—"} 日`;
  return `公历 ${record.solarMonth ?? "—"} 月 ${record.solarDay ?? "—"} 日`;
}

/** 全局日期切换会话仍保留；普通刷新不覆盖编辑快照和草稿。 */
export default function FestivalPanel({ context }: { context: CurrentContext }) {
  const browser=useRemoteCardBrowser<CustomFestivalRecord>("dates",context);
  const [dateSelection,setDateSelection]=useState<string|null>(null);
  const [calendarRevision,setCalendarRevision]=useState(0);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [draft, setDraft] = useState<CustomFestivalInput | null>(null);
  const [editing, setEditing] = useState<CustomFestivalRecord | null>(null);
  const [removing, setRemoving] = useState<CustomFestivalRecord | null>(null);
  const alive = useRef(false);
  const locked = useRef(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const formRef = useRef<HTMLFormElement>(null);
  useEffect(() => { if (draft) formRef.current?.scrollIntoView?.({behavior:window.matchMedia?.("(prefers-reduced-motion: reduce)").matches?"auto":"smooth",block:"start"}); }, [!!draft]);
  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);
  useEffect(() => { if (removing) dialog.current?.showModal(); else dialog.current?.close(); }, [removing]);

  const act = async (operation: () => Promise<void>) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError(""); setNotice("");
    try { await operation(); }
    catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally {
      locked.current = false;
      if (alive.current) {
        setBusy(false);
      }
    }
  };
  const edit = (id: string) => { void act(async () => {
    const current = await festivalsApi.get(id);
    if (alive.current) { setEditing(current); setDraft(draftOf(current)); }
  }); };
  const save = () => { if (!draft) return; const input = draft; const original = editing;
    void act(async () => {
      if (original) await festivalsApi.update(original.id, original.revisionToken, input);
      else await festivalsApi.create(input);
      if (alive.current) { setDraft(null); setEditing(null); setNotice("日期已保存。"); browser.refresh(); setCalendarRevision(value=>value+1); }
    });
  };

  return <div className="ym-festival-panel">
    <CalendarView canAdd={!busy&&!draft} revision={String(calendarRevision)} onAdd={date=>{
      if(busy||draft)return;
      const [,month,day]=date.split("-").map(Number); setEditing(null);setDraft({...emptyDraft(),calendarType:"solar",month,day});setError("");
    }} onEdit={id=>{if(!busy&&!draft)edit(id);}}/>
    {error&&!draft&&<p role="alert" className="ym-page-error">{error}</p>}
    {notice&&<p role="status">{notice}</p>}
    <details className="ym-page-card" open={draft!==null||undefined}><summary>管理自定义日期</summary><section aria-label="自定义日期">
      <div className="ym-page-heading"><div><h2>自定义日期</h2><p className="ym-page-note">所有人设和会话共用，用户与 AI 都可以维护。生日、纪念日按固定月日重复，不填写出生年份；农历同年出现两次均生效，不存在的日期跳过。</p></div>
        <button className="ym-button" disabled={busy || draft !== null} onClick={() => { setEditing(null); setDraft(emptyDraft()); setError(""); }}>新增日期</button></div>
      <>
        {!browser.loading && !browser.total && !browser.search && !browser.error && <p className="ym-page-empty">还没有自定义日期。</p>}
        <CardBrowserControls browser={browser} label="自定义日期" timeLabel="创建时间" disabled={busy||draft!==null}/>
        {browser.error&&<p role="alert" className="ym-page-error">{browser.error}</p>}
        <ul className="ym-festival-list ym-object-grid">{browser.items.map(record => <li key={record.id}>
          <div className="ym-page-heading"><h3><button className="ym-card-open" disabled={busy||draft!==null} onClick={()=>setDateSelection(dateSelection===record.id?null:record.id)}>{record.name}</button></h3><span className="ym-page-note">{kinds[record.kind]} · {definition(record)}</span></div>
          <p className="ym-page-note">{record.year} 年：{record.occurrences.length ? record.occurrences.join("、") : "本年无对应日期"} · {record.source === "ai" ? "AI记录" : "手动记录"}</p>
          {dateSelection===record.id&&<>
          <p className="ym-festival-text">{record.blessing}</p>{record.note && <p className="ym-page-note">{record.note}</p>}
          <div className="ym-inline-form"><button className="ym-button" disabled={busy || draft !== null} onClick={() => edit(record.id)}>编辑 {record.name}</button>
            <button className="ym-button" data-variant="danger" disabled={busy || draft !== null} onClick={() => { setRemoving(record); setError(""); }}>删除 {record.name}</button></div>
          </>}
        </li>)}</ul>
      <CardBrowserPagination browser={browser} label="自定义日期" disabled={busy||draft!==null}/>

      </>
      {draft && <form ref={formRef} className="ym-festival-form" aria-label={editing ? "编辑日期" : "新增日期"} onSubmit={event => { event.preventDefault(); save(); }}>
        <h3>{editing ? `编辑：${editing.name}` : "新增固定日期"}</h3>
        {error && <p role="alert" className="ym-page-error">{error}</p>}
        <fieldset disabled={busy}>
          <div className="ym-festival-fields"><label className="ym-field">名称<input required value={draft.name} onChange={event => setDraft({ ...draft, name: event.target.value })} /></label>
            <label className="ym-field">类型<select value={draft.kind} onChange={event => setDraft({ ...draft, kind: event.target.value as CustomFestivalInput["kind"] })}>{Object.entries(kinds).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label>
            <label className="ym-field">历法<select value={draft.calendarType} onChange={event => setDraft({ ...draft, calendarType: event.target.value as "solar" | "lunar", isLeapMonth: false })}><option value="solar">公历</option><option value="lunar">农历</option></select></label>
            <label className="ym-field">月份<input required type="number" min={1} max={12} step={1} value={draft.month} onChange={event => setDraft({ ...draft, month: Number(event.target.value) })} /></label>
            <label className="ym-field">日期<input required type="number" min={1} max={draft.calendarType === "lunar" ? 30 : 31} step={1} value={draft.day} onChange={event => setDraft({ ...draft, day: Number(event.target.value) })} /></label>
          </div>
          {draft.calendarType === "lunar" && <label className="ym-festival-check"><input type="checkbox" checked={draft.isLeapMonth} onChange={event => setDraft({ ...draft, isLeapMonth: event.target.checked })} />闰月（仅在该年有对应闰月时生效）</label>}
          <label className="ym-field">祝福词<textarea required rows={3} value={draft.blessing} onChange={event => setDraft({ ...draft, blessing: event.target.value })} /></label>
          <label className="ym-field">备注<textarea rows={2} value={draft.note ?? ""} onChange={event => setDraft({ ...draft, note: event.target.value || null })} /></label>
          <div className="ym-inline-form"><button className="ym-button" type="submit">{busy ? "正在保存…" : "保存日期"}</button>
            <button className="ym-button" type="button" onClick={() => { setDraft(null); setEditing(null); setError(""); }}>取消编辑</button>
            {editing && <button className="ym-button" type="button" onClick={() => edit(editing.id)}>放弃草稿并重新读取</button>}</div>
        </fieldset>
        <p className="ym-page-note">普通刷新保留草稿。如果其他窗口或AI修改了同一记录，保存会提示冲突；可复制草稿后重新读取。固定日期不替代到点提醒。</p>
      </form>}
    </section></details>
    <dialog ref={dialog} className="ym-session-dialog" aria-labelledby="festival-delete-title" onCancel={event => { if (busy) event.preventDefault(); else setRemoving(null); }}>
      <h2 id="festival-delete-title">删除这条固定日期？</h2><div className="ym-session-dialog-body"><p>{removing?.name}删除后不可恢复，不影响聊天、记忆或已领取的签到奖励。</p>{error && <p className="ym-page-error">{error}</p>}</div>
      <div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={() => setRemoving(null)}>取消删除</button>
        <button className="ym-button" data-variant="danger" disabled={busy || !removing} onClick={() => { if (!removing) return; const target = removing; void act(async () => {
          await festivalsApi.delete(target.id, target.revisionToken);
          if (alive.current) { setRemoving(null); setNotice("日期已删除。"); browser.refresh(); setCalendarRevision(value=>value+1); }
        }); }}>{busy ? "正在删除…" : "确认删除"}</button></div>
    </dialog>
  </div>;
}
