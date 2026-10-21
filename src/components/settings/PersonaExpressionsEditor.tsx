import { useEffect, useRef, useState } from "react";
import { configurationApi, errorMessage } from "../../api";
import type { Persona } from "../../types";

const groups = [
  ["normal", "日常"], ["walk_left", "向左行走"], ["walk_right", "向右行走"], ["lifted", "被拎后颈"],
  ["peek_left", "左侧探头"], ["peek_right", "右侧探头"], ["rest_sit", "底部坐下"], ["rest_lie", "底部趴下"],
  ["click.happy", "点击 · 开心"], ["click.surprised", "点击 · 惊讶"], ["click.shy", "点击 · 害羞"],
  ["standby", "待机"], ["hungry", "饥饿"], ["craving", "等待投喂"], ["craving_favorite", "期待最爱"],
  ["eating", "进食"], ["eating_favorite", "喜欢地品尝"], ["sleepy", "困倦"], ["bored", "无聊"], ["thinking", "思考"],
] as const;
function fields(persona: Persona): Record<string, string> {
  const library = persona.effectiveExpressions ?? persona.expressions;
  return Object.fromEntries(groups.map(([key]) => {
    const [category, kind] = key.split(".");
    const value = kind ? (library?.[category] as Record<string, unknown> | undefined)?.[kind] : library?.[category];
    return [key, Array.isArray(value) ? value.filter(v => typeof v === "string").join("\n") : ""];
  }));
}
export default function PersonaExpressionsEditor({ persona, disabled, onSaved }: { persona: Persona; disabled: boolean; onSaved: () => void }) {
  const [draft, setDraft] = useState(() => fields(persona));
  const [dirty, setDirty] = useState(false), [saving, setSaving] = useState(false);
  const [error, setError] = useState(""), [notice, setNotice] = useState("");
  const expected = useRef(persona.expressions), edited = useRef(false), lock = useRef(false), alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    if (!edited.current && !lock.current) { setDraft(fields(persona)); expected.current = persona.expressions; }
  }, [persona.expressions, persona.effectiveExpressions]);
  const reset = () => {
    setDraft(fields(persona)); expected.current = persona.expressions;
    edited.current = false; setDirty(false); setError(""); setNotice("");
  };
  return <form className="ym-expression-editor" aria-label="颜文字表情编辑" onSubmit={event => {
    event.preventDefault(); if (lock.current || disabled) return;
    const library: Record<string, unknown> = { click: {} };
    for (const [key, label] of groups) {
      const lines = draft[key].split(/\r?\n/).filter(line => line.trim());
      if (!lines.length || lines.length > 8) { setError(`${label}请填写1–8个颜文字，每行一个`); return; }
      const [category, kind] = key.split(".");
      if (kind) (library.click as Record<string, unknown>)[kind] = lines; else library[category] = lines;
    }
    lock.current = true; setSaving(true); setError(""); setNotice("");
    void configurationApi.saveExpressions(persona.id, expected.current, library).then(saved => {
      if (!alive.current) return;
      expected.current = saved.expressions; setDraft(fields(saved)); edited.current = false; setDirty(false);
      setNotice("颜文字已保存"); onSaved();
    }).catch(cause => { if (alive.current) setError(errorMessage(cause)); }).finally(() => {
      lock.current = false; if (alive.current) setSaving(false);
    });
  }}>
    <h4>颜文字表情</h4>
    <p className="ym-page-note">每个分类可以直接修改，每行一个，最多8个；请保持简短以免裁切。保存不调用AI，图片模式仍使用图片库。</p>
    <div className="ym-expression-fields">{groups.map(([key, label]) => <label className="ym-field" key={key}>{label}<textarea
      aria-label={`${label}颜文字`} rows={2} value={draft[key]} disabled={disabled || saving}
      onChange={event => { edited.current = true; setDirty(true); setNotice(""); setDraft(previous => ({ ...previous, [key]: event.target.value })); }} /></label>)}</div>
    {error && <p role="alert" className="ym-page-error">{error}</p>}{notice && <p role="status">{notice}</p>}
    <div className="ym-inline-form"><button className="ym-button" disabled={disabled || saving || !dirty}>{saving ? "正在保存…" : "保存颜文字"}</button>
      <button type="button" className="ym-button" disabled={disabled || saving || !dirty} onClick={reset}>取消修改</button></div>
  </form>;
}
