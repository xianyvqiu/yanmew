import { useCallback, useEffect, useRef, useState } from "react";
import { chatApi, configurationApi, errorMessage, festivalsApi, petApi, walletApi } from "../../api";
import type { WorkspaceContext } from "../../hooks/useCurrentContext";
import type { SessionIntimacy, SignInStatus } from "../../types";
import "./PetStatusPanel.css";

export default function PetStatusPanel({ snapshot }: { snapshot: WorkspaceContext }) {
  const { personaId, sessionId, contextRevision } = snapshot.context;
  const [status, setStatus] = useState<SignInStatus | null>(null);
  const [intimacy, setIntimacy] = useState<SessionIntimacy | null>(null);
  const [readError, setReadError] = useState("");
  const [intimacyError, setIntimacyError] = useState("");
  const [actionError, setActionError] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<{ date: string; text: string } | null>(null);
  const mounted = useRef(false);
  const locked = useRef(false);
  const refreshPending = useRef(false);
  const walletGeneration = useRef(0);
  const contextKey = `${personaId}:${sessionId}:${contextRevision}`;
  const currentKey = useRef(contextKey); currentKey.current = contextKey;

  const readStatus = useCallback(async () => {
    if (locked.current) { refreshPending.current = true; return; }
    const generation = ++walletGeneration.current;
    try {
      const value = await walletApi.signInStatus();
      if (!mounted.current || generation !== walletGeneration.current) return;
      setStatus(value); setReadError("");
    } catch (cause) {
      if (mounted.current && generation === walletGeneration.current) setReadError(errorMessage(cause));
    }
  }, [personaId, sessionId, contextRevision]);

  useEffect(() => {
    mounted.current = true; setNotice(null); setStatus(null);
    let disposed = false;
    const stops: (() => void)[] = [];
    const reload = () => { if (!disposed) void readStatus(); };
    const subscribe = (promise: Promise<() => void>) => {
      void promise.then(stop => { if (disposed) stop(); else stops.push(stop); })
        .catch(cause => { if (!disposed) setReadError(`变更通知连接失败：${errorMessage(cause)}。恢复窗口时会重新读取。`); });
    };
    subscribe(walletApi.listen(reload));
    subscribe(configurationApi.listenChanges(reload));
    subscribe(festivalsApi.listen(reload));
    reload();
    window.addEventListener("focus", reload);
    // 本地时钟只安排只读刷新。至多每分钟重算一次，兼顾跨日、改时区和时钟调整。
    let timer: ReturnType<typeof setTimeout>;
    const schedule = () => {
      const next = new Date(); next.setHours(24, 0, 0, 0);
      timer = setTimeout(() => { reload(); if (!disposed) schedule(); },
        Math.max(250, Math.min(60_000, next.getTime() - Date.now() + 100)));
    };
    schedule();
    return () => {
      disposed = true; mounted.current = false; walletGeneration.current++;
      stops.forEach(stop => stop()); clearTimeout(timer); window.removeEventListener("focus", reload);
    };
  }, [readStatus]);

  useEffect(() => {
    let disposed = false;
    let generation = 0;
    const stops: (() => void)[] = [];
    setIntimacy(null); setIntimacyError("");
    const read = async () => {
      if (!sessionId || disposed) return;
      const request = ++generation;
      try {
        const value = await walletApi.intimacy({ personaId, sessionId, contextRevision });
        if (disposed || generation !== request) return;
        if (value.personaId !== personaId || value.sessionId !== sessionId || value.contextRevision !== contextRevision) return;
        setIntimacy(value); setIntimacyError("");
      } catch (cause) {
        if (!disposed && generation === request) setIntimacyError(errorMessage(cause));
      }
    };
    const reload = () => { void read(); };
    const subscribe = (promise: Promise<() => void>) => {
      void promise.then(stop => { if (disposed) stop(); else stops.push(stop); })
        .catch(cause => { if (!disposed) setIntimacyError(`变更通知连接失败：${errorMessage(cause)}`); });
    };
    if (sessionId) {
      subscribe(petApi.listen(view => { if (view.sessionId === sessionId) reload(); }));
      subscribe(chatApi.listenMessages(change => { if (change.sessionId === sessionId) reload(); }));
      subscribe(chatApi.listen(event => {
        if (event.sessionId === sessionId && event.type === "finished") reload();
      }));
    }
    reload(); window.addEventListener("focus", reload);
    return () => { disposed = true; stops.forEach(stop => stop()); window.removeEventListener("focus", reload); };
  }, [personaId, sessionId, contextRevision]);

  const signIn = async () => {
    if (locked.current || !status || status.signedIn) return;
    const startedKey = currentKey.current;
    locked.current = true; walletGeneration.current++;
    setBusy(true); setActionError("");
    try {
      const result = await walletApi.signIn();
      if (!mounted.current) return;
      if (startedKey !== currentKey.current) { refreshPending.current = true; return; }
      setStatus(result.status); setReadError("");
      setNotice({ date: result.status.localDate, text: result.grantedCoins === null
        ? "今天已经签到过了。" : `签到成功，获得 ${result.grantedCoins} 喵币。` });
    } catch (cause) { if (mounted.current) setActionError(errorMessage(cause)); }
    finally {
      locked.current = false;
      if (mounted.current) {
        setBusy(false);
        if (refreshPending.current) { refreshPending.current = false; void readStatus(); }
      }
    }
  };

  const currentIntimacy = intimacy?.personaId === personaId && intimacy.sessionId === sessionId
    && intimacy.contextRevision === contextRevision ? intimacy : null;
  const nextTier = currentIntimacy && status
    ? status.intimacyThresholds.findIndex(value => value > currentIntimacy.intimacy) : -1;
  const title = snapshot.sessions.find(session => session.id === sessionId)?.title ?? "当前会话";
  return <div className="ym-pet-status">
    <section className="ym-page-card" aria-labelledby="pet-intimacy-heading">
      <div className="ym-page-heading"><div><h2 id="pet-intimacy-heading">亲密度</h2>
        <p>{snapshot.persona.name} · {sessionId ? title : "尚无会话"}</p></div></div>
      {!sessionId ? <p className="ym-page-note">创建会话后，可以在这里查看你们的亲密度。</p>
        : currentIntimacy ? <>
          <p className="ym-pet-status-value" aria-label="当前亲密度">{currentIntimacy.intimacy}<span> / {currentIntimacy.maximum}</span></p>
          <progress aria-label="亲密度进度" value={currentIntimacy.intimacy} max={currentIntimacy.maximum} />
          <p className="ym-page-note">{currentIntimacy.intimacy === currentIntimacy.maximum ? "已达上限" : "在家时点击、摸摸、喂食或完成一轮聊天，亲密度增加 1。"}</p>
          {status && <p className="ym-page-note">{nextTier < 0 ? "所有亲密度档位已解锁。"
            : `下一解锁档位：第 ${nextTier + 1} 档 · ${status.intimacyThresholds[nextTier]} 亲密度（还差 ${status.intimacyThresholds[nextTier] - currentIntimacy.intimacy}）。`}</p>}
        </> : !intimacyError && <p role="status">正在读取亲密度…</p>}
      {intimacyError && <p role="alert" className="ym-page-error">{intimacyError}。请重新聚焦窗口重试。</p>}
    </section>
    <section className="ym-page-card" aria-labelledby="pet-wallet-heading">
      <div className="ym-page-heading"><div><h2 id="pet-wallet-heading">喵币</h2><p>全局共享，不随人设或会话切换。</p></div></div>
      <p className="ym-pet-status-value" aria-label="喵币余额">{status?.wallet.coins ?? "—"}<span> 喵币</span></p>
    </section>
    <section className="ym-page-card ym-pet-sign-in" aria-labelledby="pet-sign-in-heading">
      <div className="ym-page-heading"><div><h2 id="pet-sign-in-heading">每日签到</h2><p>每天领一次，按电脑当前时区计算。</p></div>
        <button className="ym-button" disabled={!status || status.signedIn || busy} onClick={() => { void signIn(); }}>
          {busy ? "正在签到…" : status?.signedIn ? "今天已签到" : "今日签到"}</button></div>
      {status ? <><p className="ym-page-note">{status.localDate} · {status.signedIn ? "已签到" : "尚未签到"}</p>
        <p className="ym-page-note">普通签到奖励：{status.rewardMin === status.rewardMax ? status.rewardMin : `${status.rewardMin}–${status.rewardMax}`} 喵币。</p>
        {status.festivals.length > 0 && <div className="ym-pet-festival-blessings" aria-label="今日日期祝福">
          <p className="ym-page-note">今日命中：{status.festivals.map(item => item.name).join("、")} · 签到奖励固定 10 喵币，仅领取一次。</p>
          {status.festivals.map(item => <p key={`${item.scope}:${item.id}`} className="ym-festival-text">{item.name}：{item.blessing}</p>)}
        </div>}</>
        : !readError && <p role="status">正在读取签到状态…</p>}
      {readError && <div role="alert" className="ym-page-error">{readError}<button className="ym-button" disabled={busy} onClick={() => { void readStatus(); }}>重新读取</button></div>}
      {actionError && <p role="alert" className="ym-page-error">{actionError}</p>}
      {notice && notice.date === status?.localDate && <p role="status" className="ym-pet-status-notice">{notice.text}</p>}
    </section>
  </div>;
}
