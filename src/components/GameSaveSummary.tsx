import type { PluginJson } from "../types";

type SavedObject = Record<string, PluginJson>;
function object(value: PluginJson | undefined): SavedObject | null {
  return value && typeof value === "object" && !Array.isArray(value) ? value : null;
}
const count = (value: PluginJson | undefined): number | null => typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : null;
const format = (value: number) => value.toLocaleString("zh-CN");
const difficulties: Record<string, string> = { easy:"初级", medium:"中级", hard:"高级" };
type Summary = {status:"saved"|"playing"|"completed";metrics:Array<{label:string;value:string}>};
function summary(value:PluginJson|undefined):Summary|null {
  const row=object(value);
  if(!row||row.version!==1||Object.keys(row).some(key=>!["version","status","metrics"].includes(key)))return null;
  const status=row.status===undefined?"saved":row.status,items=row.metrics===undefined?[]:row.metrics;
  if(typeof status!=="string"||!["saved","playing","completed"].includes(status)||!Array.isArray(items)||items.length>3)return null;
  const metrics:Summary["metrics"]=[];
  for(const item of items){
    const metric=object(item);
    if(!metric||Object.keys(metric).some(key=>!["label","value"].includes(key)))return null;
    const {label,value}=metric;
    if(typeof label!=="string"||typeof value!=="string"||!label.trim()||!value.trim()||Array.from(label).length>20||Array.from(value).length>40||/[\u0000-\u001f\u007f]/.test(label+value))return null;
    metrics.push({label,value});
  }
  return {status:status as Summary["status"],metrics};
}

export default function GameSaveSummary({data,pluginId}:{data:Record<string,PluginJson>;pluginId:string}) {
  if (!Object.keys(data).length) return <p className="ym-game-save-empty">还没有存档，玩一局后会自动保存。</p>;
  const firstParty = ["2048","minesweeper","my-home","rock-paper-scissors","dice","star-catcher"].some(id => pluginId === `com.yanmew.game.${id}`);
  const envelope = firstParty ? object(data["state.main"]) : null, progress = object(envelope?.progress), current = object(progress?.current);
  const pending = !!envelope?.pending;
  const classic = pluginId === "com.yanmew.game.2048" || pluginId === "com.yanmew.game.minesweeper";
  const active = classic && current?.settled === false;
  const declared=summary(data["ui.summary"]);
  const metrics: Array<{label:string;value:string}> = [];
  const addCount = (label:string,value:PluginJson|undefined,suffix="") => {
    const n=count(value);if(n!==null)metrics.push({label,value:format(n)+suffix});
  };
  if (progress) {
    if (pluginId === "com.yanmew.game.2048") {
      addCount("最高得分",progress.bestScore);
      addCount(active ? "本局最高数字" : "上局最高数字",current?.highest);
    } else if (pluginId === "com.yanmew.game.minesweeper") {
      addCount("累计通关",progress.totalWins," 次");
      if (typeof current?.level === "string" && difficulties[current.level]) metrics.push({label:active?"本局难度":"上局难度",value:difficulties[current.level]});
    } else if (pluginId === "com.yanmew.game.my-home") {
      addCount("生活技能",progress.affinity);
      const farm=object(progress.farm),barn=object(progress.barn);
      if (typeof farm?.enabled === "boolean" || typeof barn?.enabled === "boolean") metrics.push({label:"已解锁区域",value:[farm?.enabled&&"田地",barn?.enabled&&"养殖场"].filter(Boolean).join("、")||"尚未解锁"});
    }
    if (["com.yanmew.game.2048","com.yanmew.game.minesweeper","com.yanmew.game.rock-paper-scissors","com.yanmew.game.dice","com.yanmew.game.star-catcher"].includes(pluginId)) {
      const now=new Date(),today=`${now.getFullYear()}-${String(now.getMonth()+1).padStart(2,"0")}-${String(now.getDate()).padStart(2,"0")}`;
      const used=count(classic?progress.paidRounds:progress.usedToday);
      if(used!==null&&typeof progress.day==="string"&&/^\d{4}-\d{2}-\d{2}$/.test(progress.day))metrics.push({label:"今日赚钱已玩",value:`${format(progress.day===today?used:0)} 局`});
    }
  }
  const playing=declared?declared.status==="playing":active;
  const title=pending?"进度等待恢复":declared?.status==="completed"?"已通关":playing?(declared?"游戏进行中":"有未结束的棋局"):"进度已保存";
  const highlights=declared?.metrics??metrics;
  return <div className="ym-game-save-summary">
    <div className="ym-game-save-heading"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M5 3h12l4 4v14H3V3h2Zm2 0v6h10V3M7 21v-8h10v8"/></svg><div><strong>{title}</strong><p>{pending?"打开游戏后会自动核对上次结算。":playing?"打开游戏即可继续这一局。":"详细进度可在游戏内查看。"}</p></div></div>
    {highlights.length>0&&<dl className="ym-game-save-highlights">{highlights.slice(0,3).map((item,index)=><div key={index}><dt>{item.label}</dt><dd>{item.value}</dd></div>)}</dl>}
  </div>;
}
