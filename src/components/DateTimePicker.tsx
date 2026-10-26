import { createPortal } from "react-dom";
import { useRef, useState, type ChangeEvent, type InputHTMLAttributes } from "react";

const pad = (value: number) => String(value).padStart(2, "0");
const dayKey = (date: Date) => `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
export default function DateTimePicker(props: InputHTMLAttributes<HTMLInputElement>) {
  const input = useRef<HTMLInputElement>(null), dialog = useRef<HTMLDialogElement>(null);
  const [chosen, setChosen] = useState(new Date()), [month, setMonth] = useState(new Date());
  const [hour, setHour] = useState("12"), [minute, setMinute] = useState("00");
  const timeOnly = props.type === "time";
  const open = () => {
    const value = String(props.value ?? "");
    const parsed = new Date(timeOnly ? `${dayKey(new Date())}T${value || "12:00"}` : value);
    const date = Number.isNaN(parsed.getTime()) ? new Date() : parsed;
    setChosen(date); setMonth(new Date(date.getFullYear(), date.getMonth(), 1)); setHour(pad(date.getHours())); setMinute(pad(date.getMinutes()));
    if(dialog.current?.showModal)dialog.current.showModal();else dialog.current?.setAttribute("open","");
  };
  const start = new Date(month.getFullYear(), month.getMonth(), 1);
  start.setDate(1 - (start.getDay() + 6) % 7);
  return <div className="ym-date-picker">
    <div className="ym-date-picker-field"><input {...props} ref={input} onClick={event => { if (!props.disabled) { event.preventDefault(); open(); } }} />
      <button type="button" className="ym-button" aria-label={`选择${timeOnly ? "循环时间" : "提醒时间"}`} disabled={props.disabled} onClick={open}><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg></button></div>
    {createPortal(<dialog ref={dialog} className="ym-session-dialog ym-date-dialog" aria-label={timeOnly ? "选择循环时间" : "选择提醒日期与时间"}>
      <h2>{timeOnly ? "选择时间" : "选择日期与时间"}</h2>
      {!timeOnly && <><div className="ym-page-heading"><button type="button" className="ym-button" aria-label="上个月" onClick={() => setMonth(new Date(month.getFullYear(), month.getMonth() - 1, 1))}>上一月</button><strong>{month.getFullYear()}年{month.getMonth() + 1}月</strong><button type="button" className="ym-button" aria-label="下个月" onClick={() => setMonth(new Date(month.getFullYear(), month.getMonth() + 1, 1))}>下一月</button></div>
      <div className="ym-date-grid">{"一二三四五六日".split("").map(label => <span key={label}>{label}</span>)}{Array.from({ length: 42 }, (_, index) => {
        const date = new Date(start); date.setDate(start.getDate() + index);
        return <button type="button" key={dayKey(date)} className="ym-button" data-selected={dayKey(date) === dayKey(chosen)} data-muted={date.getMonth() !== month.getMonth()} aria-label={dayKey(date)} aria-pressed={dayKey(date) === dayKey(chosen)} onClick={() => setChosen(date)}>{date.getDate()}</button>;
      })}</div></>}
      <div className="ym-time-fields"><label className="ym-field">小时<select aria-label="小时" value={hour} onChange={event => setHour(event.target.value)}>{Array.from({ length: 24 }, (_, index) => <option key={index}>{pad(index)}</option>)}</select></label><span>:</span><label className="ym-field">分钟<select aria-label="分钟" value={minute} onChange={event => setMinute(event.target.value)}>{Array.from({ length: 60 }, (_, index) => <option key={index}>{pad(index)}</option>)}</select></label></div>
      <div className="ym-inline-form"><button type="button" className="ym-button" onClick={() => dialog.current?.close()}>取消</button><button type="button" className="ym-button" data-variant="primary" onClick={() => {
        if (input.current) { input.current.value = `${timeOnly ? "" : dayKey(chosen) + "T"}${hour}:${minute}`; props.onChange?.({ target: input.current, currentTarget: input.current } as ChangeEvent<HTMLInputElement>); }
        dialog.current?.close();
      }}>确定时间</button></div>
    </dialog>,document.body)}
  </div>;
}
