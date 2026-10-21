import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { configurationApi, petCreationApi, errorMessage } from "../api";
export type Dock = "none" | "peek_left" | "peek_right" | "rest_sit" | "rest_lie";
type Walking = { direction: "left" | "right"; personaId: string; sessionId: string; contextRevision: number };
type Snapshot = { fullscreenWander: boolean; dock: Dock; revision: number; walking?: Walking | null };
export function usePetDesktop(personaId: string, contextRevision: number) {
  const [state, setState] = useState<Snapshot>({ fullscreenWander: false, dock: "none", revision: -1 });
  const [expression, setExpression] = useState<string>(), [error, setError] = useState(""), [busy, setBusy] = useState(false);
  const lock = useRef(false), sequence = useRef(0);
  useEffect(() => {
    let disposed = false;
    const accept = (next: Snapshot) => { if (!disposed) setState(old => next.revision >= old.revision ? next : old); };
    const stop = listen<Snapshot>("pet-desktop-state", e => accept(e.payload));
    void (async () => { await stop; accept(await invoke<Snapshot>("get_pet_desktop_state")); })().catch(e => { if (!disposed) setError(errorMessage(e)); });
    return () => { disposed = true; void stop.then(f => f()).catch(() => {}); };
  }, []);
  const walking = state.walking?.personaId === personaId && state.walking.contextRevision === contextRevision ? state.walking : null;
  useEffect(() => {
    sequence.current++; setExpression(undefined);
    if (!personaId || (!walking && state.dock === "none")) return;
    let alive = true;
    const refresh = () => { if (!alive) return; const request = ++sequence.current; void invoke<string>(walking ? "get_pet_walk_expression" : "get_pet_dock_expression", walking ? { personaId, direction: walking.direction } : { personaId, dock: state.dock }).then(value => {
      if (alive && sequence.current === request) { setExpression(value); setError(""); }
    }).catch(e => { if (alive && sequence.current === request) setError(errorMessage(e)); }); };
    refresh(); const stop = petCreationApi.listen(refresh).catch(() => () => {});
    const stopConfiguration = configurationApi.listenChanges(refresh).catch(() => () => {});
    return () => { alive = false; sequence.current++; void stop.then(f => f()); void stopConfiguration.then(f => f()); };
  }, [personaId, contextRevision, state.dock, walking?.direction]);
  const toggle = async () => {
    if (lock.current || state.revision < 0) return;
    lock.current = true; setBusy(true); setError("");
    try { const next = await invoke<Snapshot>("set_pet_fullscreen_wander", { enabled: !state.fullscreenWander }); setState(old => next.revision >= old.revision ? next : old); }
    catch (e) { setError(errorMessage(e)); }
    finally { lock.current = false; setBusy(false); }
  };
  return { ...state, expression, error, busy, toggle };
}
