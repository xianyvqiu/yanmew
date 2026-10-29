import { useEffect, useRef } from "react";
import type { UserProfile } from "../types";
import CardBrowserControls, {CardBrowserPagination} from "./CardBrowserControls";
import { useCardBrowser } from "../hooks/useCardBrowser";

/** 选择只绑定已有设定，详情跳转使用原设置窗口，不复制编辑表单。 */
export default function UserProfilePicker({ profiles, selectedId, defaultProfileId, onDefaultChange, busy, error, onSelect, onDetails, onClose }: {
  defaultProfileId?:string; onDefaultChange?:(id:string)=>void; profiles: UserProfile[]; selectedId: string | null; busy: boolean; error: string;
  onSelect: (id: string | null) => void; onDetails: (id: string) => void; onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const browser = useCardBrowser(profiles, item => `${item.name} ${item.content}`, item => item.updatedAt, item => item.id, 6);
  useEffect(() => { const element = dialog.current; element?.showModal(); return () => element?.close(); }, []);
  return <dialog ref={dialog} className="ym-session-dialog ym-profile-picker" aria-labelledby="profile-picker-title" onCancel={event => { event.preventDefault(); if (!busy) onClose(); }}>
    <div className="ym-page-heading"><h2 id="profile-picker-title">绑定用户设定</h2><button className="ym-button" disabled={busy} onClick={onClose}>关闭</button></div>
    <div className="ym-session-dialog-body">
    <p className="ym-page-note">点击卡片绑定到当前会话；修改绑定会中断正在生成的回复，已有聊天和记忆保留。</p>
    {onDefaultChange && <section className="ym-profile-default"><label className="ym-field">新会话默认用户设定<select aria-label="新会话的用户设定" value={defaultProfileId??""} disabled={busy} onChange={event=>onDefaultChange(event.target.value)}><option value="">未绑定</option>{profiles.map(profile=><option key={profile.id} value={profile.id}>{profile.name}</option>)}</select></label><p className="ym-page-note">用于此聊天窗口接下来创建的新会话，不改变已有会话的绑定。</p></section>}
    {error && <p className="ym-page-error" role="alert">{error}</p>}
    {profiles.length > 0 ? <>
      <CardBrowserControls browser={browser} label="用户设定" timeLabel="更新时间" disabled={busy} />
      <div className="ym-profile-picker-list">{browser.items.map(profile => <article className="ym-profile-picker-card" key={profile.id} data-selected={profile.id === selectedId}>
        <button type="button" className="ym-profile-picker-select" aria-label={`绑定 ${profile.name}`} aria-pressed={profile.id === selectedId} disabled={busy || profile.id === selectedId} onClick={() => onSelect(profile.id)}>
          <strong>{profile.name}{profile.id === selectedId && " · 已绑定"}</strong><span>{profile.content}</span>
        </button>
        <button type="button" className="ym-button" aria-label={`查看用户设定详情：${profile.name}`} disabled={busy} onClick={() => onDetails(profile.id)}>详情</button>
      </article>)}</div>
      <CardBrowserPagination browser={browser} label="用户设定" disabled={busy}/>
    </> : <p className="ym-page-empty">暂无用户设定，请先到设置中的“用户设定”创建。</p>}
    </div>
    <div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy || !selectedId} onClick={() => onSelect(null)}>解除当前绑定</button></div>
  </dialog>;
}
