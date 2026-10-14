import { useEffect, useId, useRef, useState, type FormEvent } from "react";
import { configurationApi, errorMessage } from "../../api";
import type { NumericSettingKey, Settings } from "../../types";
import "./PetTimingPanel.css";

const MAX_INTEGER = 2_147_483_647;
const timingKeys = [
  "hungry_minutes", "bored_minutes", "sleep_idle_minutes", "sleep_min_minutes", "sleep_max_minutes",
  "travel_min_online_hours", "travel_max_online_hours",
  "intimacy_threshold_1", "intimacy_threshold_2", "intimacy_threshold_3", "intimacy_threshold_4",
  "accessory_cost_1", "accessory_cost_2", "accessory_cost_3", "accessory_cost_4",
] as const satisfies readonly NumericSettingKey[];
type PetTimingKey = (typeof timingKeys)[number];
type Draft = Record<PetTimingKey, string>;
type FieldErrors = Partial<Record<PetTimingKey, string>>;

const definitions: Record<PetTimingKey, { label: string; unit: string; group: string }> = {
  hungry_minutes: { label: "饥饿触发", unit: "分钟", group: "timing" },
  bored_minutes: { label: "无聊触发", unit: "分钟", group: "timing" },
  sleep_idle_minutes: { label: "无互动入睡", unit: "分钟", group: "timing" },
  sleep_min_minutes: { label: "睡眠最短时长", unit: "分钟", group: "timing" },
  sleep_max_minutes: { label: "睡眠最长时长", unit: "分钟", group: "timing" },
  travel_min_online_hours: { label: "出行最短在线时长", unit: "小时", group: "travel" },
  travel_max_online_hours: { label: "出行最长在线时长", unit: "小时", group: "travel" },
  intimacy_threshold_1: { label: "档位 1 亲密度", unit: "点", group: "levels" },
  intimacy_threshold_2: { label: "档位 2 亲密度", unit: "点", group: "levels" },
  intimacy_threshold_3: { label: "档位 3 亲密度", unit: "点", group: "levels" },
  intimacy_threshold_4: { label: "档位 4 亲密度", unit: "点", group: "levels" },
  accessory_cost_1: { label: "档位 1 制作价格", unit: "喵币", group: "levels" },
  accessory_cost_2: { label: "档位 2 制作价格", unit: "喵币", group: "levels" },
  accessory_cost_3: { label: "档位 3 制作价格", unit: "喵币", group: "levels" },
  accessory_cost_4: { label: "档位 4 制作价格", unit: "喵币", group: "levels" },
};

function project(settings: Settings): Draft {
  return Object.fromEntries(timingKeys.map(key => [key, settings[key]])) as Draft;
}

function validate(draft: Draft): FieldErrors {
  const errors: FieldErrors = {};
  const parsed: Partial<Record<PetTimingKey, number>> = {};
  for (const key of timingKeys) {
    const value = draft[key];
    if (!/^(?:0|[1-9]\d*)$/.test(value)) {
      errors[key] = "请输入规范的十进制整数，不要使用符号、小数或前导零。";
      continue;
    }
    const number = Number(value);
    parsed[key] = number;
    if (key.startsWith("intimacy_threshold_")) {
      if (number < 1 || number > 999) errors[key] = "亲密度档位须为 1–999 的整数。";
    } else if (key.startsWith("accessory_cost_")) {
      if (number > MAX_INTEGER) errors[key] = `价格不能超过 ${MAX_INTEGER} 喵币。`;
    } else if (number < 1 || number > MAX_INTEGER) {
      errors[key] = `请输入 1–${MAX_INTEGER} 之间的正整数。`;
    }
  }
  for (let level = 2; level <= 4; level += 1) {
    const previous = `intimacy_threshold_${level - 1}` as PetTimingKey;
    const current = `intimacy_threshold_${level}` as PetTimingKey;
    if (parsed[previous] !== undefined && parsed[current] !== undefined && parsed[current]! <= parsed[previous]!) {
      errors[current] = `档位 ${level} 亲密度须高于档位 ${level - 1}。`;
    }
  }
  const pairs: [PetTimingKey, PetTimingKey, string][] = [
    ["sleep_min_minutes", "sleep_max_minutes", "睡眠最长时长须大于或等于最短时长。"],
    ["travel_min_online_hours", "travel_max_online_hours", "出行最长在线时长须大于或等于最短时长。"],
  ];
  for (const [minimum, maximum, message] of pairs) {
    if (parsed[minimum] !== undefined && parsed[maximum] !== undefined && parsed[minimum]! > parsed[maximum]!) errors[maximum] = message;
  }
  return errors;
}

