import { useEffect, useId, useRef, useState, type FormEvent } from "react";
import { configurationApi, errorMessage } from "../../api";
import type { NumericSettingKey, Settings } from "../../types";
import "./MemorySettingsPanel.css";

const MAX_INTEGER = 2_147_483_647;
const memoryKeys = [
  "short_memory_min_chars", "short_memory_max_chars", "long_memory_min_chars", "long_memory_max_chars",
  "memory_merge_count", "recent_message_count", "recent_long_memory_count",
  "travel_journal_min_chars", "travel_journal_max_chars", "notebook_max_chars",
] as const satisfies readonly NumericSettingKey[];
type MemoryKey = (typeof memoryKeys)[number];
type Draft = Record<MemoryKey, string>;
type FieldErrors = Partial<Record<MemoryKey, string>>;

const definitions: Record<MemoryKey, { label: string; unit: string; group: "memory" | "context" | "tools"; help: string }> = {
  short_memory_min_chars: { label: "短期记忆最少字数", unit: "字", group: "memory", help: "每轮完整对话生成一条短期记忆。" },
  short_memory_max_chars: { label: "短期记忆最多字数", unit: "字", group: "memory", help: "按中文汉字个数计算。" },
  long_memory_min_chars: { label: "长期记忆最少字数", unit: "字", group: "memory", help: "只统计正文，不包含标题。" },
  long_memory_max_chars: { label: "长期记忆最多字数", unit: "字", group: "memory", help: "按中文汉字个数计算。" },
  memory_merge_count: { label: "每条长期记忆使用短期记忆数 k", unit: "条", group: "memory", help: "出现第 k+1 条尚未整合的短期记忆时，整合前 k 条。修改 k 不会重算已有长期记忆。" },
  recent_message_count: { label: "进入上下文的最近消息原文数", unit: "条", group: "context", help: "按当前会话最近消息计算。" },
  recent_long_memory_count: { label: "直接进入上下文的最近长期记忆数 m", unit: "条", group: "context", help: "m 与最近消息原文条数独立设置。" },
  travel_journal_min_chars: { label: "旅行游记最少字数", unit: "字", group: "memory", help: "按中文汉字个数计算。" },
  travel_journal_max_chars: { label: "旅行游记最多字数", unit: "字", group: "memory", help: "按中文汉字个数计算。" },
  notebook_max_chars: { label: "记事本 AI 维护建议上限", unit: "字", group: "tools", help: "这是软限制；已有或超限内容仍完整保存，不会截断。" },
};

function project(settings: Settings): Draft {
  return Object.fromEntries(memoryKeys.map(key => [key, settings[key]])) as Draft;
}

function validate(draft: Draft): FieldErrors {
  const errors: FieldErrors = {};
  const parsed: Partial<Record<MemoryKey, number>> = {};
  for (const key of memoryKeys) {
    const value = draft[key];
    if (!/^[1-9]\d*$/.test(value)) {
      errors[key] = "请输入规范的十进制正整数，不要使用符号、小数或前导零。";
      continue;
    }
    const number = Number(value);
    if (number > MAX_INTEGER) {
      errors[key] = `数值不能超过 ${MAX_INTEGER}。`;
      continue;
    }
    parsed[key] = number;
  }
  const pairs: [MemoryKey, MemoryKey, string][] = [
    ["short_memory_min_chars", "short_memory_max_chars", "短期记忆最多字数须大于或等于最少字数。"],
    ["long_memory_min_chars", "long_memory_max_chars", "长期记忆最多字数须大于或等于最少字数。"],
    ["travel_journal_min_chars", "travel_journal_max_chars", "旅行游记最多字数须大于或等于最少字数。"],
  ];
  for (const [minimum, maximum, message] of pairs) {
    if (parsed[minimum] !== undefined && parsed[maximum] !== undefined && parsed[minimum]! > parsed[maximum]!) errors[maximum] = message;
  }
  return errors;
}

