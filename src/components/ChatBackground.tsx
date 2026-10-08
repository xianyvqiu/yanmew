import { useEffect, useState, type CSSProperties } from "react";
import { backgroundApi, errorMessage } from "../api";
import type { Persona } from "../types";
import "./ChatBackground.css";

export function BackgroundImage({ source, opacity, blur }: { source: string; opacity: number; blur: number }) {
  return <div className="ym-background-image" aria-hidden="true" style={{ "--ym-background-blur": `${blur}px`, "--ym-background-opacity": opacity / 100 } as CSSProperties}>
    <img src={source} alt="" draggable={false} />
  </div>;
}

/** 背景只消费当前人设资源，不读取任意本机路径，也不影响聊天状态。 */
export default function ChatBackground({ persona, contextRevision }: { persona: Persona; contextRevision: number }) {
  const [image, setImage] = useState<{ personaId: string; file: string; source: string } | null>(null);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let disposed = false;
    setError(""); setImage(null);
    if (persona.chatBgFile) {
      void backgroundApi.get(persona.id, contextRevision).then(view => {
        if (disposed) return;
        if (view.record.personaId !== persona.id || view.record.file !== persona.chatBgFile) return;
        if (view.imageError) setError(view.imageError.message);
        else if (view.imageDataUrl) setImage({ personaId: persona.id, file: persona.chatBgFile!, source: view.imageDataUrl });
      }).catch(cause => { if (!disposed) setError(errorMessage(cause)); });
    }
    return () => { disposed = true; };
  }, [persona.id, persona.chatBgFile, contextRevision, attempt]);
  const source = image?.personaId === persona.id && image.file === persona.chatBgFile ? image.source : null;
  return <>
    {source && <BackgroundImage source={source} opacity={persona.chatBgOpacity} blur={persona.chatBgBlur} />}
    {error && <div className="ym-page-error ym-background-error" role="alert">{error}，聊天仍可使用。
      <button type="button" className="ym-button" onClick={() => setAttempt(value => value + 1)}>重试读取背景</button>
    </div>}
  </>;
}
