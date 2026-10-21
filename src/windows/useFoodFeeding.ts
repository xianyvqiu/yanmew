import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { companionApi, type Food, type Preferences, type FoodReceipt } from "../companionApi";
import { errorMessage } from "../api";
type Context = {
    personaId: string;
    sessionId: string | null;
    contextRevision: number;
};
export function useFoodFeeding(context: Context, allowed: boolean, hidden: boolean, onRefresh: () => Promise<unknown>) {
    const [open, setOpen] = useState(false), [foods, setFoods] = useState<Food[]>([]), [prefs, setPrefs] = useState<Preferences>({ defaultFoodId: "token", favoriteFoodId: null }), [held, setHeld] = useState<Food | null>(null), [busy, setBusy] = useState(false), [error, setError] = useState(""), [feedback, setFeedback] = useState<{
        receipt: FoodReceipt;
        food: Food;
    } | null>(null), [expression, setExpression] = useState<string>();
    const generation = useRef(0);
    const cancelledAt = useRef(0);
    const lock = useRef(false), action = useRef<string | null>(null), scope = useRef(context), timer = useRef<ReturnType<typeof setTimeout>>();
    const cancel = () => { generation.current++; setOpen(false); if (lock.current) {
        setHeld(null);
        setExpression(undefined);
        void invoke("set_food_cursor", { foodId: null }).catch(() => { });
        return;
    } setHeld(null); setExpression(undefined); action.current = null; void invoke("set_food_cursor", { foodId: null }).catch(() => { }); };
    useEffect(() => { scope.current = context; cancel(); setFeedback(null); setError(""); }, [context.personaId, context.sessionId, context.contextRevision]);
    useEffect(() => { if (hidden || !allowed)
        cancel(); }, [hidden, allowed]);
    useEffect(() => { const stop = listen("food-cursor-cancelled", () => { cancelledAt.current = Date.now(); generation.current++; setHeld(null); setExpression(undefined); action.current = null; }).catch(() => () => { }); return () => { generation.current++; void stop.then(f => f()); if (timer.current)
        clearTimeout(timer.current); void invoke("set_food_cursor", { foodId: null }).catch(() => { }); }; }, []);
    const choose = async () => { if (lock.current)
        return; const g = ++generation.current; setError(""); try {
        const [f, p] = await Promise.all([companionApi.foods(), companionApi.preferences(context.personaId)]);
        if (g !== generation.current)
            return;
        setFoods(f.filter(x => x.unlimited || x.quantity > 0));
        setPrefs(p);
        setOpen(true);
    }
    catch (e) {
        setError(errorMessage(e));
    } };
    const take = async (f: Food) => { if (lock.current)
        return; lock.current = true; const g = ++generation.current; setError(""); try {
        await invoke("set_food_cursor", { foodId: f.id });
        if (g !== generation.current) {
            void invoke("set_food_cursor", { foodId: null }).catch(() => { });
            return;
        }
        setHeld(f);
        action.current = crypto.randomUUID();
        const face = await invoke<string>("get_food_expression", { personaId: context.personaId, favorite: f.id === prefs.favoriteFoodId });
        if (g !== generation.current)
            return;
        setExpression(face);
        setOpen(false);
    }
    catch (e) {
        setHeld(null);
        setExpression(undefined);
        void invoke("set_food_cursor", { foodId: null }).catch(() => { });
        setError(errorMessage(e));
    }
    finally {
        lock.current = false;
    } };
    const feed = async () => {
        if (!held || !context.sessionId || lock.current)
            return;
        lock.current = true;
        setBusy(true);
        setError("");
        const requestGeneration = generation.current;
        const c = { ...context, sessionId: context.sessionId }, f = held, id = action.current ?? crypto.randomUUID();
        action.current = id;
        const accept = async (r: FoodReceipt) => { await invoke("set_food_cursor", { foodId: null }).catch(() => { }); if (generation.current === requestGeneration && scope.current.personaId === c.personaId && scope.current.sessionId === c.sessionId && scope.current.contextRevision === c.contextRevision) {
            setHeld(null);
            setExpression(undefined);
            setFeedback({ receipt: r, food: f });
            timer.current = setTimeout(() => setFeedback(null), 2400);
            await onRefresh().catch(() => { });
        } action.current = null; };
        try {
            await accept(await companionApi.feed(id, f.id, c));
        }
        catch (e) {
            try {
                const r = await companionApi.receipt(id);
                if (r)
                    await accept(r);
                else {
                    setError(errorMessage(e));
                    setHeld(null);
                    setExpression(undefined);
                    action.current = null;
                    void invoke("set_food_cursor", { foodId: null }).catch(() => { });
                }
            }
            catch {
                setError("投喂结果暂未确认，请点击桌宠重试同一次操作，不会重复消耗。");
            }
        }
        finally {
            lock.current = false;
            setBusy(false);
            if (scope.current.personaId !== c.personaId || scope.current.sessionId !== c.sessionId) {
                setHeld(null);
                setExpression(undefined);
                void invoke("set_food_cursor", { foodId: null }).catch(() => { });
            }
        }
    };
    const star = async (f: Food) => { setError(""); try {
        const p = { ...prefs, defaultFoodId: f.id };
        await companionApi.savePreferences(context.personaId, p, "default");
        setPrefs(p);
    }
    catch (e) {
        setError(errorMessage(e));
    } };
    return { open, foods: [...foods].sort((a, b) => Number(b.id === prefs.defaultFoodId) - Number(a.id === prefs.defaultFoodId)), prefs, held, busy, error, feedback, expression, choose, take, feed, star, cancel, cancelledAt };
}
