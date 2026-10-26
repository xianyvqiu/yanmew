import { useCallback, useEffect, useRef, useState } from "react";
import { companionApi, foodImage, type Food, type FoodReceipt } from "../../companionApi";
import { errorMessage, walletApi } from "../../api";
import { useCardBrowser } from "../../hooks/useCardBrowser";
import CardBrowserControls from "../CardBrowserControls";
import "./CompanionPanels.css";
export default function FoodPanel() {
    const [items, setItems] = useState<Food[]>([]), [tab, setTab] = useState<"inventory" | "shop">("inventory"), [error, setError] = useState(""), [selected, setSelected] = useState<Food | null>(null), [quantity, setQuantity] = useState(1), [busy, setBusy] = useState(false);
    const [coins, setCoins] = useState<string>();
    const [reward, setReward] = useState<{ id: string; text: string } | null>(null);
    const [success, setSuccess] = useState<{name:string;quantity:number;coinDelta:number}|null>(null);
    const operation = useRef<{
        id: string;
        food: string;
        quantity: number;
        quote: number;
    } | null>(null);
    const lock = useRef(false);
    const dialog=useRef<HTMLDialogElement>(null);
    const closing=useRef(false),alive=useRef(true),exitAnimation=useRef<Animation>();
    useEffect(()=>{alive.current=true;return()=>{alive.current=false;exitAnimation.current?.cancel();};},[]);
    const closeDialog=useCallback(()=>{
        if(closing.current)return;closing.current=true;
        const finish=()=>{if(!alive.current)return;setSelected(null);setSuccess(null);closing.current=false;};
        const animation=window.matchMedia?.("(prefers-reduced-motion: reduce)").matches?undefined:dialog.current?.animate?.([{opacity:1,transform:"translateY(0)"},{opacity:0,transform:"translateY(6px) scale(.98)"}],{duration:160,easing:"cubic-bezier(.22,1,.36,1)"});
        exitAnimation.current=animation;if(animation)void animation.finished.then(finish,finish);else finish();
    },[]);
    useEffect(()=>{if(!success)return;const timer=window.setTimeout(closeDialog,1100);return()=>window.clearTimeout(timer);},[success,closeDialog]);
    useEffect(()=>{if(selected){if(dialog.current?.showModal)dialog.current.showModal();else dialog.current?.setAttribute("open","");}},[selected]);
    const dismiss=()=>{if(busy||operation.current)return;closeDialog();};
    const readGeneration=useRef(0);
    const reload = useCallback(async () => {
        const token=++readGeneration.current;
        try {
            const [food,wallet]=await Promise.all([companionApi.foods(),walletApi.get()]);
            if(alive.current&&token===readGeneration.current){setItems(food);setCoins(wallet.coins);}
        } catch(cause) { if(alive.current&&token===readGeneration.current)throw cause; }
    },[]);
    useEffect(() => {
        let disposed=false;const stops:Array<()=>void>=[];
        const refresh=()=>{if(!disposed)void reload().catch(e=>{if(!disposed)setError(errorMessage(e));});};
        const subscribe=async(listen:(receive:()=>void)=>Promise<()=>void>)=>{
            try{const stop=await listen(refresh);if(disposed)stop();else stops.push(stop);}
            catch(cause){if(!disposed)setError(`通知暂不可用：${errorMessage(cause)}`);}
        };
        void Promise.all([subscribe(companionApi.listenFoods),subscribe(walletApi.listen)]).then(refresh);
        window.addEventListener("focus",refresh);
        return()=>{disposed=true;readGeneration.current++;stops.forEach(stop=>stop());window.removeEventListener("focus",refresh);};
    },[reload]);
    const browser = useCardBrowser(items.filter(f => tab === "shop" || f.unlimited || f.quantity > 0), f => f.name, f => tab === "shop" ? f.createdAt : f.updatedAt, f => f.id);
    const accept = async (receipt: FoodReceipt) => {
        operation.current = null;
        setSuccess({name:selected!.name,quantity:receipt.quantity??quantity,coinDelta:receipt.coinDelta});
        setReward({ id: receipt.actionId, text: `${receipt.coinDelta} 喵币` });
        await reload().catch(e => setError(`购买已成功，暂时无法刷新库存：${errorMessage(e)}`));
    };
    const buy = async () => {
        if (!selected || lock.current || success || closing.current)
            return;
        lock.current = true;
        setBusy(true);
        setError("");
        const request = operation.current ?? { id: crypto.randomUUID(), food: selected.id, quantity, quote: selected.revision };
        operation.current = request;
        try {
            const r = await companionApi.buy(request.id, request.food, request.quantity, request.quote);
            await accept(r);
        }
        catch (e) {
            try {
                const r = await companionApi.receipt(request.id);
                if (r) {
                    await accept(r);
                }
                else {
                    operation.current = null;
                    setError(errorMessage(e));
                }
            }
            catch {
                setError("暂时无法确认购买结果，请保持当前数量并重试，系统会核对同一笔操作。");
            }
        }
        finally {
            lock.current = false;
            setBusy(false);
        }
    };
    return <section className="ym-page-card" aria-label="食物仓库与商城"><div className="ym-page-heading"><h2>食物</h2><span className="ym-page-note">余额 {coins ?? "读取中…"} 喵币</span><div className="ym-inline-form">{(["inventory", "shop"] as const).map(t => <button className="ym-button" data-variant={tab === t ? "primary" : undefined} key={t} onClick={() => { setTab(t); setSelected(null); }} disabled={busy || !!operation.current || !!success}>{t === "inventory" ? "仓库" : "商城"}</button>)}</div></div>
 <p className="ym-page-note">Token 小点心免费无限；其他食物 5 喵币／份。喜好食物有 10% 概率额外增加 1 点亲密度。</p>
 {error && !selected&&<p role="alert" className="ym-page-error">{error}</p>}{reward && <p role="status" className="ym-reward-flash" key={reward.id} onAnimationEnd={() => setReward(null)}>{reward.text}</p>}
 <CardBrowserControls browser={browser} label="食物" timeLabel={tab === "shop" ? "上架时间" : "库存变化时间"}/>
 <div className="ym-companion-grid">{browser.items.map(f => <button className="ym-food-card ym-object-card" key={f.id} onClick={() => { setSelected(f); setQuantity(1); setError(""); }} disabled={busy || !!operation.current || !!success}><img src={foodImage(f.id)} alt=""/><strong>{f.name}</strong><span>{f.unlimited ? "免费 · 无限" : tab === "shop" ? `${f.price} 喵币／份 · 拥有 ${f.quantity}` : `拥有 ${f.quantity} 份`}</span></button>)}</div>
 {selected && <dialog ref={dialog} className="ym-session-dialog ym-food-dialog" aria-label={tab==="shop"&&!selected.unlimited?`购买${selected.name}`:`${selected.name}详情`} onCancel={event=>{event.preventDefault();dismiss();}}><div className="ym-food-dialog-heading"><img src={foodImage(selected.id)} alt=""/><div><p className="ym-page-note">{tab==="shop"?"食物商城":"食物仓库"}</p><h3>{selected.name}</h3><p>{selected.unlimited ? "这是免费无限的基础食物，直接在宠窗投喂即可。" : `库存 ${items.find(f => f.id === selected.id)?.quantity ?? 0} 份 · 单价 ${selected.price} 喵币`}</p></div></div>{error&&<p role="alert" className="ym-page-error">{error}</p>}
 {success?<div className="ym-food-purchase-success" role="status"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="m7 12 3 3 7-7"/></svg><strong>购买成功</strong><p>{success.name} × {success.quantity} 已加入仓库</p><small>本次消费 {Math.abs(success.coinDelta)} 喵币</small></div>:<>
 {!selected.unlimited && tab === "shop" && <div className="ym-food-order"><label className="ym-field">购买数量<input type="number" min="1" step="1" value={quantity} disabled={busy || !!operation.current} onChange={e => setQuantity(Number(e.target.value))}/></label><p className="ym-food-order-total">共 {selected.price * quantity} 喵币</p></div>}
 <div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy || !!operation.current} onClick={dismiss}>{tab==="shop"?"取消":"关闭"}</button>
 {!selected.unlimited && tab === "shop" && <button className="ym-button" data-variant="primary" disabled={busy || !Number.isSafeInteger(quantity) || quantity <= 0 || !Number.isSafeInteger(quantity * selected.price)} onClick={() => void buy()}>{busy ? "正在购买…" : operation.current ? "核对并重试原购买" : "确认购买"}</button>}</div></>}</dialog>}
 </section>;
}
