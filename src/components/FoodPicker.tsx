import UiIcon from "./UiIcon";
import { foodImage } from "../companionApi";
import type { useFoodFeeding } from "../windows/useFoodFeeding";
export default function FoodPicker({ feeding }: {
    feeding: ReturnType<typeof useFoodFeeding>;
}) { return <section className="pet-food-picker" data-pet-interactive aria-label="选择投喂食物"><div className="ym-page-heading"><strong>今天喂点什么？</strong><button className="ym-button" onClick={feeding.cancel}>取消</button></div><div className="pet-food-grid">{feeding.foods.map(f => <article key={f.id}><button className="pet-food-select" aria-label={`投喂 ${f.name}`} onClick={() => void feeding.take(f)}><img src={foodImage(f.id)} alt=""/><span>{f.name}</span><small>{f.unlimited ? "无限" : `${f.quantity} 份`}{f.id === feeding.prefs.favoriteFoodId ? " · 最爱" : ""}</small></button><button className="pet-food-star" aria-label={`设为默认食物：${f.name}`} aria-pressed={f.id === feeding.prefs.defaultFoodId} title="设为默认食物" onClick={() => void feeding.star(f)}>{<UiIcon name="star" filled={f.id===feeding.prefs.defaultFoodId}/>}</button></article>)}</div><p>选好后点击桌宠投喂，右键或 Esc 取消。</p></section>; }
