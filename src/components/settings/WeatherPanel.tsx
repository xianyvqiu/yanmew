import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import { configurationApi, errorMessage, weatherApi } from "../../api";
import type { AppError, SecretUpdate, WeatherInput, WeatherSettings, WeatherView } from "../../types";
import "./WeatherPanel.css";
import WeatherCard from "./WeatherCard";
import WeatherInformation from "./WeatherInformation";

type WeatherPanelProps = { contextRevision: number; personaId?:string };

const availabilityLabels: Record<WeatherView["availability"], string> = {
  available: "最新天气",
  cached: "缓存天气",
  stale: "缓存已过期",
  unavailable: "天气不可用",
};

const formatDate = (value: string) => {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
};

function toAppError(cause: unknown): AppError {
  const details = cause && typeof cause === "object" ? cause as Partial<AppError> : {};
  return { code: details.code ?? "UNKNOWN", message: errorMessage(cause), field: details.field };
}

function validWeatherHost(value: string) {
  if (!value.trim()) return true;
  try {
    const url = new URL(value.trim());
    return url.protocol === "https:" && Boolean(url.hostname) && url.pathname === "/" &&
      !url.username && !url.password && !url.search && !url.hash;
  } catch { return false; }
}

export default function WeatherPanel({ contextRevision, personaId }: WeatherPanelProps) {
  const [weather, setWeather] = useState<WeatherView | null>(null);
  const [settings, setSettings] = useState<WeatherSettings | null>(null);
  const [apiHost, setApiHost] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [keyUpdate, setKeyUpdate] = useState<SecretUpdate>({ action: "keep" });
  const [loading, setLoading] = useState(true);
  const [settingsLoading, setSettingsLoading] = useState(true);
  const [settingsReady, setSettingsReady] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [canceling, setCanceling] = useState(false);
  const [saving, setSaving] = useState(false);
  const [weatherError, setWeatherError] = useState("");
  const [settingsError, setSettingsError] = useState<AppError | null>(null);
  const [notice, setNotice] = useState("");
  const mounted = useRef(false);
  const locked = useRef(false);
  const refreshActive = useRef(false);
  const operation = useRef(0);
  const cacheGeneration = useRef(0);
  const initialSettingsLoaded = useRef(false);
  const settingsGeneration = useRef(0);

  const loadSettings = useCallback(async () => {
    const generation = ++settingsGeneration.current;
    setSettingsLoading(true);
    setSettingsError(null);
    try {
      const value = await configurationApi.getWeatherSettings();
      if (!mounted.current || generation !== settingsGeneration.current) return;
      if (!initialSettingsLoaded.current) {
        initialSettingsLoaded.current = true;
        setApiHost(value.apiHost ?? "");
      }
      setSettings(value);
      setSettingsReady(true);
    } catch (cause) {
      if (mounted.current && generation === settingsGeneration.current) setSettingsError(toAppError(cause));
    } finally {
      if (mounted.current && generation === settingsGeneration.current) setSettingsLoading(false);
    }
  }, []);

  const readCache = useCallback(async () => {
    if (locked.current) return;
    const generation = ++cacheGeneration.current;
    try {
      const result = await weatherApi.get();
      if (!mounted.current || generation !== cacheGeneration.current) return;
      if (result.contextRevision !== contextRevision) {
        setWeatherError("缓存属于其他会话上下文，已隐藏；请刷新当前天气。");
        setWeather(null);
        return;
      }
      setWeather(result);
      setWeatherError(result.error?.message ?? "");
    } catch (cause) {
      if (mounted.current && generation === cacheGeneration.current) {
        setWeatherError(`读取天气缓存失败：${errorMessage(cause)}`);
      }
    } finally {
      if (mounted.current && generation === cacheGeneration.current) setLoading(false);
    }
  }, [contextRevision]);

  const refreshWeather = useCallback(async () => {
    if (locked.current) return;
    locked.current = true;
    const token = ++operation.current;
    cacheGeneration.current++;
    refreshActive.current = true;
    setRefreshing(true);
    setWeatherError("");
    setNotice("");
    try {
      const result = await weatherApi.refresh(contextRevision);
      if (!mounted.current || token !== operation.current) return;
      if (result.contextRevision !== contextRevision) {
        setWeatherError("天气结果属于其他会话上下文，已忽略；请重新刷新。");
        return;
      }
      setWeather(result);
      setWeatherError(result.error?.message ?? "");
    } catch (cause) {
      if (mounted.current && token === operation.current) {
        setWeatherError(`刷新天气失败：${errorMessage(cause)}`);
      }
    } finally {
      if (token === operation.current) {
        cacheGeneration.current++;
        refreshActive.current = false;
        locked.current = false;
        if (mounted.current) { setRefreshing(false); setLoading(false); }
      }
    }
  }, [contextRevision]);

  useEffect(() => {
    mounted.current = true;
    operation.current++;
    // 打开页面只读缓存；启动和每小时联网刷新由后台任务负责。
    void loadSettings();

    let disposed = false;
    const refreshCache = () => { if (!disposed) void readCache(); };
    let stopWeather: (() => void) | undefined;
    void weatherApi.listen(refreshCache).then(stop => {
      if (!disposed) { stopWeather = stop; refreshCache(); } else stop();
    }).catch(refreshCache);
    window.addEventListener("focus", refreshCache);
    const timer = window.setInterval(refreshCache, 60_000);
    return () => {
      disposed = true;
      mounted.current = false;
      cacheGeneration.current++;
      operation.current++;
      settingsGeneration.current++;
      window.removeEventListener("focus", refreshCache);
      window.clearInterval(timer);
      stopWeather?.();
      if (refreshActive.current) {
        refreshActive.current = false;
        void weatherApi.cancel().catch(() => {});
      }
      locked.current = false;
    };
  }, [loadSettings, readCache, refreshWeather]);

  const cancelRefresh = async () => {
    if (!refreshActive.current || canceling) return;
    const token = ++operation.current;
    setCanceling(true);
    try {
      await weatherApi.cancel();
      if (!mounted.current || token !== operation.current) return;
      cacheGeneration.current++;
      refreshActive.current = false;
      locked.current = false;
      setRefreshing(false);
      setLoading(false);
      setNotice("已取消天气刷新；现有缓存已保留。");
    } catch (cause) {
      if (mounted.current && token === operation.current) setWeatherError(`取消天气刷新失败：${errorMessage(cause)}`);
    } finally {
      if (mounted.current && token === operation.current) setCanceling(false);
    }
  };

  const saveSettings = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!settingsReady || settingsLoading) return;
    if (locked.current || !validWeatherHost(apiHost)) {
      if (!validWeatherHost(apiHost)) setSettingsError({ code: "VALIDATION_ERROR", field: "apiHost", message: "请输入完整的 HTTPS 主机地址，不能包含接口路径、账号或查询参数。" });
      return;
    }
    locked.current = true;
    const token = ++operation.current;
    cacheGeneration.current++;
    setSaving(true);
    setSettingsError(null);
    setWeatherError("");
    setNotice("");
    const input: WeatherInput = {
      apiHost: apiHost.trim() || null,
      apiKey: keyUpdate,
    };
    try {
      const saved = await configurationApi.saveWeatherSettings(input);
      if (!mounted.current || token !== operation.current) return;
      initialSettingsLoaded.current = true;
      setSettings(saved);
      setApiHost(saved.apiHost ?? "");
      setApiKey("");
      setKeyUpdate({ action: "keep" });
      setWeather(null);
      setNotice("和风天气配置已保存。查询仍优先使用 Open-Meteo；保存密钥不代表已验证连接。");
      const generation = ++cacheGeneration.current;
      try {
        const cached = await weatherApi.get();
        if (!mounted.current || token !== operation.current || generation !== cacheGeneration.current) return;
        if (cached.contextRevision === contextRevision) {
          setWeather(cached);
          setWeatherError(cached.error?.message ?? "");
        } else {
          setWeather(null);
          setWeatherError("缓存属于其他会话上下文，已隐藏；请刷新当前天气。");
        }
      } catch (cause) {
        if (mounted.current && token === operation.current && generation === cacheGeneration.current) {
          setWeatherError(`配置已保存，但读取天气缓存失败：${errorMessage(cause)}`);
        }
      }
    } catch (cause) {
      if (mounted.current && token === operation.current) setSettingsError(toAppError(cause));
    } finally {
      if (token === operation.current) {
        locked.current = false;
        if (mounted.current) { setSaving(false); setLoading(false); }
      }
    }
  };

  const weatherBusy = saving || refreshing;
  const settingsBusy = settingsLoading || !settingsReady || saving || refreshing;
  const fieldError = settingsError?.field === "apiHost" ? settingsError.message : "";
  return <section className="ym-page-card" aria-label="时间与天气">
    <div className="ym-page-heading"><h2>时间与天气</h2></div>
    <p className="ym-page-note">启动颜喵时自动获取天气，之后每小时后台更新。这里直接显示已有结果，也可手动刷新；刷新失败会保留上次成功的天气。</p>
    {loading && <p role="status">正在读取天气缓存…</p>}
    {weatherError && <div className="ym-page-error" role="alert">{weatherError}</div>}
    {weather && <WeatherCard weather={weather} />}
    <WeatherInformation information={weather?.information} personaId={personaId} contextRevision={contextRevision} onRefresh={readCache} disabled={weatherBusy}/>
    {weather && <details><summary>天气数据详情</summary><section aria-label="天气结果">
      <p role="status">状态：{availabilityLabels[weather.availability]}</p>
      {weather.data ? <dl>
        <dt>天气</dt><dd>{weather.data.observation.description}</dd>
        <dt>温度</dt><dd>{weather.data.observation.temperatureCelsius == null ? "暂无温度数据" : `${weather.data.observation.temperatureCelsius} °C`}</dd>
        <dt>定位坐标</dt><dd>{weather.data.position.latitude}, {weather.data.position.longitude}</dd>
        <dt>天气来源</dt><dd>{weather.data.observation.source === "openMeteo" ? "Open-Meteo" : "和风天气"}</dd>
        <dt>观测时间</dt><dd>{formatDate(weather.data.observation.observedAt)}</dd>
        <dt>获取时间</dt><dd>{formatDate(weather.data.fetchedAt)}</dd>
        <dt>定位时间</dt><dd>{formatDate(weather.data.position.locatedAt)}</dd>
      </dl> : <p className="ym-page-note">当前没有可显示的天气缓存。</p>}
      {weather.availability === "stale" && <p className="ym-page-note">当前显示的是过期缓存，刷新失败时仍保留上次位置和天气。</p>}
      {weather.availability === "unavailable" && weather.error && <p className="ym-page-note">定位或天气服务当前不可用；如有旧缓存，仍会保留显示。</p>}
    </section></details>}
    <div className="ym-inline-form">
      <button className="ym-button" disabled={weatherBusy} onClick={() => { void refreshWeather(); }}>{refreshing ? "正在刷新…" : "刷新天气"}</button>
      {refreshing && <button className="ym-button" disabled={canceling} onClick={() => { void cancelRefresh(); }}>{canceling ? "正在取消…" : "取消刷新"}</button>}
      <button className="ym-button" disabled={weatherBusy} onClick={() => { void readCache(); }}>重新读取缓存</button>
    </div>
    {notice && <p role="status">{notice}</p>}
    {settingsLoading && <p role="status">正在读取天气配置…</p>}
    {settingsError && !settingsReady && <div className="ym-page-error" role="alert">读取天气配置失败：{settingsError.message}
      <button className="ym-button" disabled={settingsLoading || weatherBusy} onClick={() => { void loadSettings(); }}>重试读取配置</button>
    </div>}
    <form className="ym-weather-form" onSubmit={saveSettings}>
      <fieldset disabled={settingsBusy}>
        <legend>和风天气备用配置（可选）</legend>
        <p className="ym-page-note">Open-Meteo 优先；和风天气作为备用来源。Host 与密钥都配置后才会启用备用查询。</p>
        {settingsError && settingsReady && !fieldError && <p role="alert" className="ym-page-error">保存天气配置失败：{settingsError.message}</p>}
        <label className="ym-field">和风天气 API Host
          <input type="url" aria-label="和风天气 API Host" placeholder="https://example.qweatherapi.com" value={apiHost}
            aria-invalid={Boolean(fieldError)} onChange={event => { setApiHost(event.target.value); if (settingsError) setSettingsError(null); }} />
          {fieldError && <span role="alert">{fieldError}</span>}
        </label>
        <div className="ym-field">
          <label htmlFor="ym-weather-api-key">和风天气 API Key</label>
          <input id="ym-weather-api-key" type="password" autoComplete="new-password" aria-label="和风天气 API Key" value={apiKey}
            onChange={event => { const value = event.target.value; setApiKey(value); setKeyUpdate(value ? { action: "set", value } : { action: "keep" }); }} />
          <span className="ym-page-note">{keyUpdate.action === "clear" ? "保存后清除已保存密钥" : settings?.hasApiKey ? "密钥已保存；留空会保留" : "尚未保存密钥；留空不会设置"}</span>
          {settings?.hasApiKey && keyUpdate.action !== "clear" && <button className="ym-button" type="button" onClick={() => { setApiKey(""); setKeyUpdate({ action: "clear" }); }}>清除已保存密钥</button>}
          {keyUpdate.action === "clear" && <button className="ym-button" type="button" onClick={() => setKeyUpdate({ action: "keep" })}>撤销清除</button>}
        </div>
        <div className="ym-inline-form">
          <button className="ym-button" type="submit">{saving ? "正在保存…" : "保存天气配置"}</button>
          <span className="ym-page-note">{settings?.apiHost && settings.hasApiKey ? "备用配置已齐备" : "备用配置尚未齐备"}</span>
        </div>
      </fieldset>
    </form>
  </section>;
}
