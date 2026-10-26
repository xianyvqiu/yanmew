import OnboardingGuide from "../components/settings/OnboardingGuide";
import UserManualPanel from "../components/settings/UserManualPanel";
import RealMoneyPanel from "../components/settings/RealMoneyPanel";
import FoodPanel from "../components/settings/FoodPanel";
import LedgerPanel from "../components/settings/LedgerPanel";
import MotionRegion from "../components/MotionRegion";
import SettingsScope from "../components/settings/SettingsScope";
import SkillPanel from "../components/settings/SkillPanel";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { navigation, type SettingsDetailRequest } from "../platform/navigation";
import { errorMessage } from "../api";
import ResponsiveWindowShell from "../components/ResponsiveWindowShell";
import PersonasPanel from "../components/settings/PersonasPanel";
import ReminderPanel from "../components/settings/ReminderPanel";
import ProvidersPanel from "../components/settings/ProvidersPanel";
import GeneralPanel from "../components/settings/GeneralPanel";
import WeatherPanel from "../components/settings/WeatherPanel";
import PetTimingPanel from "../components/settings/PetTimingPanel";
import MemorySettingsPanel from "../components/settings/MemorySettingsPanel";
import UserProfilePanel from "../components/settings/UserProfilePanel";
import PluginManagerPanel from "../components/PluginManagerPanel";
import PetStatusPanel from "../components/settings/PetStatusPanel";
import PetCreationPanel from "../components/settings/PetCreationPanel";
import AccessoryPanel from "../components/settings/AccessoryPanel";
import TravelPanel from "../components/TravelPanel";
import FestivalPanel from "../components/settings/FestivalPanel";
import type { WorkspaceContext } from "../hooks/useCurrentContext";
import SettingsNavigation, { settingsGroups } from "../components/settings/SettingsNavigation";
import "./workspace.css";

