import { useEffect, useState, type CSSProperties } from "react";
import { listen } from "@tauri-apps/api/event";
import { configurationApi, errorMessage } from "../api";

export function usePetAppearance() {
  const [color, setColor] = useState<string>();
  const [error, setError] = useState("");
  const [ready, setReady] = useState(false);
  useEffect(() => {
    let disposed = false;
    let generation = 0;
    let mode = "system";
    const stops: (() => void)[] = [];
    const media = matchMedia("(prefers-color-scheme: dark)");
    const apply = () => { document.documentElement.dataset.theme = mode === "system" ? (media.matches ? "dark" : "light") : mode; };
    apply();
    media.addEventListener("change", apply);
    const refresh = async () => {
      const current = ++generation;
      try {
        const [settings, personas] = await Promise.all([configurationApi.getSettings(), configurationApi.listPersonas()]);
        if (disposed || current !== generation) return;
        mode = settings.theme_mode; apply();
        setColor(personas.find((p) => p.isActive)?.color);
        setError("");
      } catch (cause) { if (!disposed && current === generation) setError(errorMessage(cause)); }
      finally { if (!disposed && current === generation) setReady(true); }
    };
    void (async () => {
      for (const name of ["window_context_changed", "configuration_changed"]) {
        try {
          const stop = await listen(name, () => { void refresh(); });
          if (disposed) { stop(); return; }
          stops.push(stop);
        } catch (cause) { if (!disposed) setError(errorMessage(cause)); }
      }
      if (!disposed) await refresh();
    })();
    return () => { disposed = true; stops.forEach((stop) => stop()); media.removeEventListener("change", apply); };
  }, []);
  return { style: color ? { "--ym-pet-color": color } as CSSProperties : undefined, error, ready };
}
