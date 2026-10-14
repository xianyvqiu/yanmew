import { useState } from "react";
import { errorMessage, webSearchApi } from "../api";

/** 仅把 HTTP(S) 来源链接变成可点击项；其余文本仍按原样显示，不执行 HTML。 */
export default function ChatMessageText({ content }: { content: string }) {
  const [error, setError] = useState("");
  const pattern = /\[([^\]\n]{1,160})\]\((https?:\/\/[^\s<>"\u0000-\u001f]+)\)|(https?:\/\/[^\s<>"\u0000-\u001f]+)/g;
  const pieces: React.ReactNode[] = [];
  let start = 0;
  for (const match of content.matchAll(pattern)) {
    const index = match.index ?? 0;
    pieces.push(content.slice(start, index));
    const raw = match[2] ?? match[3];
    const url = raw.replace(/[.,;!，。；！）]+$/, "");
    let safe = false;
    try { const value = new URL(url); safe = !value.username && !value.password && url.length <= 2048; } catch { /* 无效地址保持原文。 */ }
    if (safe) pieces.push(<a key={index} href={url} onClick={event => { event.preventDefault(); setError(""); void webSearchApi.open(url).catch(cause => setError(errorMessage(cause))); }}>{match[1] ?? url}</a>);
    else pieces.push(match[0]);
    if (safe && !match[2]) pieces.push(raw.slice(url.length));
    start = index + match[0].length;
  }
  pieces.push(content.slice(start));
  return <><p>{pieces}</p>{error && <span role="alert" className="ym-page-error">{error}</span>}</>;
}
