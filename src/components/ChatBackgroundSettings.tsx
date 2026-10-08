import { useCallback, useEffect, useId, useRef, useState } from "react";
import { backgroundApi, errorMessage } from "../api";
import type { ChatBackgroundInput, ChatBackgroundView, Persona } from "../types";
import { BackgroundImage } from "./ChatBackground";

type Props = { persona: Persona; contextRevision?: number; disabled?: boolean; onSaved: () => void; label?: string };
const MAX_BYTES = 10 * 1024 * 1024;

export default function ChatBackgroundSettings({ persona, contextRevision, disabled = false, onSaved, label = "聊天窗背景" }: Props) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [view, setView] = useState<ChatBackgroundView | null>(null);
  const [opacity, setOpacity] = useState(85);
  const [blur, setBlur] = useState(0);
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
      setImage(null); setPreview(next.imageDataUrl); setStale(false); dirty.current = false;
      if (fileInput.current) fileInput.current.value = "";
      if (next.imageError) setError(next.imageError.message);
    } catch (cause) { if (alive.current && generation === token.current) setError(errorMessage(cause)); }
    finally { if (alive.current && generation === token.current) setLoading(false); }
  }, [persona.id, contextRevision]);
  useEffect(() => {
    if (!open || busy) return;
    const config = JSON.stringify([persona.id, persona.chatBgFile, persona.chatBgOpacity, persona.chatBgBlur, contextRevision]);
    if (observedConfig.current === config) return;
    observedConfig.current = config;
    const changed = !view || view.record.personaId !== persona.id || view.record.file !== persona.chatBgFile
      || view.record.opacity !== persona.chatBgOpacity || view.record.blur !== persona.chatBgBlur;
    if (changed) {
      if (dirty.current) setStale(true);
      else void load();
    }
    // 只响应新的权威配置；保存回包先更新预览，不因通知尚未抵达而重读旧配置。
  }, [open, persona.id, persona.chatBgFile, persona.chatBgOpacity, persona.chatBgBlur, contextRevision, load, busy]);

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
      const saved = await backgroundApi.save(persona.id, { expected: view.record, opacity, blur, image: reset ? null : image, reset }, contextRevision);
      if (!alive.current || generation !== token.current) return;
      dirty.current = false; setStale(false); setImage(null);
      const source = reset ? null : preview;
      setView({ record: saved, imageDataUrl: source, imageError: null }); setPreview(source);
      setOpacity(saved.opacity); setBlur(saved.blur); setNotice(reset ? "已恢复默认背景" : "聊天背景已保存");
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
  return <div className="ym-background-settings">
    <button type="button" className="ym-button" disabled={busy} aria-expanded={open} aria-controls={`${id}-body`}
      onClick={() => setOpen(value => !value)}>{label}</button>
    <div id={`${id}-body`} hidden={!open}>
      <p className="ym-page-note">按人设保存，所有会话共用；JPG / PNG / WebP，最大 10MB。预览满意后点击保存。</p>
      {loading && <p role="status">正在读取背景…</p>}
      {error && <p role="alert">{error}</p>}
      {stale && <p role="alert">背景已在其他窗口更新。重新读取会丢弃本页未保存的背景修改。</p>}
      {notice && <p role="status">{notice}</p>}
      <fieldset disabled={blocked || !view}>
        <legend className="ym-page-note">{persona.name}的聊天背景</legend>
        <label className="ym-field">选择背景图片<input ref={fileInput} type="file" accept=".jpg,.jpeg,.png,.webp,image/jpeg,image/png,image/webp" onChange={event => choose(event.target.files?.[0])} /></label>
        {reading && <p role="status">正在读取图片…</p>}
        <div className="ym-background-preview" aria-label="聊天背景预览">
          {preview && <BackgroundImage source={preview} opacity={opacity} blur={blur} />}
          <p>{preview ? "聊天消息预览" : "默认纯色背景"}</p>
        </div>
        <label className="ym-field" htmlFor={`${id}-opacity`}>背景透明度：{opacity}%<input id={`${id}-opacity`} type="range" min="0" max="100" value={opacity} onChange={event => { dirty.current = true; setOpacity(Number(event.target.value)); setNotice(""); }} /></label>
        <label className="ym-field" htmlFor={`${id}-blur`}>背景模糊度：{blur}px<input id={`${id}-blur`} type="range" min="0" max="40" value={blur} onChange={event => { dirty.current = true; setBlur(Number(event.target.value)); setNotice(""); }} /></label>
        <div className="ym-inline-form">
          <button type="button" className="ym-button" disabled={stale} onClick={() => { void save(false); }}>{busy ? "正在保存…" : "保存背景"}</button>
          <button type="button" className="ym-button" disabled={stale} onClick={() => { void save(true); }}>恢复默认背景</button>
        </div>
      </fieldset>
      <button type="button" className="ym-button" disabled={blocked} onClick={() => { void load(); }}>重新读取背景</button>
    </div>
  </div>;
}
