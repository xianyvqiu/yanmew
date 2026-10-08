import { useId, useMemo, useState } from "react";
import { personaPromptDiff, type PersonaPromptChange } from "../utils/personaPromptDiff";
import "./PersonaPromptDiff.css";

type Props = { initialPrompt: string; currentPrompt: string };
type Mode = "inline" | "side";

function Fragments({ changes }: { changes: PersonaPromptChange[] }) {
  if (!changes.length) return <span className="ym-page-note">（空）</span>;
  return <>{changes.map((change, index) => {
    if (change.kind === "equal") return <span key={index}>{change.text}</span>;
    const content = <><span className="ym-prompt-diff-label">{change.kind === "added" ? "新增" : "删除"}</span>{change.text}</>;
    return change.kind === "added"
      ? <ins className="ym-prompt-diff-added" key={index}>{content}</ins>
      : <del className="ym-prompt-diff-removed" key={index}>{content}</del>;
  })}</>;
}

export default function PersonaPromptDiff({ initialPrompt, currentPrompt }: Props) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<Mode>("inline");
  // 只在用户展开后计算；配置刷新使用最新文本，不持久化或展示未保存草稿。
  const changes = useMemo(() => open ? personaPromptDiff(initialPrompt, currentPrompt) : [],
    [open, initialPrompt, currentPrompt]);
  const changed = changes.some(change => change.kind !== "equal");

  return <div className="ym-prompt-diff">
    <button type="button" className="ym-button" aria-expanded={open} aria-controls={`${id}-content`}
      onClick={() => setOpen(value => !value)}>查看与初始设定的差异</button>
    <section id={`${id}-content`} hidden={!open} aria-labelledby={`${id}-title`}>
      <h4 id={`${id}-title`}>当前与初始设定的累计差异</h4>
      <p className="ym-page-note">包含 AI 自调和手动编辑造成的变化，不是逐次 AI 修改记录；仅比较已保存的设定。</p>
      {open && (changed ? <>
        <div className="ym-inline-form" role="group" aria-label="差异显示方式">
          <button type="button" className="ym-button" aria-pressed={mode === "inline"} onClick={() => setMode("inline")}>行内对比</button>
          <button type="button" className="ym-button" aria-pressed={mode === "side"} onClick={() => setMode("side")}>左右对比</button>
        </div>
        {mode === "inline" ? <div className="ym-prompt-diff-text" aria-label="行内差异"><Fragments changes={changes} /></div>
          : <div className="ym-prompt-diff-columns">
            <section aria-label="初始设定对比"><h5>初始设定</h5><div className="ym-prompt-diff-text"><Fragments changes={changes.filter(change => change.kind !== "added")} /></div></section>
            <section aria-label="当前设定对比"><h5>当前设定</h5><div className="ym-prompt-diff-text"><Fragments changes={changes.filter(change => change.kind !== "removed")} /></div></section>
          </div>}
      </> : <p role="status">当前人设与初始设定一致</p>)}
    </section>
  </div>;
}
