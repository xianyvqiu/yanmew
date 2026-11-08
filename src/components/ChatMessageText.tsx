import { memo, useState, type ReactNode } from "react";
import Markdown, { type ExtraProps } from "react-markdown";
import remarkGfm from "remark-gfm";
import { errorMessage, webSearchApi } from "../api";
import "./ChatMessageText.css";
import { plainDisplay } from "./plainDisplay";

/** 只允许经用户点击、交给宿主打开的 HTTP(S) 地址。 */
function safeUrl(url: string): string | undefined {
  if (url.length > 2048 || !/^https?:\/\//i.test(url) || /[\u0000-\u0020\u007f]/.test(url)) return;
  try {
    const value = new URL(url);
    if (!value.username && !value.password) return url;
  } catch { /* 无效地址只显示文字，不提供跳转。 */ }
}

/** 仅还原强调节点中的常见括号颜文字，不预处理代码、网址或整段正文。 */
function kaomojiSource(content: string, node: ExtraProps["node"]): string | undefined {
  const start = node?.position?.start.offset;
  const end = node?.position?.end.offset;
  if (start === undefined || end === undefined) return;
  const source = content.slice(start, end);
  const surrounding = content.slice(Math.max(0, start - 1), end + 1);
  const candidate = /^[（(]/.test(surrounding) && /[）)]$/.test(surrounding) ? surrounding : source;
  const face = candidate.replace(/^[*_]+|[*_]+$/g, "");
  if (!/^[（(][^()（）\r\n]{1,40}[）)]$/.test(face)) return;
  const inside = face.slice(1, -1);
  // 排除自然语言和数字表达式，同时要求有明确的眼睛/嘴部符号。
  if (/^[\p{P}\p{S}\sωΩзツっつTtQqOoUuVvXxWwDd]+$/u.test(inside)
    && /[\^ωΩ・･]|[TQOoUuVvXx>][._-][TQOoUuVvXx<]|:з/u.test(inside)) return source;
}

/** AI 正文渲染 Markdown；持久化和编辑使用原文，原文区只在展示与复制时去掉包装。 */
function ChatMessageText({ content, markdown = true, expression, streaming = false }: { content: string; markdown?: boolean; expression?: string | null; streaming?: boolean }) {
  const [error, setError] = useState("");
  const prepared = plainDisplay(content, markdown ? expression : undefined, streaming);
  const link = (url: string, children: ReactNode, key?: number) => <a key={key} href={url} onClick={event => {
    event.preventDefault();
    setError("");
    void webSearchApi.open(url).catch(cause => setError(errorMessage(cause)));
  }}>{children}</a>;

  // 用户消息保留原有的文字及来源链接展示，不解释 Markdown 标记。
  const plainText = () => {
    const pattern = /\[([^\]\n]{1,160})\]\((https?:\/\/[^\s<>"\u0000-\u001f]+)\)|(https?:\/\/[^\s<>"\u0000-\u001f]+)/g;
    const pieces: ReactNode[] = [];
    let start = 0;
    for (const match of content.matchAll(pattern)) {
      const index = match.index ?? 0;
      pieces.push(content.slice(start, index));
      const raw = match[2] ?? match[3];
      const url = safeUrl(raw.replace(/[.,;!，。；！）]+$/, ""));
      pieces.push(url ? link(url, match[1] ?? url, index) : match[0]);
      if (url && !match[2]) pieces.push(raw.slice(url.length));
      start = index + match[0].length;
    }
    pieces.push(content.slice(start));
    return <p>{pieces}</p>;
  };

  return <div className={`ym-message-text${markdown ? " ym-markdown" : ""}`}>
    {markdown ? <Markdown
      remarkPlugins={[[remarkGfm, { singleTilde: false }]]}
      urlTransform={(url, key) => key === "href" ? safeUrl(url) ?? "" : ""}
      components={{
        a: ({ href, children }) => href && safeUrl(href) ? link(href, children) : <span>{children}</span>,
        code: ({ children, ...props }) => prepared.values.has(String(children))
          ? <span className="ym-plain-display">{prepared.values.get(String(children))}</span>
          : <code className={props.className}>{children}</code>,
        em: ({ node, children }) => kaomojiSource(prepared.markdown, node) ?? <em>{children}</em>,
        strong: ({ node, children }) => kaomojiSource(prepared.markdown, node) ?? <strong>{children}</strong>,
        // 不自动请求消息中的图片；用户上传的聊天附件仍由 ChatImage 显示。
        img: ({ alt }) => <span className="ym-markdown-image">[图片：{alt || "未加载"}]</span>,
        pre: ({ children }) => <pre tabIndex={0} aria-label="代码块">{children}</pre>,
        table: ({ children }) => <div className="ym-markdown-table" role="region" aria-label="表格" tabIndex={0}><table>{children}</table></div>,
      }}
    >{prepared.markdown}</Markdown> : plainText()}
    {error && <span role="alert" className="ym-page-error">{error}</span>}
  </div>;
}

// 流式追加只重新解析变化的正文，已有历史气泡继续复用。
export default memo(ChatMessageText);
