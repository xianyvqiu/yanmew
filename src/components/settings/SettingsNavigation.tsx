import { useEffect, useId, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";

const scopedTitle="人设与会话设置";
export const settingsGroups = [
  { title: "基础服务", items: ["通用", "提供商", "人设", "用户设定", "Skill 仓库", "记忆", "时间与桌宠", "提醒", "时间与天气"] },
  { title: "扩展服务", items: ["喵币与签到", "现实记账", "喵币账单", "食物", "日历", "小游戏插件"] },
  { title: scopedTitle, items: ["桌宠状态", "桌宠自定义创作", "出行", "饰品"] },
];

function NavigationGroup({ title, open, onToggle, children }: {
  title: string; open: boolean; onToggle: () => void; children: ReactNode;
}) {
  const id = useId(), body = useRef<HTMLDivElement>(null), trigger = useRef<HTMLButtonElement>(null);
  const [present, setPresent] = useState(open);
  useEffect(() => {
    if (open) { setPresent(true); return; }
    const timer = window.setTimeout(() => setPresent(false), window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ? 0 : 180);
    return () => window.clearTimeout(timer);
  }, [open]);
  useLayoutEffect(() => {
    if (!body.current) return;
    if (!open && body.current.contains(document.activeElement)) trigger.current?.focus({ preventScroll: true });
    body.current.inert = !open;
  }, [open]);
  return <section className="ym-settings-group">
    <h2><button ref={trigger} className="ym-button ym-settings-group-toggle" type="button" aria-expanded={open} aria-controls={id} onClick={onToggle}>
      <span>{title}</span><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
    </button></h2>
    {(present || open) && <div ref={body} id={id} className="ym-settings-group-body" aria-hidden={!open} data-closing={!open}>{children}</div>}
  </section>;
}

export default function SettingsNavigation({ page, onSelect }: { page: string; onSelect: (page: string) => void }) {
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const previousPage = useRef(page);
  const toggle = (title: string) => setExpanded(current => ({ ...current, [title]: !current[title] }));
  // 外部详情跳转或窄窗选择后，展开目标所在分组；首次打开仍保持全部收起。
  useEffect(() => {
    if (previousPage.current === page) return;
    previousPage.current = page;
    const group = settingsGroups.find(group => group.items.includes(page));
    if (group) setExpanded(current => ({ ...current, [group.title]: true, ...(group.title !== scopedTitle ? { "全局设置": true } : {}) }));
  }, [page]);
  const items = (group: typeof settingsGroups[number]) => group.items.map((item, index) => <button key={item} type="button" className="ym-button ym-settings-nav-item"
    style={{ "--ym-nav-delay": `${Math.min(index, 6) * 35}ms` } as CSSProperties}
    onClick={() => onSelect(item)} aria-current={item === page ? "page" : undefined}>{item}</button>);
  return <nav className="ym-settings-nav" aria-label="设置导航">
    <NavigationGroup title="全局设置" open={!!expanded["全局设置"]} onToggle={() => toggle("全局设置")}>
      {settingsGroups.slice(0, 2).map(group => <NavigationGroup key={group.title} title={group.title} open={!!expanded[group.title]} onToggle={() => toggle(group.title)}>{items(group)}</NavigationGroup>)}
    </NavigationGroup>
    <NavigationGroup title={scopedTitle} open={!!expanded[scopedTitle]} onToggle={() => toggle(scopedTitle)}>{items(settingsGroups[2])}</NavigationGroup>
    <button type="button" className="ym-button ym-settings-nav-item" aria-current={page === "帮助与使用手册" ? "page" : undefined} onClick={() => onSelect("帮助与使用手册")}>帮助与使用手册</button>
  </nav>;
}
