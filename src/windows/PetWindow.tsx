import PetAwayElapsed from "../components/PetAwayElapsed";
import {usePetReturnNotice} from "./usePetReturnNotice";
import {usePetAutoCollapse} from "./usePetAutoCollapse";
import PetMail from "../components/PetMail";
import {usePetMail} from "./usePetMail";
import RealMoneyCatchupNotice,{useRealMoneyNotice} from "../components/RealMoneyCatchupNotice";
import ValueChange from "../components/ValueChange";
import SkillUpdateConfirmation from "../components/SkillUpdateConfirmation";
import {useSkillUpdate} from "../hooks/useSkillUpdate";
import {useFoodFeeding} from "./useFoodFeeding";
import FoodPicker from "../components/FoodPicker";
import {foodImage} from "../companionApi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState, type PointerEvent, type CSSProperties } from "react";
import { getPetErrorExpression, getPetLiftExpression, petCreationApi, configurationApi, errorMessage, petApi, travelApi } from "../api";
import { usePetWindow } from "./usePetWindow";
import { usePetAppearance } from "./usePetAppearance";
import { usePetActivity } from "./usePetActivity";
import { usePetChat } from "./usePetChat";
import { PetChatBubble, PetChatInput } from "../components/PetChat";
import { usePetMemoryErrors } from "./usePetMemoryErrors";
import { usePetWander } from "./usePetWander";
import {usePetPhysics} from "./usePetPhysics";
import { usePetDesktop } from "./usePetDesktop";
import { navigation, type ApplicationPage } from "../platform/navigation";
import PetArtworkDisplay from "../components/PetArtworkDisplay";
import AccessoryLayer from "../components/AccessoryLayer";
import { AccessoryCraftPresentation } from "../components/AccessoryCraftCard";
import { useAccessoryCraft } from "../hooks/useAccessoryCraft";
import PetReminderNotifications from "../components/PetReminderNotifications";
import PetTravelBubble from "../components/PetTravelBubble";
import PetConversationBubbles from "../components/PetConversationBubbles";
import "./PetWindow.css";

const DRAG_THRESHOLD_PX = 5;
type PointerStart = { pointerId: number; x: number; y: number };