export default function PetTimingPanel() {
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
  const changed = Boolean(draft && baseline && timingKeys.some(key => draft[key] !== baseline[key]));
  const hasErrors = Object.keys(errors).length > 0;

  const edit = (key: PetTimingKey, value: string) => {
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
    if (Object.keys(validation).length || !timingKeys.some(key => draft[key] !== baseline[key])) return;
    const changes = Object.fromEntries(timingKeys.filter(key => draft[key] !== baseline[key]).map(key => [key, draft[key]])) as Partial<Settings>;
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
      setNotice("桌宠计时与档位设置已保存。");
    } catch (cause) {
      if (mounted.current) {
        const details = cause && typeof cause === "object" ? cause as { field?: unknown; message?: unknown } : {};
        const field = typeof details.field === "string" && timingKeys.includes(details.field as PetTimingKey)
          ? details.field as PetTimingKey : null;
        const message = errorMessage(cause);
        if (field) setServerErrors(current => ({ ...current, [field]: typeof details.message === "string" ? details.message : message }));
        else setOperationError(message);
      }
    } finally {
      saveLocked.current = false;
      if (mounted.current) setSaving(false);
    }
  };

  const field = (key: PetTimingKey) => {
    const fieldId = `${id}-${key}`;
    const errorId = `${fieldId}-error`;
    const { label, unit } = definitions[key];
    const fieldError = errors[key] ?? serverErrors[key];
    return <label className="ym-field ym-pet-timing-field" key={key} htmlFor={fieldId}>
      <span>{label}</span>
      <span className="ym-pet-timing-input-wrap">
        <input id={fieldId} aria-label={label} aria-invalid={Boolean(fieldError)}
          aria-describedby={fieldError ? errorId : undefined} inputMode="numeric" type="text"
          value={draft?.[key] ?? ""} disabled={saving} onChange={event => edit(key, event.target.value)} />
        <span className="ym-pet-timing-unit">{unit}</span>
      </span>
      {fieldError && <span className="ym-pet-timing-error" id={errorId} role="alert">{fieldError}</span>}
    </label>;
  };

  return <section className="ym-page-card ym-pet-timing-panel" aria-labelledby={`${id}-title`}>
    <h2 id={`${id}-title`}>时间与桌宠</h2>
    <p className="ym-page-note">计时规则为全局配置，桌宠活动时按当前会话分别累计。</p>
    {readError ? <div className="ym-page-error" role="alert">配置读取失败：{readError}
      <button className="ym-button" type="button" onClick={() => void readSettings()}>重试读取</button>
    </div> : loading ? <p role="status">正在读取设置…</p> : draft && <form onSubmit={save}>
      <fieldset className="ym-pet-timing-group" disabled={saving}>
        <legend>计时</legend>
        <div className="ym-pet-timing-grid">{timingKeys.filter(key => definitions[key].group === "timing").map(field)}</div>
        <p className="ym-page-note">饥饿、无聊与睡眠分别计时；默认无互动 45 分钟时随机触发无聊或睡眠。阈值不相等时，先达到的事件先触发。睡眠开始后，修改设置不会重算本次睡眠结束时间。</p>
      </fieldset>
      <fieldset className="ym-pet-timing-group" disabled={saving}>
        <legend>出行</legend>
        <div className="ym-pet-timing-grid">{timingKeys.filter(key => definitions[key].group === "travel").map(field)}</div>
        <p className="ym-page-note">只有当前会话在家空闲时累计出行时间。范围变化只影响以后抽取的旅行倒计时。</p>
      </fieldset>
      <fieldset className="ym-pet-timing-group" disabled={saving}>
        <legend>四档亲密度与饰品价格</legend>
        <div className="ym-pet-level-head" aria-hidden="true"><span>档位</span><span>亲密度门槛</span><span>制作价格</span></div>
        <div className="ym-pet-level-rows">{[1, 2, 3, 4].map(level => <div className="ym-pet-level-row" key={level}>
          <span className="ym-pet-level-label">档位 {level}</span>
          {field(`intimacy_threshold_${level}` as PetTimingKey)}
          {field(`accessory_cost_${level}` as PetTimingKey)}
        </div>)}</div>
        <p className="ym-page-note">饰品制作先使用尚未使用的最低档，前三档各一次，第四档可重复。修改档位不会退还或补发喵币，也不会改变已使用档位。</p>
      </fieldset>
      <div className="ym-pet-timing-actions">
        <button className="ym-button" type="button" disabled={saving || defaultsLoading} onClick={() => void restoreDefaults()}>
          {defaultsLoading ? "正在读取默认值…" : "恢复默认值"}
        </button>
        <button className="ym-button ym-pet-timing-save" type="submit" disabled={saving || defaultsLoading || !changed || hasErrors}>
          {saving ? "正在保存…" : "保存"}
        </button>
      </div>
      {operationError && <p className="ym-page-error" role="alert">操作失败：{operationError}</p>}
      {notice && <p className="ym-pet-timing-notice" role="status">{notice}</p>}
    </form>}
  </section>;
}
