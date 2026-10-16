import { useEffect, useRef, useState } from "react";
import type { Session, UserProfile } from "../types";
import "./ChatSessionSidebar.css";

type IconName = "panel" | "add" | "profile" | "options" | "notebook" | "settings";
function SidebarIcon({ name }: { name: IconName }) {
  const paths: Record<IconName, React.ReactNode> = {
    panel: <><rect x="3" y="4" width="18" height="16" rx="3" /><path d="M9 4v16" /></>,
    add: <><path d="M12 5v14M5 12h14" /></>,
    profile: <><circle cx="12" cy="8" r="3" /><path d="M5 21v-2a7 7 0 0 1 14 0v2" /></>,
    options: <><path d="M4 7h16M4 17h16" /><circle cx="9" cy="7" r="2" /><circle cx="15" cy="17" r="2" /></>,
    notebook: <><path d="M6 3h13v18H6a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2ZM8 3v18M11 8h5M11 12h5" /></>,
    settings: <><circle cx="12" cy="12" r="3" /><path d="m9 3-1 3-3 1-2 3 2 2-1 3 3 2 1 3h4l2-3 3-1 2-3-2-2 1-3-3-2-1-3Z" /></>,
  };
  return <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[name]}</svg>;
}

/** 侧栏只负责展示和交互目标；写入仍通过聊天页的既有会话业务入口。 */
export default function ChatSessionSidebar({ sessions, profiles, currentId, contextKey, collapsed, disabled, bindingDisabled, memoryDisabled,
  onCollapse, onCreate, onSelect, onRename, onDelete, onPin, onBind, onSessionSettings, onMemory, onSettings,
}: {
  sessions: Session[]; profiles: UserProfile[]; currentId: string | null; contextKey: string; collapsed: boolean;
  disabled: boolean; bindingDisabled: boolean; memoryDisabled: boolean;
  onCollapse: (value: boolean) => void; onCreate: () => void; onSelect: (id: string) => void;
  onRename: (session: Session) => void; onDelete: (session: Session) => void; onPin: (session: Session) => void;
  onBind: () => void; onSessionSettings: () => void; onMemory: () => void; onSettings: () => void;
}) {
  const [menu, setMenu] = useState<{ session: Session; x: number; y: number } | null>(null);
  const menuElement = useRef<HTMLDivElement>(null);
  useEffect(() => { setMenu(null); }, [contextKey, collapsed, disabled]);
  useEffect(() => {
    if (!menu) return;
    if (!sessions.some(item => item.id === menu.session.id)) { setMenu(null); return; }
    menuElement.current?.querySelector("button")?.focus();
    const outside = (event: PointerEvent) => { if (!menuElement.current?.contains(event.target as Node)) setMenu(null); };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") { event.preventDefault(); setMenu(null); } };
    const blur = () => setMenu(null);
    window.addEventListener("pointerdown", outside);
    window.addEventListener("keydown", escape);
    window.addEventListener("blur", blur);
    return () => { window.removeEventListener("pointerdown", outside); window.removeEventListener("keydown", escape); window.removeEventListener("blur", blur); };
  }, [menu, sessions]);
  const footer = (label: string, icon: IconName, action: () => void, unavailable = disabled) => <button type="button" className="ym-chat-sidebar__action" aria-label={label} title={collapsed ? label : undefined} disabled={unavailable} onClick={action}>
    <SidebarIcon name={icon} /><span className="ym-chat-sidebar__label">{label}</span>
  </button>;
  const groups = [{ title: "已置顶", items: sessions.filter(item => item.isPinned) }, { title: "最近会话", items: sessions.filter(item => !item.isPinned) }];
  return <>
    {!collapsed && <button type="button" className="ym-chat-sidebar-scrim" aria-label="关闭会话侧栏" onClick={() => onCollapse(true)} />}
    <aside className="ym-chat-sidebar" aria-label="会话侧栏" data-collapsed={collapsed} onScrollCapture={() => setMenu(null)}>
      <div className="ym-chat-sidebar__top"><strong className="ym-chat-sidebar__label">我们的对话</strong><button type="button" className="ym-chat-sidebar__toggle" aria-label={collapsed ? "展开会话侧栏" : "收起会话侧栏"} aria-expanded={!collapsed} aria-controls="chat-session-list" title={collapsed ? "展开会话侧栏" : "收起会话侧栏"} onClick={() => onCollapse(!collapsed)}><SidebarIcon name="panel" /></button></div>
      {footer("新建会话", "add", onCreate)}
      <nav id="chat-session-list" className="ym-chat-sidebar__list" aria-label="会话列表" hidden={collapsed}>
        {!sessions.length && <p className="ym-page-note">暂无会话，点击上方新建。</p>}
        {groups.filter(group => group.items.length).map(group => <section key={group.title} aria-label={group.title}>
          <h2>{group.title}</h2>
          {group.items.map(session => {
            const profileName = profiles.find(profile => profile.id === session.userProfileId)?.name ?? "未绑定";
            return <button type="button" key={session.id} className="ym-chat-sidebar__session" aria-label={`${session.title} · 用户设定：${profileName}`} aria-current={session.id === currentId ? "page" : undefined} disabled={disabled}
              onClick={() => { setMenu(null); if (session.id !== currentId) onSelect(session.id); }} onContextMenu={event => {
                event.preventDefault(); if (disabled) return;
                const rect = event.currentTarget.getBoundingClientRect();
                setMenu({ session, x: Math.max(8, Math.min(event.clientX || rect.left + 12, window.innerWidth - 188)), y: Math.max(8, Math.min(event.clientY || rect.top + 12, window.innerHeight - 148)) });
              }}>
              <span className="ym-chat-sidebar__session-title">{session.title}</span><span className="ym-chat-sidebar__session-profile">用户设定：{profileName}</span>
            </button>;
          })}
        </section>)}
      </nav>
      <div className="ym-chat-sidebar__footer">
        {footer("绑定用户设定", "profile", onBind, bindingDisabled)}
        {footer("会话设置", "options", onSessionSettings)}
        {footer("查看记事本", "notebook", onMemory, memoryDisabled)}
        {footer("设置", "settings", onSettings)}
      </div>
    </aside>
    {menu && <div ref={menuElement} className="ym-chat-session-menu" role="menu" aria-label={`会话操作：${menu.session.title}`} style={{ left: menu.x, top: menu.y }} onKeyDown={event => {
      if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
      event.preventDefault(); const buttons = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>("button"));
      const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
      buttons[event.key === "Home" ? 0 : event.key === "End" ? buttons.length - 1 : (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length]?.focus();
    }}>
      <button type="button" role="menuitem" onClick={() => { const target = menu.session; setMenu(null); onPin(target); }}>{menu.session.isPinned ? "取消置顶" : "置顶"}</button>
      <button type="button" role="menuitem" onClick={() => { const target = menu.session; setMenu(null); onRename(target); }}>重命名</button>
      <button type="button" role="menuitem" data-variant="danger" onClick={() => { const target = menu.session; setMenu(null); onDelete(target); }}>删除会话</button>
    </div>}
  </>;
}