export default function PetWindow() {
  const root = useRef<HTMLDivElement>(null);
  const face = useRef<HTMLDivElement>(null);
  const seenTravelIds = useRef(new Set<string>());
  const seenProactiveMessages = useRef(new Set<string>());
  const { state, error, pending, toggleHidden, setChatOpen, setImageMode, retry, fail } = usePetWindow(root);
  const pointerStart = useRef<PointerStart | null>(null);
  const draggingStarted = useRef(false);
  const dragReleased = useRef(false);
  const [dragging, setDragging] = useState(false);
  const [pressed, setPressed] = useState(false);
  const [lifted,setLifted]=useState<{personaId:string;expression:string}|null>(null);

  const [errorPose,setErrorPose]=useState<{personaId:string;expression:string}|null>(null);
  const [startupError, setStartupError] = useState("");
  const appearance = usePetAppearance();
  const activity = usePetActivity();
  const quickChat = usePetChat(activity.view);
  const [tooltip, setTooltip] = useState<string | null>(null);
  // 显式属性变化让原生裁剪区域跟随提示出现/收起，不能只依赖CSS :hover。
  const tooltipEvents = (name: string) => ({
    "data-tooltip-open": tooltip === name,
    onPointerEnter: () => setTooltip(name), onPointerLeave: () => setTooltip(null),
    onFocus: () => setTooltip(name), onBlur: () => setTooltip(null),
  });
  const [optionsOpen, setOptionsOpen] = useState(false);
  const options = useRef<HTMLDivElement>(null);
  const accessoryContext = { personaId: activity.view?.personaId ?? "", sessionId: activity.view?.sessionId ?? null, contextRevision: activity.view?.contextRevision ?? -1 };
  const quickMail=usePetMail(accessoryContext,activity.view?.activity==="away");
  const craft = useAccessoryCraft(accessoryContext);
  const realMoneyNotice=useRealMoneyNotice();
  const skillUpdate=useSkillUpdate(accessoryContext);
  const feeding=useFoodFeeding(accessoryContext,!!activity.view?.canInteract,state.hidden,activity.refresh);
  const desktop = usePetDesktop(accessoryContext.personaId, accessoryContext.contextRevision);
  const toss=activity.view?.activity!=="away"&&desktop.toss?.personaId===accessoryContext.personaId&&desktop.toss?.sessionId===accessoryContext.sessionId&&desktop.toss?.contextRevision===accessoryContext.contextRevision?desktop.toss:null;
  usePetPhysics(root,dragging,desktop.dragVelocity,toss,!!activity.view?.canInteract&&!activity.view?.error&&!state.hidden);
  const chase=activity.view?.canWander && desktop.chase?.personaId===accessoryContext.personaId && desktop.chase?.sessionId===accessoryContext.sessionId && desktop.chase?.contextRevision===accessoryContext.contextRevision?desktop.chase:null;
  const tossStyle=toss?{"--pet-toss-x":`${toss.body.x}px`,"--pet-toss-y":`${toss.body.y}px`,"--pet-toss-width":`${toss.body.width}px`,"--pet-toss-height":`${toss.body.height}px`} as CSSProperties:{};
  const layoutDock = useRef("");
  useEffect(() => {
    if (pending || state.hidden || state.revision < 0 || desktop.revision < 0 || layoutDock.current === desktop.dock) return;
    layoutDock.current = desktop.dock;
    // 已展开输入时拖入或拖离侧边，按新停靠布局重新计算透明区域。
    if (state.chatOpen) void setChatOpen(true);
  }, [desktop.dock, desktop.revision, state.hidden, state.revision, state.chatOpen, pending]);
  const memoryErrors = usePetMemoryErrors(activity.view);
  const [menuOpen, setMenuOpen] = useState(false);
  const [elapsedOpen,setElapsedOpen]=useState(false);
  useEffect(()=>{setElapsedOpen(false);},[activity.view?.personaId,activity.view?.sessionId,activity.view?.contextRevision,activity.view?.activity,state.hidden]);
  useEffect(()=>{if(menuOpen||optionsOpen||state.chatOpen)setElapsedOpen(false);},[menuOpen,optionsOpen,state.chatOpen]);
  const returned=usePetReturnNotice(activity.view,!state.hidden&&appearance.ready&&state.revision>=0);
  const [showErrors, setShowErrors] = useState(false);
  const [collapsedError,setCollapsedError]=useState("");
  const mainError = activity.view?.error;
  const reportedErrors = [mainError ? `连接错误：${mainError}` : "", ...memoryErrors].filter(Boolean);
  const immediateError=error||startupError||appearance.error||activity.error||desktop.error;
  const allErrors=[immediateError,...reportedErrors].filter(Boolean).join("\n");
  const visibleError=allErrors&&(showErrors||(immediateError&&collapsedError!==allErrors))?allErrors:"";
  const hideErrors=()=>{setCollapsedError(allErrors);setShowErrors(false);};
  const errorActive=Boolean(allErrors);
  const wanderEnabled = desktop.fullscreenWander && desktop.dock === "none" && appearance.ready && !state.hidden && !pending
    && !toss && !feeding.held && !feeding.open && !dragging && (!pressed || chase?.phase==="waiting") && !menuOpen && !optionsOpen && !state.chatOpen && !quickChat.busy && !quickChat.reply && !errorActive && !craft.preview;
  const wanderStyle = usePetWander(root, face, activity.view, wanderEnabled);
  const dailyActivity = ["normal", "standby", "hungry", "bored"].includes(activity.view?.activity ?? "");
  const dockActivity = dailyActivity || activity.view?.activity === "dormant";
  const walking = wanderEnabled && dailyActivity && desktop.walking?.personaId === accessoryContext.personaId && desktop.walking?.sessionId === accessoryContext.sessionId && desktop.walking?.contextRevision === accessoryContext.contextRevision ? desktop.walking : null;
  useEffect(()=>{let alive=true;let sequence=0;const stops:(()=>void)[]=[];setLifted(null);const id=accessoryContext.personaId;const read=()=>{const request=++sequence;if(id)void getPetLiftExpression(id).then(expression=>{if(alive&&request===sequence)setLifted({personaId:id,expression});}).catch(()=>{});};const saveStop=(stop:()=>void)=>{if(alive)stops.push(stop);else stop();};void listen("configuration_changed",read).then(saveStop).catch(()=>{});void petCreationApi.listen(read).then(saveStop).catch(()=>{});read();return()=>{alive=false;sequence++;stops.forEach(stop=>stop());};},[accessoryContext.personaId,accessoryContext.contextRevision]);
  // 报错覆盖显示并暂停自动移动，不写入活动、会话表情或任何业务计时。
  useEffect(()=>{let alive=true;let sequence=0;const stops:(()=>void)[]=[];setErrorPose(null);const id=accessoryContext.personaId;const read=()=>{const request=++sequence;if(id)void getPetErrorExpression(id).then(expression=>{if(alive&&request===sequence)setErrorPose({personaId:id,expression});}).catch(()=>{});};const saveStop=(stop:()=>void)=>{if(alive)stops.push(stop);else stop();};void listen("configuration_changed",read).then(saveStop).catch(()=>{});void petCreationApi.listen(read).then(saveStop).catch(()=>{});read();return()=>{alive=false;sequence++;stops.forEach(stop=>stop());};},[accessoryContext.personaId,accessoryContext.contextRevision]);
  const petExpression = activity.view?.activity === "away" ? activity.view.expression : (dragging?(lifted?.personaId===accessoryContext.personaId?lifted.expression:"(｡>﹏<｡)"):null) || (errorActive?(errorPose?.personaId===accessoryContext.personaId?errorPose.expression:"(×_×)"):null) || (toss?desktop.expression:undefined) || feeding.expression || (chase && wanderEnabled && chase.phase!=="running"?desktop.expression:undefined) || ((dockActivity && desktop.dock !== "none" && desktop.dock !== "top") || walking ? desktop.expression : undefined) || activity.view?.expression || "(・ω・)";
  const hungry = activity.view?.activity === "hungry" && !feeding.held && !petExpression.includes("饿");
  const [collapseMinutes,setCollapseMinutes]=useState(3);
  useEffect(()=>{let alive=true;const read=()=>void configurationApi.getSettings().then(s=>{if(alive)setCollapseMinutes(Number(s.pet_chat_autocollapse_minutes??3));}).catch(()=>{});read();let stop:(()=>void)|undefined;void listen("configuration_changed",read).then(fn=>{if(alive)stop=fn;else fn();}).catch(()=>{});return()=>{alive=false;stop?.();};},[]);
  const collapseConditions={minutes:collapseMinutes,hidden:state.hidden,busy:quickChat.busy||quickChat.uploading||(activity.view?.activity==="away"&&quickMail.busy),hasDraft:!!quickChat.draft.trim()||quickChat.images.length>0||(activity.view?.activity==="away"&&!!quickMail.draft.trim())};
  usePetAutoCollapse(root,{...collapseConditions,visible:state.chatOpen||!!quickChat.reply,onCollapse:()=>{quickChat.dismissReply();if(state.chatOpen)void setChatOpen(false);}});
  const openPage = async (page: ApplicationPage) => {
    setOptionsOpen(false);
    try { await navigation.open(page); }
    catch (cause) { fail(errorMessage(cause)); }
  };

  const openLetter = async () => {
    const view = activity.view;
    if (!view?.sessionId) return;
    const context = { personaId:view.personaId,sessionId:view.sessionId,contextRevision:view.contextRevision };
    try {
      const trip = await travelApi.current(context);
      if (!trip) throw new Error("这次旅行已经结束，请刷新后再打开聊天。");
      await navigation.openLetter(context,trip.id);
    } catch (cause) { fail(errorMessage(cause)); }
  };

  useEffect(() => {
    let disposed = false;
    void configurationApi.status().catch((cause: unknown) => {
      if (!disposed) setStartupError(`配置存储不可用：${errorMessage(cause)}。处理后请重新启动应用。`);
    });
    return () => { disposed = true; };
  }, []);
  useEffect(() => {
    pointerStart.current = null; draggingStarted.current = false; setDragging(false); setPressed(false);
    setMenuOpen(false);
  }, [state.hidden]);
  useEffect(() => { setMenuOpen(false); setOptionsOpen(false); }, [activity.view?.contextRevision, state.hidden]);
  useEffect(() => {
    const close = (event: globalThis.PointerEvent) => { if (options.current && !options.current.contains(event.target as Node)) setOptionsOpen(false); };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, []);
  useEffect(() => {
    const close = (event: KeyboardEvent) => { if (event.key === "Escape") { setMenuOpen(false); setOptionsOpen(false); setShowErrors(false); if (state.chatOpen && !event.isComposing) void setChatOpen(false); } };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [state.chatOpen, setChatOpen]);
  useEffect(() => {
    const reset = () => {
      // 原生拖动可能使 WebView 失焦，不能把鼠标交接误判为松手。
      if (!draggingStarted.current) { pointerStart.current = null; setDragging(false); setPressed(false); }
      setOptionsOpen(false); setTooltip(null);
    };
    window.addEventListener("blur", reset);
    return () => window.removeEventListener("blur", reset);
  }, []);

  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || pointerStart.current) return;
    if(toss&&(toss.phase==="up"||toss.phase==="down")){
      event.preventDefault();setPressed(true);setMenuOpen(false);setOptionsOpen(false);
      pointerStart.current={pointerId:event.pointerId,x:event.clientX,y:event.clientY};
      startNativeDrag();return;
    }
    if(feeding.held){event.preventDefault();void feeding.feed();return;}
    setMenuOpen(false); setOptionsOpen(false);
    setPressed(true);
    pointerStart.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY };
    draggingStarted.current = false;
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };
  useEffect(() => {
    let disposed = false;
    const stop = listen("pet-drag-finished", () => {
      if (disposed) return;
      pointerStart.current = null; draggingStarted.current = false; setDragging(false); setPressed(false);
    });
    void stop.catch(fail);
    return () => { disposed = true; void stop.then(fn => fn()).catch(() => {}); };
  }, [fail]);
  const handlePointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const start = pointerStart.current;
    if (!start || start.pointerId !== event.pointerId || draggingStarted.current) return;
    if (Math.hypot(event.clientX - start.x, event.clientY - start.y) < DRAG_THRESHOLD_PX) return;
    startNativeDrag();
  };
  const startNativeDrag = () => {
    const pointer=pointerStart.current;
    draggingStarted.current = true; dragReleased.current=false; setDragging(true);
    // 松手可能早于两次IPC交接；不能在鼠标已松开后再启动系统拖动。
    void petApi.dragging(true).then(async() => {
      if(pointerStart.current!==pointer)return;
      if(dragReleased.current){
        await petApi.dragging(false);
        if(pointerStart.current===pointer){pointerStart.current=null;draggingStarted.current=false;setDragging(false);setPressed(false);}
        return;
      }
      await getCurrentWindow().startDragging();
    }).catch(async cause => {
      fail(cause); await petApi.dragging(false).catch(fail);
      pointerStart.current = null; draggingStarted.current = false; setDragging(false); setPressed(false);
    });
  };
  const resetPointer = (event: PointerEvent<HTMLDivElement>) => {
    // 系统接管拖动会取消网页指针；真正结束由 pet-drag-finished 通知。
    if (draggingStarted.current) return;
    if (pointerStart.current?.pointerId === event.pointerId) {
      pointerStart.current = null; draggingStarted.current = false; setDragging(false); setPressed(false);
    }
  };
  const releasePointer = (event: PointerEvent<HTMLDivElement>) => {
    if(pointerStart.current?.pointerId===event.pointerId&&draggingStarted.current){dragReleased.current=true;return;}
    const click = pointerStart.current?.pointerId === event.pointerId && !draggingStarted.current;
    resetPointer(event);
    if(click&&activity.view?.activity==="away"){setMenuOpen(false);setOptionsOpen(false);setElapsedOpen(value=>!value);return;}
    if (click) void activity.interact("click");
  };

  return (
    <div ref={root} data-testid="pet-window" className="pet-window" data-hidden={state.hidden}
      data-transitioning={pending} data-chat-open={Boolean(state.chatOpen)}
      data-chase={wanderEnabled?chase?.phase:undefined} data-upper-layout={desktop.upperLayout} data-dock={desktop.dock} data-toss={toss?.phase} data-toss-controls={toss?(toss.controlsBelow?"below":"above"):undefined} data-walking={walking?.direction}
      onContextMenuCapture={event => { if (feeding.held || Date.now() - feeding.cancelledAt.current < 500) { event.preventDefault(); event.stopPropagation(); feeding.cancel(); } }}
      data-dragging={dragging} data-error={errorActive} data-ready={appearance.ready} style={{...appearance.style,...wanderStyle,...tossStyle,"--dock-floor-inset":`${desktop.floorInset??0}px`} as CSSProperties}>
      <button className="ym-button pet-visibility" data-pet-interactive {...tooltipEvents("visibility")}
        data-variant={error ? "danger" : undefined}
        disabled={pending || (state.revision < 0 && !error)}
        onClick={() => { void (state.revision < 0 ? retry() : toggleHidden()); }}
        aria-label={state.revision < 0 ? (error ? "重试连接" : "正在连接…") : state.hidden ? "显示宠窗" : "隐藏宠窗"}
        aria-description={error || undefined} aria-busy={pending}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          {state.hidden ? <><path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6S2 12 2 12Z"/><circle cx="12" cy="12" r="2.5"/></>
            : <><path d="M3 3l18 18M10.6 6.1A12.8 12.8 0 0 1 12 6c6.5 0 10 6 10 6a17 17 0 0 1-3.2 3.5M6.1 6.9A18 18 0 0 0 2 12s3.5 6 10 6c1.6 0 3.1-.4 4.4-1"/></>}
        </svg>
        <span className="pet-tooltip" data-pet-interactive aria-hidden="true">{error || (pending ? "正在更新宠窗" : state.hidden ? "显示宠窗" : "隐藏宠窗")}</span>
      </button>
      {!!allErrors && !state.hidden && <button className="ym-button pet-health" data-pet-interactive {...tooltipEvents("health")}
        data-variant="danger" aria-label="查看错误" aria-expanded={Boolean(visibleError)}
        onClick={() => { if(visibleError)hideErrors();else setShowErrors(true);setMenuOpen(false); }}>!<span className="pet-tooltip" data-pet-interactive aria-hidden="true">查看错误</span></button>}
      {!state.hidden && <>
        <div className="pet-options" ref={options}>
          <button className="ym-button pet-more" data-pet-interactive {...tooltipEvents("more")} aria-label="更多操作" aria-haspopup="menu" aria-expanded={optionsOpen}
            onClick={() => { setOptionsOpen(value => !value); setMenuOpen(false); hideErrors(); }}><svg viewBox="0 0 24 24" fill="currentColor" stroke="none" aria-hidden="true"><circle cx="5" cy="12" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="19" cy="12" r="1.6"/></svg><span className="pet-tooltip" data-pet-interactive aria-hidden="true">更多操作</span></button>
          {optionsOpen && <div className="pet-options-menu" data-pet-interactive role="menu" aria-label="宠窗更多操作">
            <button className="ym-button" role="menuitem" onClick={() => { void openPage("chat"); }}>聊天详情</button>
            <button className="ym-button" role="menuitem" onClick={() => { void openPage("settings"); }}>设置</button>
            <button className="pet-wander-switch" role="switch" aria-label="全屏游走" aria-checked={desktop.fullscreenWander}
              disabled={desktop.busy || desktop.revision < 0} onClick={() => void desktop.toggle()}>
              <span>全屏游走</span><span className="pet-switch-track" aria-hidden="true"><span /></span>
            </button>
          </div>}
        </div>
        <div className="pet-chat-entry">
          <button className="ym-button pet-chat-toggle" data-pet-interactive {...tooltipEvents("chat")} data-unread={quickMail.hasUnread} aria-label={activity.view?.activity==="away"?"邮箱":"聊天"} aria-expanded={Boolean(state.chatOpen)} aria-controls="pet-quick-chat"
            disabled={pending || state.revision < 0} onClick={() => { setOptionsOpen(false); setMenuOpen(false); void setChatOpen(!state.chatOpen); }}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{activity.view?.activity==="away"?<><rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 6 9 7 9-7"/></>:<><path d="M20 11.5a8 8 0 0 1-8 8H5l-3 2v-10a9 9 0 1 1 18 0Z"/><path d="M7 10h8M7 14h5"/></>}</svg>
            <span className="pet-tooltip" data-pet-interactive aria-hidden="true">{activity.view?.activity==="away"?"邮箱":"聊天"}</span>
          </button>{quickMail.hasUnread&&<span className="pet-new-mail" data-pet-interactive>有新信件！</span>}
        </div>
        {state.chatOpen && <div id="pet-quick-chat">{activity.view?.activity==="away"?<PetMail mail={quickMail} onInbox={()=>{void openPage("mailbox");}}/>:<PetChatInput chat={quickChat} hasSession={Boolean(activity.view?.sessionId)} away={false} pending={pending}
          onDetails={() => { void openPage("chat"); }} onLetter={() => { void openLetter(); }} />}</div>}
      </>}
      {visibleError && !state.hidden && <div className="pet-error" data-pet-interactive role="alert" tabIndex={0}>
        <strong><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 6v7m0 3v2"/></svg>暂时遇到一点问题</strong>
        <p style={{ whiteSpace: "pre-line" }}>{visibleError}</p>
        <div className="pet-error-actions">
          {mainError && <button className="ym-button" disabled={activity.pending} onClick={() => { void activity.refresh(); }}>{activity.pending ? "正在刷新…" : "刷新"}</button>}
          {!!memoryErrors.length && <button className="ym-button" onClick={() => { void openPage("memory"); }}>查看记忆</button>}
          <button className="ym-button" onClick={hideErrors}>收起警示</button>
        </div>
      </div>}
      {menuOpen && !state.hidden && <div className="pet-menu" data-pet-interactive aria-label="桌宠互动">
        <p>{activity.view?.intimacy == null ? "当前未选择会话" : `亲密度 ${activity.view.intimacy} / 999${activity.view.intimacy === 999 ? " · 已达上限" : ""}`}</p>
        <div className="pet-menu-actions">
        <button className="ym-button" disabled={activity.pending || !activity.view?.canInteract}
          onClick={() => { setMenuOpen(false); void feeding.choose(); }}>投喂</button>
        <button className="ym-button" disabled={activity.pending || !activity.view?.canInteract}
          onClick={() => { setMenuOpen(false); void activity.interact("stroke"); }}>摸摸</button>
        <button className="ym-button" onClick={() => { setMenuOpen(false); void openPage("accessories"); }}>饰品</button>
        </div>
      </div>}
      {!state.hidden && feeding.open && <FoodPicker feeding={feeding}/>}
      {!state.hidden && feeding.error && <div className="pet-food-status" data-pet-interactive role="alert">{feeding.error}<button className="ym-button" onClick={feeding.cancel}>取消投喂</button></div>}
      {!state.hidden && feeding.held && <div className="pet-food-status" data-pet-interactive>拿着 {feeding.held.name} · 右键取消<button className="ym-button" disabled={feeding.busy} onClick={feeding.cancel}>取消</button></div>}
      {!state.hidden && <div ref={face} data-pet-interactive aria-label="桌宠" role="img" className="pet-face"
        data-away={activity.view?.activity === "away"} data-hungry={hungry} title={hungry ? "饿～，可以投喂一点食物" : undefined}
        onContextMenu={event => { event.preventDefault(); setElapsedOpen(false); setMenuOpen(value => !value); setShowErrors(false); }}
        onLostPointerCapture={resetPointer} onPointerCancel={resetPointer}
        onPointerDown={handlePointerDown} onPointerMove={handlePointerMove} onPointerUp={releasePointer}>
        {activity.view?.sessionId && activity.view.activity !== "away" && <AccessoryLayer key={`${accessoryContext.personaId}:${accessoryContext.sessionId}:${accessoryContext.contextRevision}`} context={accessoryContext} face={face} canvas={root} />}
        {feeding.feedback&&<div key={feeding.feedback.receipt.actionId} className="pet-food-feedback"><img src={foodImage(feeding.feedback.food.id)} alt=""/></div>}
        <div className="pet-intimacy-reward"><ValueChange key={`${accessoryContext.personaId}:${accessoryContext.sessionId}:${accessoryContext.contextRevision}`} value={activity.view?.intimacy?.toString()} unit="亲密度"/></div>
        {elapsedOpen&&!menuOpen&&!optionsOpen&&!state.chatOpen&&<PetAwayElapsed view={activity.view}/>}<PetArtworkDisplay personaId={accessoryContext.personaId} dock={toss?.phase==="landed"?"rest_sit":desktop.dock} expression={petExpression} onImageMode={setImageMode} />

        {hungry && <span className="pet-hunger-hint">饿～</span>}
      </div>}
      {chase?.phase==="waiting" && wanderEnabled && <span className="pet-petting-hint" data-pet-interactive role="status">等摸摸</span>}
      <PetReminderNotifications hidden={state.hidden} ready={appearance.ready && state.revision >= 0 && !pending}
        context={activity.view ? { personaId: activity.view.personaId, contextRevision: activity.view.contextRevision } : undefined}
        unobscured={!visibleError && !menuOpen && !optionsOpen}
        stateBubble={!dragging && (returned?<div className="pet-state-bubble" role="status" aria-label="返回提示" data-pet-interactive><p>我回来了</p></div>:realMoneyNotice.visible ? <RealMoneyCatchupNotice notice={realMoneyNotice}/> : skillUpdate.preview || skillUpdate.error ? <SkillUpdateConfirmation update={skillUpdate} pet/> : craft.preview || craft.error ? <AccessoryCraftPresentation context={accessoryContext} craft={craft} pet /> : activity.view?.activity === "away" ? <PetTravelBubble view={activity.view} seenTravelIds={seenTravelIds.current} /> : quickChat.reply ? <PetChatBubble chat={quickChat} onDetails={() => { void openPage("chat"); }} /> : <PetConversationBubbles view={activity.view} seenMessageIds={seenProactiveMessages.current} autoCollapse={{root,...collapseConditions}} />)} />
    </div>
  );
}
