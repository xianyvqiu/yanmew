import { useCardBrowser } from "../../hooks/useCardBrowser";
import CardBrowserControls, {CardBrowserPagination} from "../CardBrowserControls";
import { useEffect, useState } from "react";
import { companionApi, openSkill, type Food, type Preferences, type Skill } from "../../companionApi";
import { errorMessage } from "../../api";
import MotionDetails from "../MotionDetails";
export default function PersonaCompanionSettings({ personaId }: {
    personaId: string;
}) {
    const [foods, setFoods] = useState<Food[]>([]), [prefs, setPrefs] = useState<Preferences | null>(null), [skills, setSkills] = useState<Skill[]>([]), [busy, setBusy] = useState(false), [error, setError] = useState("");
    const reload = async () => { const [f, p, s] = await Promise.all([companionApi.foods(), companionApi.preferences(personaId), companionApi.skills()]); setFoods(f); setPrefs(p); setSkills(s); };
    useEffect(() => { void reload().catch(e => setError(errorMessage(e))); }, [personaId]);
    useEffect(() => {
        let alive=true;
        const stop=companionApi.listenSkills(()=>{if(alive)void reload().catch(e=>setError(errorMessage(e)));}).catch(e=>{if(alive)setError(errorMessage(e));return()=>{};});
        return()=>{alive=false;void stop.then(f=>f());};
    }, [personaId]);
    const run = async (fn: () => Promise<unknown>) => { if (busy)
        return; setBusy(true); setError(""); try {
        await fn();
        await reload().catch(e => setError(`操作已成功，暂时无法刷新绑定：${errorMessage(e)}`));
    }
    catch (e) {
        setError(errorMessage(e));
    }
    finally {
        setBusy(false);
    } };
    const browser = useCardBrowser(skills, s => `${s.title} ${s.summary} ${s.body}`, s => s.updatedAt, s => s.id, 12, s => Number(s.personas.some(p => p.id === personaId)));
    return <>
      {error && <p className="ym-page-error" role="alert">{error}</p>}
      <section className="ym-page-card"><h3>食物喜好</h3>{prefs && <label className="ym-field">喜欢的食物<select value={prefs.favoriteFoodId ?? ""} disabled={busy} onChange={e => void run(() => companionApi.savePreferences(personaId, { ...prefs, favoriteFoodId: e.target.value || null }, "favorite"))}><option value="">暂无喜好</option>{foods.filter(f => !f.unlimited).map(f => <option key={f.id} value={f.id}>{f.name}</option>)}</select></label>}<p className="ym-page-note">喜欢的食物有 10% 概率额外增加 1 点亲密度；星标默认投喂可在投喂弹窗选择。</p></section>
      <MotionDetails className="ym-feature-module"><summary><span>Skill</span><small>绑定资料与使用说明</small></summary>
        <p className="ym-page-note">绑定标题为 <code>SKILL_BASE</code>、<code>AGENTS</code>、<code>SKILL_BASE.md</code> 或 <code>AGENTS.md</code> 的 Skill 是 AI 必读基础规则（区分大小写）；其他绑定项按需检索、读取正文。仅改成此标题但未绑定不会生效。已绑定项优先显示。</p>
        <CardBrowserControls browser={browser} label="人设Skill" disabled={busy}/>
        <div className="ym-object-grid">{browser.items.map(s => <article className="ym-page-card" key={s.id}><h4>{s.title}</h4><p className="ym-card-summary">{s.body.slice(0,160) || s.summary || "暂无内容"}</p><div className="ym-inline-form"><button type="button" className="ym-button" disabled={busy} aria-label={`查看 Skill 详情：${s.title}`} onClick={() => openSkill(s.id)}>详情</button><button type="button" className="ym-button" disabled={busy} onClick={() => void run(() => companionApi.bind(s.id, personaId, !s.personas.some(p => p.id === personaId)))}>{s.personas.some(p => p.id === personaId) ? "已绑定 · 解除" : "绑定"}</button></div></article>)}</div>
        {!skills.length && <p className="ym-page-note">可在 Skill 仓库新建资料。</p>}
        <CardBrowserPagination browser={browser} label="人设Skill" disabled={busy}/>
      </MotionDetails>
    </>;
}
