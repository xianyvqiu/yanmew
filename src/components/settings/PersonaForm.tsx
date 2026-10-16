import { useId, useState, type FormEvent } from "react";
import type { AppError, Persona, PersonaInput } from "../../types";

type PersonaFormProps = {
  persona: Persona | null;
  busy: boolean;
  error: AppError | null;
  onSave: (input: PersonaInput) => Promise<void>;
  onCancel: () => void;
};

const colorPattern = /^#[\da-f]{6}$/i;

export default function PersonaForm({ persona, busy, error, onSave, onCancel }: PersonaFormProps) {
  const id = useId();
  const [name, setName] = useState(persona?.name ?? "");
  const [prompt, setPrompt] = useState(persona?.prompt ?? "");
  const [initialPrompt, setInitialPrompt] = useState(persona?.initialPrompt ?? "");
  const [color, setColor] = useState(persona?.color ?? "#ffb6c1");
  const [selfEvolveEnabled, setSelfEvolveEnabled] = useState(persona?.selfEvolveEnabled ?? true);
  const [validationErrors, setValidationErrors] = useState<Partial<Record<"name" | "prompt" | "color", string>>>({});

  const errorFor = (field: "name" | "prompt" | "color") =>
    validationErrors[field] ?? (error?.field === field ? error.message : undefined);
  const nameError = errorFor("name");
  const promptError = errorFor("prompt");
  const colorError = errorFor("color");
  const colorPickerValue = colorPattern.test(color) ? color : "#ffb6c1";

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (busy) return;

    const nextErrors: typeof validationErrors = {};
    if (!name.trim()) nextErrors.name = "请输入人设名称";
    if (!prompt.trim()) nextErrors.prompt = "请输入当前设定";
    if (!colorPattern.test(color)) nextErrors.color = "颜色必须为 #RRGGBB 格式";
    setValidationErrors(nextErrors);
    if (Object.keys(nextErrors).length > 0) return;
    if (persona && !initialPrompt.trim()) return;

    await onSave({
      name: name.trim(),
      prompt,
      selfEvolveEnabled,
      color,
      chatBgOpacity: persona?.chatBgOpacity ?? 85,
      chatBgBlur: persona?.chatBgBlur ?? 0,
      saveAsInitial: false,
      initialPrompt: persona ? initialPrompt : prompt,
    });
  };

  return <form className="ym-persona-form" onSubmit={submit}>
    <fieldset disabled={busy}>
      <legend>{persona ? "编辑人设" : "新增人设"}</legend>

      <label className="ym-field" htmlFor={`${id}-name`}>
        人设名称
        <input
          id={`${id}-name`}
          aria-label="人设名称"
          value={name}
          required
          aria-invalid={Boolean(nameError)}
          aria-describedby={nameError ? `${id}-name-error` : undefined}
          onChange={event => {
            setName(event.target.value);
            setValidationErrors(current => ({ ...current, name: undefined }));
          }}
        />
        {nameError && <span id={`${id}-name-error`} role="alert">{nameError}</span>}
      </label>

      <label className="ym-field" htmlFor={`${id}-prompt`}>
        当前设定
        <textarea
          id={`${id}-prompt`}
          aria-label="当前设定"
          value={prompt}
          required
          aria-invalid={Boolean(promptError)}
          aria-describedby={promptError ? `${id}-prompt-error` : undefined}
          onChange={event => {
            setPrompt(event.target.value);
            setValidationErrors(current => ({ ...current, prompt: undefined }));
          }}
        />
        {promptError && <span id={`${id}-prompt-error`} role="alert">{promptError}</span>}
      </label>

      {persona && <label className="ym-field">初始设定<textarea aria-label="初始设定" required value={initialPrompt} onChange={event=>setInitialPrompt(event.target.value)} /><span className="ym-page-note">分别保存初始与当前设定；编辑初始设定不会自动覆盖当前内容。</span></label>}
      <div className="ym-field">
        <label htmlFor={`${id}-color`}>人设颜色（#RRGGBB）</label>
        <div className="ym-inline-form">
          <input
            id={`${id}-color`}
            aria-label="人设颜色"
            value={color}
            required
            pattern="#[\da-fA-F]{6}"
            aria-invalid={Boolean(colorError)}
            aria-describedby={colorError ? `${id}-color-error` : undefined}
            onChange={event => {
              setColor(event.target.value);
              setValidationErrors(current => ({ ...current, color: undefined }));
            }}
          />
          <input
            type="color"
            aria-label="选择人设颜色"
            value={colorPickerValue}
            onChange={event => {
              setColor(event.target.value);
              setValidationErrors(current => ({ ...current, color: undefined }));
            }}
          />
        </div>
        <div className="ym-inline-form" aria-label="预设人设颜色">
          {[["粉色", "#ffb6c1"], ["蓝色", "#b6c7ff"], ["绿色", "#b8dfc0"], ["紫色", "#d0b8ed"]].map(([label, value]) =>
            <button key={value} type="button" className="ym-button" aria-pressed={color.toLowerCase() === value}
              onClick={() => { setColor(value); setValidationErrors(current => ({ ...current, color: undefined })); }}>
              <span className="ym-persona-swatch" style={{ backgroundColor: value }} aria-hidden="true" />{label}
            </button>)}
        </div>
        {colorError && <span id={`${id}-color-error`} role="alert">{colorError}</span>}
      </div>

      <div className="ym-field">
        <label htmlFor={`${id}-self-evolve`}>人设自调</label>
        <input
          id={`${id}-self-evolve`}
          type="checkbox"
          checked={selfEvolveEnabled}
          onChange={event => setSelfEvolveEnabled(event.target.checked)}
        />
        <span className="ym-page-note">仅在重要且明确的问题需要修正时小幅调整，日常小事不修改；完整回复成功后保存，初始设定保留。</span>
      </div>

      <div className="ym-inline-form">
        <button className="ym-button" type="submit">{busy ? "正在保存…" : "保存人设"}</button>
        <button className="ym-button" type="button" onClick={onCancel}>取消</button>
      </div>
    </fieldset>
  </form>;
}