export default function SettingsPage({ snapshot, loading, error, onRefresh }: {
  snapshot: WorkspaceContext | null; loading: boolean; error: string; onRefresh: () => void;
}) {
  const [tutorialReplay, setTutorialReplay] = useState(0);
  const [skillId,setSkillId]=useState<string>();
  const [moneyRequest,setMoneyRequest]=useState<{date:string|null}>();
  useEffect(()=>{const open=(event:Event)=>{setSkillId((event as CustomEvent<string>).detail);setPage("Skill 仓库");};window.addEventListener("ym-open-skill",open);return()=>window.removeEventListener("ym-open-skill",open);},[]);
  const [travelTab,setTravelTab]=useState<import("../components/TravelPanel").TravelTab>("history");
  const [page, setPage] = useState("通用");
  const pageRoot = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const content = pageRoot.current?.closest(".ym-window-shell__content");
    if (content) content.scrollTop = 0;
  }, [page]);
  const [detailRequest, setDetailRequest] = useState<SettingsDetailRequest>();
  const selectPage = (value: string) => { setDetailRequest(undefined); setMoneyRequest(undefined); setPage(value); };
  const [navigationError, setNavigationError] = useState("");
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    const read = async () => {
      try {
        const requested = await navigation.takeSettingsRequest();
        if (!disposed && requested) {
          if(requested.page==="travel")setTravelTab(requested.mailbox?"mailbox":"history");
          if(requested.page==="realMoney")setMoneyRequest({date:requested.date??null});
          setPage(requested.page === "travel" ? "出行" : requested.page === "realMoney" ? "现实记账" : requested.page === "persona" ? "人设" : requested.page === "userProfile" ? "用户设定" : requested.page === "accessories" ? "饰品" : "提醒");
          setDetailRequest("id" in requested ? requested : undefined);
        }
        if (!disposed) setNavigationError("");
      } catch (cause) { if (!disposed) setNavigationError(errorMessage(cause)); }
    };
    const reload = () => { if (!disposed) void read(); };
    void navigation.listenSettings(reload).then(unlisten => {
      if (disposed) { unlisten(); return; }
      stop = unlisten; reload();
    }).catch(cause => { if (!disposed) { setNavigationError(errorMessage(cause)); reload(); } });
    window.addEventListener("focus", reload);
    return () => { disposed = true; stop?.(); window.removeEventListener("focus", reload); };
  }, []);
  const independentPage = page === "人设" || page === "帮助与使用手册";
  return <><OnboardingGuide replay={tutorialReplay} onNavigate={selectPage}/><ResponsiveWindowShell title="设置" subtitle="把日常陪伴，调成你喜欢的样子。"
    contextLabel={snapshot ? `当前人设：${snapshot.persona.name} · ${snapshot.sessions.find(item => item.id === snapshot.context.sessionId)?.title ?? "尚无会话"}` : "正在读取当前人设"}
    loading={loading && !independentPage} error={snapshot || independentPage ? "" : error} onRetry={onRefresh}
    sidebar={<><label className="ym-settings-compact">设置页面<select value={page} onChange={event => selectPage(event.target.value)} aria-label="设置页面">{settingsGroups.map(group => <optgroup key={group.title} label={group.title}>{group.items.map(item => <option key={item} value={item}>{item}</option>)}</optgroup>)}<option value="帮助与使用手册">帮助与使用手册</option></select></label>
    <SettingsNavigation page={page} onSelect={selectPage} /></>}>
    {(snapshot || independentPage) && <MotionRegion motionKey={page}><div ref={pageRoot} className="ym-workspace-page"><SettingsScope page={page}/>{navigationError && <p role="alert" className="ym-page-error">设置页面跳转失败：{navigationError}，可从左侧选择目标页面。</p>}{error && <div role="alert" className="ym-page-error">{error}<button className="ym-button" onClick={onRefresh}>重新加载</button></div>}{page === "帮助与使用手册" ? <UserManualPanel onTutorial={() => setTutorialReplay(value => value + 1)} /> : page === "人设" ? <PersonasPanel detailRequest={detailRequest?.page === "persona" ? detailRequest : undefined} /> : !snapshot ? null : page === "喵币与签到" ? <PetStatusPanel snapshot={snapshot} scope="global"/> : page === "现实记账" ? <RealMoneyPanel request={moneyRequest}/> : page === "喵币账单" ? <LedgerPanel/> : page === "食物" ? <FoodPanel/> : page === "Skill 仓库" ? <SkillPanel requestedId={skillId}/> : page === "桌宠自定义创作" ? <PetCreationPanel key={snapshot.context.personaId} personaId={snapshot.context.personaId} personaName={snapshot.persona.name} /> : page === "饰品" ? <AccessoryPanel key={`${snapshot.context.personaId}:${snapshot.context.sessionId}:${snapshot.context.contextRevision}`} context={snapshot.context} /> : page === "出行" ? <TravelPanel context={snapshot.context} personaName={snapshot.persona.name} tab={travelTab} onTabChange={setTravelTab} /> : page === "提供商" ? <ProvidersPanel /> : page === "用户设定" ? <UserProfilePanel detailRequest={detailRequest?.page === "userProfile" ? detailRequest : undefined} /> : page === "提醒" ? <ReminderPanel /> : page === "时间与天气" ? <WeatherPanel key={snapshot.context.contextRevision} contextRevision={snapshot.context.contextRevision} personaId={snapshot.context.personaId} /> : page === "时间与桌宠" ? <PetTimingPanel /> : page === "记忆" ? <MemorySettingsPanel /> : page === "小游戏插件" ? <PluginManagerPanel /> : page === "日历" ? <FestivalPanel context={snapshot.context} /> : page === "桌宠状态" ? <PetStatusPanel snapshot={snapshot} scope="persona"/> : <GeneralPanel />}</div></MotionRegion>}
  </ResponsiveWindowShell></>;
}
