import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState, type RefObject } from "react";

export type PetSnapshot = { hidden: boolean; revision: number; chatOpen?: boolean };
export const newestSnapshot = (current: PetSnapshot, incoming: PetSnapshot) =>
  incoming.revision >= current.revision ? incoming : current;

export function usePetWindow(root: RefObject<HTMLDivElement>) {
  const [state, setState] = useState<PetSnapshot>({ hidden: false, revision: -1 });
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const [connectionAttempt, setConnectionAttempt] = useState(0);
  const operationPending = useRef(false);
  const mounted = useRef(false);
  const accept = useCallback((incoming: PetSnapshot) => {
    if (mounted.current) setState((current) => newestSnapshot(current, incoming));
  }, []);
  const fail = useCallback((cause: unknown) => {
    if (mounted.current) setError(`宠窗操作失败：${String(cause)}`);
  }, []);

  useEffect(() => {
    mounted.current = true;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void (async () => {
      try {
        const stop = await listen<PetSnapshot>("pet-window-state", ({ payload }) => {
          if (!disposed) accept(payload);
        });
        if (disposed) { stop(); return; }
        unlisten = stop;
        const snapshot = await invoke<PetSnapshot>("get_pet_window_state");
        if (!disposed) accept(snapshot);
      } catch (cause) { if (!disposed) fail(cause); }
    })();
    return () => { disposed = true; mounted.current = false; unlisten?.(); };
  }, [accept, fail, connectionAttempt]);

  useEffect(() => {
    const element = root.current;
    if (!element || state.revision < 0) return;
    let disposed = false;
    let frame = 0;
    let previous = "";
    let running = false;
    let dirty = false;
    const submit = async () => {
      frame = 0;
      if (disposed) return;
      if (running) { dirty = true; return; }
      const rects = Array.from(element.querySelectorAll<HTMLElement>("[data-pet-interactive]"))
        .filter((node) => getComputedStyle(node).visibility !== "hidden")
        .map((node) => node.getBoundingClientRect())
        .filter((rect) => rect.width > 0 && rect.height > 0)
        .map(({ x, y, width, height }) => ({ x, y, width, height }));
      const payload = { revision: state.revision, scale: window.devicePixelRatio, rects };
      const key = JSON.stringify(payload);
      if (key === previous) return;
      running = true;
      try {
        await invoke("update_pet_regions", payload);
        previous = key;
      } catch (cause) { if (!disposed) fail(cause); }
      finally {
        running = false;
        if (dirty && !disposed) { dirty = false; schedule(); }
      }
    };
    const schedule = () => { if (!disposed && !frame) frame = requestAnimationFrame(() => { void submit(); }); };
    const resize = new ResizeObserver(schedule);
    const observe = () => {
      resize.disconnect();
      resize.observe(element);
      element.querySelectorAll("[data-pet-interactive]").forEach((node) => resize.observe(node));
      schedule();
    };
    const mutation = new MutationObserver(observe);
    mutation.observe(element, { attributes: true, childList: true, subtree: true, characterData: true });
    observe();
    window.addEventListener("resize", schedule);
    element.addEventListener("transitionend", schedule);
    element.addEventListener("animationend", schedule);
    let dpi: MediaQueryList;
    const watchDpi = () => {
      dpi?.removeEventListener("change", watchDpi);
      dpi = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
      dpi.addEventListener("change", watchDpi);
      schedule();
    };
    watchDpi();
    document.fonts?.addEventListener("loadingdone", schedule);
    void document.fonts?.ready.then(() => { if (!disposed) schedule(); });
    return () => {
      disposed = true;
      cancelAnimationFrame(frame);
      resize.disconnect(); mutation.disconnect();
      dpi.removeEventListener("change", watchDpi);
      document.fonts?.removeEventListener("loadingdone", schedule);
      window.removeEventListener("resize", schedule);
      element.removeEventListener("transitionend", schedule);
      element.removeEventListener("animationend", schedule);
    };
  }, [root, state.revision, fail]);

  const toggleHidden = async () => {
    if (operationPending.current || state.revision < 0) return;
    operationPending.current = true;
    setPending(true); setError("");
    try { accept(await invoke<PetSnapshot>("set_pet_hidden", { hidden: !state.hidden })); }
    catch (cause) {
      fail(cause);
      try { accept(await invoke<PetSnapshot>("get_pet_window_state")); } catch (refreshError) { fail(refreshError); }
    } finally { operationPending.current = false; if (mounted.current) setPending(false); }
  };
  const setChatOpen = async (open: boolean) => {
    if (operationPending.current || state.revision < 0 || state.hidden) return;
    operationPending.current = true; setPending(true); setError("");
    try { accept(await invoke<PetSnapshot>("set_pet_chat_open", { open })); }
    catch (cause) {
      fail(cause);
      try { accept(await invoke<PetSnapshot>("get_pet_window_state")); } catch (refreshError) { fail(refreshError); }
    } finally { operationPending.current = false; if (mounted.current) setPending(false); }
  };
  const retry = () => { setError(""); setConnectionAttempt((value) => value + 1); };
  return { state, error, pending, toggleHidden, setChatOpen, retry, fail };
}
