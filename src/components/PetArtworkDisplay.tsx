import { useEffect, useState } from "react";
import { petCreationApi } from "../api";
import type { PetArtwork, PetCreation } from "../types";

/** 图片读取失败只在当前库版本尝试一次；旧人设和旧读取结果不能覆盖新状态。 */
export default function PetArtworkDisplay({ personaId, expression }: { personaId: string; expression: string }) {
  const [snapshot, setSnapshot] = useState<{ personaId: string; library: PetCreation } | null>(null);
  const [source, setSource] = useState<{ key: string; url: string } | null>(null);
  const [failed, setFailed] = useState<Set<string>>(() => new Set());
  useEffect(() => {
    let alive = true;
    let generation = 0;
    let revision = -1;
    let stop: (() => void) | undefined;
    setSnapshot(null);
    setFailed(new Set());
    const refresh = () => {
      if (!personaId) return;
      const request = ++generation;
      void petCreationApi.get(personaId).then(library => {
        if (!alive || request !== generation || library.revision < revision) return;
        revision = library.revision;
        setSnapshot({ personaId, library });
      }).catch(() => { if (alive && request === generation) setSnapshot(null); });
    };
    // 先订阅再补查，避免第一次读取与事件监听之间丢失开关变化。
    void petCreationApi.listen(refresh).then(unlisten => {
      if (!alive) { unlisten(); return; }
      stop = unlisten;
      refresh();
    }).catch(() => { if (alive) refresh(); });
    return () => { alive = false; stop?.(); };
  }, [personaId]);
  const library = snapshot?.personaId === personaId ? snapshot.library : null;
  const imageMode = Boolean(library?.enabled && expression.startsWith("@pet-image:"));
  const id = imageMode ? expression.slice(11) : null;
  const imageKey = (item: PetArtwork) => `${personaId}:${library?.revision}:${item.id}:${item.imageFile}`;
  const base = library?.items.find(item => item.kind === "base" && item.imageFile);
  const requested = library?.items.find(item => item.id === id && item.imageFile && (item.enabled || item.kind === "base"));
  const item = requested && !failed.has(imageKey(requested)) ? requested : base && !failed.has(imageKey(base)) ? base : undefined;
  const key = item ? imageKey(item) : "";
  const fail = () => { if (key) setFailed(previous => new Set(previous).add(key)); };
  useEffect(() => {
    let alive = true;
    setSource(null);
    if (imageMode && item) {
      void petCreationApi.image(personaId, item.id).then(url => {
        if (alive) setSource({ key, url });
      }).catch(() => { if (alive) setFailed(previous => new Set(previous).add(key)); });
    }
    return () => { alive = false; };
  }, [key, imageMode]);
  return <span className={`pet-expression${imageMode ? " pet-expression-image" : ""}`}>
    {imageMode && source?.key === key && source.url
      ? <img src={source.url} alt={item?.name || "桌宠"} draggable={false} onError={fail} />
      : expression.startsWith("@pet-image:") ? "(・ω・)" : expression}
  </span>;
}
