import { useCallback, useEffect, useId, useRef, useState } from "react";
import { backgroundApi, errorMessage } from "../api";
import type { ChatAppearance, ChatBackgroundInput, ChatBackgroundView, Persona } from "../types";
import { BackgroundImage } from "./ChatBackground";
import { chatAppearanceAttributes, DEFAULT_CHAT_APPEARANCE } from "./chatAppearance";

type Props = { persona: Persona; contextRevision?: number; disabled?: boolean; onSaved: () => void; label?: string };
const MAX_BYTES = 10 * 1024 * 1024;

export default function ChatBackgroundSettings({ persona, contextRevision, disabled = false, onSaved, label = "聊天外观" }: Props) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [view, setView] = useState<ChatBackgroundView | null>(null);
  const [opacity, setOpacity] = useState(85);
  const [blur, setBlur] = useState(0);
  const [appearance, setAppearance] = useState<ChatAppearance>(DEFAULT_CHAT_APPEARANCE);
  const [image, setImage] = useState<ChatBackgroundInput["image"]>(null);
  const [preview, setPreview] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [reading, setReading] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [stale, setStale] = useState(false);
  const alive = useRef(true);
  const token = useRef(0);
  const dirty = useRef(false);
  const lock = useRef(false);
  const reader = useRef<FileReader | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const chooseToken = useRef(0);
  const observedConfig = useRef<string | null>(null);
  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; token.current++; chooseToken.current++; reader.current?.abort(); };
  }, []);
  const load = useCallback(async () => {
    const generation = ++token.current;
    chooseToken.current++; reader.current?.abort();
    setLoading(true); setReading(false); setError(""); setNotice("");
    try {
      const next = await backgroundApi.get(persona.id, contextRevision);
      if (!alive.current || generation !== token.current || next.record.personaId !== persona.id) return;
      setView(next); setOpacity(next.record.opacity); setBlur(next.record.blur);
      setAppearance(next.record.appearance ?? DEFAULT_CHAT_APPEARANCE);
      setImage(null); setPreview(next.imageDataUrl); setStale(false); dirty.current = false;
      if (fileInput.current) fileInput.current.value = "";
      if (next.imageError) setError(next.imageError.message);
    } catch (cause) { if (alive.current && generation === token.current) setError(errorMessage(cause)); }
    finally { if (alive.current && generation === token.current) setLoading(false); }
  }, [persona.id, contextRevision]);
  useEffect(() => {
    if (!open || busy) return;
    const config = JSON.stringify([persona.id, persona.chatBgFile, persona.chatBgOpacity, persona.chatBgBlur, persona.chatAppearance, contextRevision]);
    if (observedConfig.current === config) return;
    observedConfig.current = config;
    const changed = !view || view.record.personaId !== persona.id || view.record.file !== persona.chatBgFile
      || view.record.opacity !== persona.chatBgOpacity || view.record.blur !== persona.chatBgBlur
      || JSON.stringify(view.record.appearance ?? DEFAULT_CHAT_APPEARANCE) !== JSON.stringify(persona.chatAppearance ?? DEFAULT_CHAT_APPEARANCE);
    if (changed) {
      if (dirty.current) setStale(true);
      else void load();
    }
    // 只响应新的权威配置；保存回包先更新预览，不因通知尚未抵达而重读旧配置。
  }, [open, persona.id, persona.chatBgFile, persona.chatBgOpacity, persona.chatBgBlur, persona.chatAppearance, contextRevision, load, busy]);

  const choose = (file?: File) => {
    if (!file || lock.current) return;
    if (!/\.(jpe?g|png|webp)$/i.test(file.name) || !file.size || file.size > MAX_BYTES) {
      setError("请选择不超过 10MB 的 JPG、PNG 或 WebP 图片");
      if (fileInput.current) fileInput.current.value = "";
      return;
    }
    const generation = ++chooseToken.current;
    reader.current?.abort();
    const current = new FileReader(); reader.current = current;
    setReading(true); setError(""); setNotice("");
    current.onload = () => {
      if (!alive.current || generation !== chooseToken.current) return;
      const encoded = String(current.result).split(",", 2)[1];
      if (!encoded) { setError("图片读取失败，请重新选择"); setReading(false); return; }
      const mime = /\.jpe?g$/i.test(file.name) ? "jpeg" : /\.webp$/i.test(file.name) ? "webp" : "png";
      setImage({ fileName: file.name, base64: encoded }); setPreview(`data:image/${mime};base64,${encoded}`);
      dirty.current = true; setReading(false);
    };
    current.onerror = () => { if (alive.current && generation === chooseToken.current) { setError("图片读取失败，请重新选择"); setReading(false); } };
    current.readAsDataURL(file);
  };
  const save = async (reset: boolean) => {
    if (!view || lock.current || disabled || stale || reading) return;
    lock.current = true; setBusy(true); setError(""); setNotice("");
    const generation = token.current;
    try {
      const saved = await backgroundApi.save(persona.id, { expected: view.record, opacity, blur, image: reset ? null : image, reset, ...(!reset ? { appearance } : {}) }, contextRevision);
      if (!alive.current || generation !== token.current) return;
      // 单独恢复背景时保留尚未保存的气泡/颜色预览。
      dirty.current = reset && JSON.stringify(appearance) !== JSON.stringify(saved.appearance ?? DEFAULT_CHAT_APPEARANCE);
      if (!reset) setAppearance(saved.appearance ?? DEFAULT_CHAT_APPEARANCE);
      setStale(false); setImage(null);
      const source = reset ? null : preview;
      setView({ record: saved, imageDataUrl: source, imageError: null }); setPreview(source);
      setOpacity(saved.opacity); setBlur(saved.blur); setNotice(reset ? `已恢复默认背景${dirty.current ? "；气泡和颜色的预览修改尚未保存" : ""}` : "聊天外观已保存");
      if (fileInput.current) fileInput.current.value = "";
      onSaved();
    } catch (cause) {
      if (alive.current && generation === token.current) {
        setError(errorMessage(cause));
        if (cause && typeof cause === "object" && "code" in cause && cause.code === "CONFLICT") setStale(true);
      }
    } finally { lock.current = false; if (alive.current) setBusy(false); }
  };
  const blocked = disabled || busy || loading || reading;
  const changeAppearance = (next: ChatAppearance) => { dirty.current = true; setAppearance(next); setNotice(""); };
  return <div className="ym-background-settings">
    <button type="button" className="ym-button" disabled={busy} aria-expanded={open} aria-controls={`${id}-body`}
      onClick={() => setOpen(value => !value)}>{label}</button>
    <div id={`${id}-body`} hidden={!open}>
      <p className="ym-page-note">背景、气泡和文字颜色按人设保存，所有会话共用。图片支持 JPG / PNG / WebP，最大 10MB。预览满意后点击保存外观。</p>
      {loading && <p role="status">正在读取外观…</p>}
      {error && <p role="alert">{error}</p>}
      {stale && <p role="alert">聊天外观已在其他窗口更新。重新读取会丢弃本页未保存的外观修改。</p>}
      {notice && <p role="status">{notice}</p>}
      <fieldset disabled={blocked || !view}>
        <legend className="ym-page-note">{persona.name}的聊天外观</legend>
        <label className="ym-field">选择背景图片<input ref={fileInput} type="file" accept=".jpg,.jpeg,.png,.webp,image/jpeg,image/png,image/webp" onChange={event => choose(event.target.files?.[0])} /></label>
        {reading && <p role="status">正在读取图片…</p>}
        <div className="ym-background-preview ym-chat-bubbles" aria-label="聊天背景预览" {...chatAppearanceAttributes(appearance)}>
          {preview && <BackgroundImage source={preview} opacity={opacity} blur={blur} />}
          <ol className="ym-message-list ym-background-preview-messages" aria-label="聊天气泡预览">
            <li data-role="user"><p>今天过得怎么样？</p></li>
            <li data-role="assistant"><p>很高兴见到你 (・ω・)</p></li>
          </ol>
        </div>
        <label className="ym-field" htmlFor={`${id}-opacity`}>背景透明度：{opacity}%<input id={`${id}-opacity`} type="range" min="0" max="100" value={opacity} onChange={event => { dirty.current = true; setOpacity(Number(event.target.value)); setNotice(""); }} /></label>
        <label className="ym-field" htmlFor={`${id}-blur`}>背景模糊度：{blur}px<input id={`${id}-blur`} type="range" min="0" max="40" value={blur} onChange={event => { dirty.current = true; setBlur(Number(event.target.value)); setNotice(""); }} /></label>
        <label className="ym-field" htmlFor={`${id}-bubble`}>聊天气泡样式
          <select id={`${id}-bubble`} value={appearance.bubbleStyle} onChange={event => changeAppearance({ ...appearance, bubbleStyle: event.target.value as ChatAppearance["bubbleStyle"] })}>
            <option value="solid">默认气泡</option><option value="transparent">透明气泡</option><option value="outline">仅描边气泡</option><option value="frosted">毛玻璃气泡</option>
          </select>
        </label>
        <p className="ym-page-note">文字始终清晰；毛玻璃只模糊气泡背后的背景。透明样式下可自行调整文字颜色，避免与背景混在一起。</p>
        <div className="ym-chat-color-fields">
          {([ ["userTextColor", "用户"], ["assistantTextColor", "AI"] ] as const).map(([key, name]) => <div className="ym-chat-color-field" key={key}>
            <label className="ym-field" htmlFor={`${id}-${key}`}>{name}文字颜色
              <input id={`${id}-${key}`} type="color" value={appearance[key] ?? "#c77694"} onChange={event => changeAppearance({ ...appearance, [key]: event.target.value })} />
            </label>
            <span className="ym-page-note">{appearance[key] ?? "跟随主题"}</span>
            <button type="button" className="ym-button" aria-label={`${name}文字颜色跟随主题`} disabled={appearance[key] === null} onClick={() => changeAppearance({ ...appearance, [key]: null })}>跟随主题</button>
          </div>)}
        </div>
        <div><button type="button" className="ym-button" onClick={() => changeAppearance(DEFAULT_CHAT_APPEARANCE)}>恢复默认样式</button><span className="ym-page-note"> 仅重置气泡和文字颜色，保存后生效。</span></div>
        <div className="ym-inline-form">
          <button type="button" className="ym-button" disabled={stale} onClick={() => { void save(false); }}>{busy ? "正在保存…" : "保存外观"}</button>
          <button type="button" className="ym-button" disabled={stale} onClick={() => { void save(true); }}>恢复默认背景</button>
        </div>
      </fieldset>
      {(stale || error) && <div className="ym-background-reload"><p className="ym-page-note">从已保存配置重新加载，并放弃当前未保存的预览修改；不会调用 AI 或重新生成。</p><button type="button" className="ym-button" disabled={blocked} onClick={() => { void load(); }}>放弃修改并重新加载</button></div>}
    </div>
  </div>;
}