export default function MemorySettingsPanel() {
  const id = useId();
  const mounted = useRef(false);
  const readGeneration = useRef(0);
  const draftRevision = useRef(0);
  const defaultGeneration = useRef(0);
  const saveLocked = useRef(false);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [baseline, setBaseline] = useState<Draft | null>(null);
  const [loading, setLoading] = useState(true);
  const [defaultsLoading, setDefaultsLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [readError, setReadError] = useState("");
  const [operationError, setOperationError] = useState("");
  const [serverErrors, setServerErrors] = useState<FieldErrors>({});
  const [notice, setNotice] = useState("");

  const readSettings = async () => {
    const generation = ++readGeneration.current;
    setLoading(true);
    setReadError("");
    try {
      const settings = await configurationApi.getSettings();
      if (!mounted.current || generation !== readGeneration.current) return;
      const value = project(settings);
      setDraft(value);
      setBaseline(value);
    } catch (cause) {
      if (mounted.current && generation === readGeneration.current) setReadError(errorMessage(cause));
    } finally {
      if (mounted.current && generation === readGeneration.current) setLoading(false);
    }
  };

  useEffect(() => {
    mounted.current = true;
    void readSettings();
    return () => {
      mounted.current = false;
      readGeneration.current += 1;
      defaultGeneration.current += 1;
    };
  }, []);

  const errors = draft ? validate(draft) : {};
  const changed = Boolean(draft && baseline && memoryKeys.some(key => draft[key] !== baseline[key]));
  const hasErrors = Object.keys(errors).length > 0;

  const edit = (key: MemoryKey, value: string) => {
    draftRevision.current += 1;
    setDraft(current => current ? { ...current, [key]: value } : current);
    setNotice("");
    setOperationError("");
    setServerErrors(current => ({ ...current, [key]: undefined }));
  };

  const restoreDefaults = async () => {
    if (!draft || defaultsLoading || saving) return;
    const revision = draftRevision.current;
    const generation = ++defaultGeneration.current;
    setDefaultsLoading(true);
    setOperationError("");
    setServerErrors({});
    setNotice("");
    try {
      const settings = await configurationApi.getSettingsDefaults();
      if (!mounted.current || generation !== defaultGeneration.current) return;
      if (revision !== draftRevision.current) {
        setNotice("读取默认值期间草稿已修改，未覆盖当前内容；如仍需恢复，请再次点击恢复默认值。");
        return;
      }
      setDraft(project(settings));
      draftRevision.current += 1;
      setNotice("默认值已填入草稿，请点击保存后生效。");
    } catch (cause) {
      if (mounted.current && generation === defaultGeneration.current) setOperationError(errorMessage(cause));
    } finally {
      if (mounted.current && generation === defaultGeneration.current) setDefaultsLoading(false);
    }
  };

  const save = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!draft || !baseline || saveLocked.current || defaultsLoading) return;
    const validation = validate(draft);
    if (Object.keys(validation).length || !memoryKeys.some(key => draft[key] !== baseline[key])) return;
    const changes = Object.fromEntries(memoryKeys.filter(key => draft[key] !== baseline[key]).map(key => [key, draft[key]])) as Partial<Settings>;
    saveLocked.current = true;
    setSaving(true);
    setOperationError("");
    setServerErrors({});
    setNotice("");
    try {
      const saved = await configurationApi.saveSettings(changes);
      if (!mounted.current) return;
      const value = project(saved);
      setDraft(value);
      setBaseline(value);
      draftRevision.current += 1;
      setNotice("记忆设置已保存。");
    } catch (cause) {
      if (mounted.current) {
        const details = cause && typeof cause === "object" ? cause as { field?: unknown; message?: unknown } : {};
        const field = typeof details.field === "string" && memoryKeys.includes(details.field as MemoryKey)
          ? details.field as MemoryKey : null;
        const message = errorMessage(cause);
        if (field) setServerErrors(current => ({ ...current, [field]: typeof details.message === "string" ? details.message : message }));
        else setOperationError(message);
      }
    } finally {
      saveLocked.current = false;
      if (mounted.current) setSaving(false);
    }
  };

  const field = (key: MemoryKey) => {
    const fieldId = `${id}-${key}`;
    const helpId = `${fieldId}-help`;
    const errorId = `${fieldId}-error`;
    const { label, unit, help } = definitions[key];
    const fieldError = errors[key] ?? serverErrors[key];
    return <label className="ym-field ym-memory-settings-field" key={key} htmlFor={fieldId}>
      <span>{label}</span>
      <span className="ym-memory-settings-input-wrap">
        <input id={fieldId} aria-label={label} aria-invalid={Boolean(fieldError)}
          aria-describedby={fieldError ? `${helpId} ${errorId}` : helpId} inputMode="numeric" type="text"
          value={draft?.[key] ?? ""} disabled={saving} onChange={event => edit(key, event.target.value)} />
        <span className="ym-memory-settings-unit">{unit}</span>
      </span>
      <span className="ym-memory-settings-help" id={helpId}>{help}</span>
      {fieldError && <span className="ym-memory-settings-error" id={errorId} role="alert">{fieldError}</span>}
    </label>;
  };

  return <section className="ym-page-card ym-memory-settings-panel" aria-labelledby={`${id}-title`}>
    <h2 id={`${id}-title`}>记忆设置</h2>
    <p className="ym-page-note">以下是全局配置；短期记忆、长期记忆、记事本和旅行游记数据仍按会话分别保存。聊天结合全文和语义检索回忆长期记忆，信息不足时可继续查询来源。可在聊天窗打开记事本小窗查看、编辑记忆，重新整合或重试待处理记忆。</p>
    {readError ? <div className="ym-page-error" role="alert">配置读取失败：{readError}
      <button className="ym-button" type="button" onClick={() => void readSettings()}>重试读取</button>
    </div> : loading ? <p role="status">正在读取设置…</p> : draft && <form onSubmit={save}>
      <fieldset className="ym-memory-settings-group" disabled={saving}>
        <legend>记忆长度与整合</legend>
        <div className="ym-memory-settings-grid">
          {memoryKeys.filter(key => definitions[key].group === "memory").map(field)}
        </div>
        <p className="ym-page-note">短期记忆和长期记忆是中性事实记录，不使用人设口吻，不需要任何氛围总结或主观评价，必须只是如同剧本一样的叙事描述。</p>
      </fieldset>
      <fieldset className="ym-memory-settings-group" disabled={saving}>
        <legend>上下文与工具</legend>
        <div className="ym-memory-settings-grid">
          {memoryKeys.filter(key => definitions[key].group !== "memory").map(field)}
        </div>
        <p className="ym-page-note">最近消息原文条数与最近长期记忆数 m 相互独立。记事本建议上限只指导 AI 后续整理，超过后内容仍完整保留，不截断正文。</p>
      </fieldset>
      <div className="ym-memory-settings-actions">
        <button className="ym-button" type="button" disabled={saving || defaultsLoading} onClick={() => void restoreDefaults()}>
          {defaultsLoading ? "正在读取默认值…" : "恢复默认值"}
        </button>
        <button className="ym-button ym-memory-settings-save" type="submit" disabled={saving || defaultsLoading || !changed || hasErrors}>
          {saving ? "正在保存…" : "保存"}
        </button>
      </div>
      {operationError && <p className="ym-page-error" role="alert">操作失败：{operationError}</p>}
      {notice && <p className="ym-memory-settings-notice" role="status">{notice}</p>}
    </form>}
  </section>;
}
