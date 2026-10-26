import {useRevealForm} from "../../hooks/useRevealForm";
import DateTimePicker from "../DateTimePicker";
import { useId, useState, type FormEvent } from "react";
import type { AppError, Reminder, ReminderInput, ReminderSchedule } from "../../types";

type ReminderFormProps = {
  reminder: Reminder | null;
  busy: boolean;
  error: AppError | null;
  onSave: (input: ReminderInput) => Promise<void>;
  onCancel: () => void;
};

type ReminderKind = Reminder["kind"];
type FormField = "name" | "content" | "fireAt" | "schedule";

function localDateTimeValue(value: string | null | undefined): string {
  if (!value) return "";
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return "";
  const pad = (part: number) => String(part).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function sameNumbers(left: number[], right: number[] | undefined): boolean {
  return Boolean(right) && left.length === right!.length && left.every(value => right!.includes(value));
}

export function parseLocalDateTime(value: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/.exec(value);
  if (!match) return null;
  const [, yearText, monthText, dayText, hourText, minuteText] = match;
  const year = Number(yearText);
  const month = Number(monthText);
  const day = Number(dayText);
  const hour = Number(hourText);
  const minute = Number(minuteText);
  const date = new Date(year, month - 1, day, hour, minute, 0, 0);
  if (!Number.isFinite(date.getTime())
    || date.getFullYear() !== year
    || date.getMonth() !== month - 1
    || date.getDate() !== day
    || date.getHours() !== hour
    || date.getMinutes() !== minute) return null;
  return date;
}

export default function ReminderForm({ reminder, busy, error, onSave, onCancel }: ReminderFormProps) {
  const id = useId();
  const form=useRevealForm();
  const originalDateInput = localDateTimeValue(reminder?.fireAt);
  const [name, setName] = useState(reminder?.name ?? "");
  const [content, setContent] = useState(reminder?.content ?? "");
  const [kind, setKind] = useState<ReminderKind>(reminder?.kind ?? "once");
  const [fireAt, setFireAt] = useState(originalDateInput);
  const [time, setTime] = useState(reminder?.schedule?.time ?? "09:00");
  const [weekdays, setWeekdays] = useState<number[]>(reminder?.schedule?.weekdays ?? []);
  const [days, setDays] = useState<number[]>(reminder?.schedule?.days ?? []);
  const [validationErrors, setValidationErrors] = useState<Partial<Record<FormField, string>>>({});

  const errorFor = (field: FormField) => validationErrors[field] ?? (error?.field === field ? error.message : undefined);
  const nameError = errorFor("name");
  const contentError = errorFor("content");
  const fireAtError = errorFor("fireAt");
  const scheduleError = errorFor("schedule");

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (busy) return;

    const nextErrors: Partial<Record<FormField, string>> = {};
    if (!name.trim()) nextErrors.name = "请输入提醒名称";
    let input: ReminderInput | null = null;

    if (kind === "once") {
      const mayKeepNull = reminder?.kind === "once" && reminder.fireAt === null && fireAt === originalDateInput;
      if (!fireAt && mayKeepNull) {
        input = { name: name.trim(), content, schedule: null, fireAt: null };
      } else if (!fireAt) {
        nextErrors.fireAt = "请选择未来的提醒时间";
      } else if (fireAt === originalDateInput && reminder?.kind === "once" && reminder.fireAt) {
        input = { name: name.trim(), content, schedule: null, fireAt: reminder.fireAt };
      } else {
        const date = parseLocalDateTime(fireAt);
        if (!date) nextErrors.fireAt = "请输入有效且当地存在的提醒时间";
        else if (date.getTime() <= Date.now()) nextErrors.fireAt = "请选择未来的提醒时间";
        else input = { name: name.trim(), content, schedule: null, fireAt: date.toISOString() };
      }
    } else {
      const validTime = /^(?:[01]\d|2[0-3]):[0-5]\d$/.test(time);
      if (!validTime) nextErrors.schedule = "循环时间须为 HH:MM";
      else if (kind === "weekly" && weekdays.length === 0) nextErrors.schedule = "请至少选择一个星期几";
      else if (kind === "monthly" && days.length === 0) nextErrors.schedule = "请至少选择一个日期";
      else {
        const schedule: ReminderSchedule = { version: 1, repeat: kind, time };
        if (kind === "weekly") {
          schedule.weekdays = sameNumbers(weekdays, reminder?.schedule?.weekdays)
            ? [...reminder!.schedule!.weekdays!]
            : [...weekdays];
        } else if (kind === "monthly") {
          schedule.days = sameNumbers(days, reminder?.schedule?.days)
            ? [...reminder!.schedule!.days!]
            : [...days];
        }
        input = { name: name.trim(), content, schedule, fireAt: null };
      }
    }

    setValidationErrors(nextErrors);
    if (Object.keys(nextErrors).length > 0 || !input) return;
    await onSave(input);
  };

  const changeKind = (value: ReminderKind) => {
    setKind(value);
    setValidationErrors(current => ({ ...current, fireAt: undefined, schedule: undefined }));
  };
  const toggleNumber = (values: number[], setValues: (value: number[]) => void, value: number) => {
    setValues(values.includes(value) ? values.filter(item => item !== value) : [...values, value]);
    setValidationErrors(current => ({ ...current, schedule: undefined }));
  };

  return <form ref={form} className="ym-reminder-form" onSubmit={submit}>
    <fieldset disabled={busy}>
      <legend>{reminder ? "编辑提醒" : "新增提醒"}</legend>

      <label className="ym-field" htmlFor={`${id}-name`}>
        提醒名称
        <input id={`${id}-name`} aria-label="提醒名称" value={name} required
          aria-invalid={Boolean(nameError)} aria-describedby={nameError ? `${id}-name-error` : undefined}
          onChange={event => { setName(event.target.value); setValidationErrors(current => ({ ...current, name: undefined })); }} />
        {nameError && <span id={`${id}-name-error`} role="alert">{nameError}</span>}
      </label>

      <label className="ym-field" htmlFor={`${id}-content`}>
        提醒详情
        <textarea id={`${id}-content`} aria-label="提醒详情" value={content}
          aria-invalid={Boolean(contentError)} aria-describedby={contentError ? `${id}-content-error` : undefined}
          onChange={event => { setContent(event.target.value); setValidationErrors(current => ({ ...current, content: undefined })); }} />
        {contentError && <span id={`${id}-content-error`} role="alert">{contentError}</span>}
      </label>

      <label className="ym-field" htmlFor={`${id}-kind`}>
        提醒类型
        <select id={`${id}-kind`} aria-label="提醒类型" value={kind} onChange={event => changeKind(event.target.value as ReminderKind)}>
          <option value="once">一次性</option>
          <option value="daily">每天</option>
          <option value="weekly">每周</option>
          <option value="monthly">每月</option>
        </select>
      </label>

      {kind === "once" ? <label className="ym-field" htmlFor={`${id}-fire-at`}>
        提醒时间
        <DateTimePicker id={`${id}-fire-at`} aria-label="提醒时间" type="datetime-local" value={fireAt}
          aria-invalid={Boolean(fireAtError)} aria-describedby={fireAtError ? `${id}-fire-at-error` : reminder?.kind === "once" && reminder.fireAt === null ? `${id}-fire-at-help` : undefined}
          onChange={event => { setFireAt(event.target.value); setValidationErrors(current => ({ ...current, fireAt: undefined })); }} />
        {reminder?.kind === "once" && reminder.fireAt === null && <span id={`${id}-fire-at-help`} className="ym-page-note">此提醒已触发；留空可只修改文字，填写未来时间可重新安排。</span>}
        {fireAtError && <span id={`${id}-fire-at-error`} role="alert">{fireAtError}</span>}
      </label> : <div className="ym-field">
        <label htmlFor={`${id}-time`}>循环时间</label>
        <DateTimePicker id={`${id}-time`} aria-label="循环时间" type="time" value={time}
          aria-invalid={Boolean(scheduleError)} aria-describedby={scheduleError ? `${id}-schedule-error` : undefined}
          onChange={event => { setTime(event.target.value); setValidationErrors(current => ({ ...current, schedule: undefined })); }} />
        {kind === "weekly" && <fieldset className="ym-reminder-form__choices" aria-label="每周日期">
          <legend>重复星期</legend>
          {["周一", "周二", "周三", "周四", "周五", "周六", "周日"].map((label, index) => {
            const value = index + 1;
            return <label key={value}><input type="checkbox" checked={weekdays.includes(value)} aria-label={label}
              onChange={() => toggleNumber(weekdays, setWeekdays, value)} />{label}</label>;
          })}
        </fieldset>}
        {kind === "monthly" && <fieldset className="ym-reminder-form__choices" aria-label="每月日期">
          <legend>重复日期</legend>
          {Array.from({ length: 31 }, (_, index) => index + 1).map(value =>
            <label key={value}><input type="checkbox" checked={days.includes(value)} aria-label={`${value}日`}
              onChange={() => toggleNumber(days, setDays, value)} />{value}日</label>)}
        </fieldset>}
        {scheduleError && <span id={`${id}-schedule-error`} role="alert">{scheduleError}</span>}
        <span className="ym-page-note">循环提醒按电脑当前时区计算，不存在的当地时刻会跳过。</span>
      </div>}

      <div className="ym-inline-form">
        <button className="ym-button" type="submit">{busy ? "正在保存…" : "保存提醒"}</button>
        <button className="ym-button" type="button" onClick={onCancel}>取消</button>
      </div>
    </fieldset>
  </form>;
}
