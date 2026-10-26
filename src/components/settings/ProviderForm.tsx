import {useRevealForm} from "../../hooks/useRevealForm";
import { useId, useState, type FormEvent } from "react";
import type { AppError, Provider, ProviderInput, SecretUpdate } from "../../types";

type ProviderFormProps = {
  provider: (Pick<Provider, "name" | "baseUrl" | "model" | "hasApiKey"> & Partial<Pick<Provider, "maxOutputTokens" | "contextWindowOverride">>) | null;
  busy: boolean;
  error: AppError | null;
  onSave: (input: ProviderInput) => Promise<void>;
  onCancel: () => void;
  embedding?: boolean;
  connectionOnly?:boolean;
};

export default function ProviderForm({ provider, busy, error, onSave, onCancel, embedding = false, connectionOnly=false }: ProviderFormProps) {
  const id = useId();
  const form=useRevealForm();
  const [name, setName] = useState(provider?.name ?? "");
  const [baseUrl, setBaseUrl] = useState(provider?.baseUrl ?? "");
  const [model, setModel] = useState(provider?.model ?? "");
  const [newApiKey, setNewApiKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [apiKeyUpdate, setApiKeyUpdate] = useState<SecretUpdate>({ action: "keep" });
  const [maxOutputTokens, setMaxOutputTokens] = useState(String(provider?.maxOutputTokens ?? 32768));
  const [contextWindow, setContextWindow] = useState(provider?.contextWindowOverride == null ? "" : String(provider.contextWindowOverride));
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [localError, setLocalError] = useState<AppError | null>(null);

  const fieldError = (field: string) => localError?.field === field ? localError.message : error?.field === field ? error.message : undefined;
  const nameError = fieldError("name");
  const baseUrlError = fieldError("baseUrl");
  const modelError = fieldError("model");
  const apiKeyError = fieldError("apiKey");
  const maxOutputError = fieldError("maxOutputTokens");
  const contextWindowError = fieldError("contextWindowOverride");

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (busy) return;
    if (!embedding && !connectionOnly) {
      const valid = (value: string) => /^\d+$/.test(value) && Number.isSafeInteger(Number(value)) && Number(value) > 0;
      if (!valid(maxOutputTokens) || (contextWindow !== "" && !valid(contextWindow))) {
        setLocalError(!valid(maxOutputTokens)
          ? { code: "VALIDATION_ERROR", field: "maxOutputTokens", message: "最大输出必须是正整数" }
          : { code: "VALIDATION_ERROR", field: "contextWindowOverride", message: "上下文容量应留空或填写正整数" });
        setAdvancedOpen(true);
        return;
      }
    }
    setLocalError(null);
    await onSave({
      name: name.trim(),
      baseUrl: baseUrl.trim() || null,
      apiKey: apiKeyUpdate,
      ...(!connectionOnly ? {model:model.trim()} : {}),
      ...(!embedding && !connectionOnly ? { maxOutputTokens: Number(maxOutputTokens), contextWindowOverride: contextWindow === "" ? null : Number(contextWindow) } : {}),
    });
  };

  return <form ref={form} className="ym-provider-form" onSubmit={submit}>
    <fieldset disabled={busy}>
      <legend>{provider ? "编辑提供商" : "新增提供商配置"}</legend>
      <label className="ym-field" htmlFor={`${id}-name`}>
        提供商名称
        <input
          id={`${id}-name`}
          aria-label="提供商名称"
          value={name}
          required
          aria-invalid={Boolean(nameError)}
          aria-describedby={nameError ? `${id}-name-error` : undefined}
          onChange={event => setName(event.target.value)}
        />
        {nameError && <span id={`${id}-name-error`} role="alert">{nameError}</span>}
      </label>

      <label className="ym-field" htmlFor={`${id}-base-url`}>
        Base URL
        <input
          id={`${id}-base-url`}
          aria-label="Base URL"
          type="url"
          required={embedding}
          value={baseUrl}
          aria-invalid={Boolean(baseUrlError)}
          aria-describedby={`${id}-base-url-note${baseUrlError ? ` ${id}-base-url-error` : ""}`}
          onChange={event => setBaseUrl(event.target.value)}
        />
        <span id={`${id}-base-url-note`} className="ym-page-note">
          {embedding ? "填写服务的 Base URL，例如 https://api.openai.com/v1；提交时补上 /embeddings。" : "留空使用 https://api.openai.com/v1；各用途按接口类型追加路径，不填写完整调用路径。"}
        </span>
        {baseUrlError && <span id={`${id}-base-url-error`} role="alert">{baseUrlError}</span>}
      </label>

      {!connectionOnly&&<label className="ym-field" htmlFor={`${id}-model`}>
        模型名称
        <input
          id={`${id}-model`}
          aria-label="模型名称"
          value={model}
          required
          aria-invalid={Boolean(modelError)}
          aria-describedby={modelError ? `${id}-model-error` : undefined}
          onChange={event => setModel(event.target.value)}
        />
        {modelError && <span id={`${id}-model-error`} role="alert">{modelError}</span>}
      </label>}

      <div className="ym-field">
        <label htmlFor={`${id}-api-key`}>API Key</label>
        <input
          id={`${id}-api-key`}
          type={showKey ? "text" : "password"}
          autoComplete="new-password"
          value={newApiKey}
          aria-invalid={Boolean(apiKeyError)}
          aria-describedby={`${id}-api-key-status${apiKeyError ? ` ${id}-api-key-error` : ""}`}
          onChange={event => {
            const value = event.target.value;
            setNewApiKey(value);
            setApiKeyUpdate(value ? { action: "set", value } : { action: "keep" });
          }}
        />
        <button className="ym-button" type="button" disabled={!newApiKey} onClick={() => setShowKey(value => !value)}>{showKey ? "隐藏本次输入" : "查看本次输入"}</button>
        <span id={`${id}-api-key-status`} className="ym-page-note">
          {apiKeyUpdate.action === "clear"
            ? "保存后清除已保存密钥"
            : provider?.hasApiKey
              ? "已保存密钥；留空会保留"
              : "尚未保存密钥；留空不会设置"}
        </span>
        {provider?.hasApiKey && apiKeyUpdate.action !== "clear" && <button
          className="ym-button"
          type="button"
          onClick={() => {
            setNewApiKey("");
            setApiKeyUpdate({ action: "clear" });
          }}
        >清除已保存密钥</button>}
        {apiKeyUpdate.action === "clear" && <button
          className="ym-button"
          type="button"
          onClick={() => setApiKeyUpdate({ action: "keep" })}
        >撤销清除</button>}
        {apiKeyError && <span id={`${id}-api-key-error`} role="alert">{apiKeyError}</span>}
      </div>

      {!embedding && !connectionOnly && <details open={advancedOpen || Boolean(maxOutputError || contextWindowError)} onToggle={event => setAdvancedOpen(event.currentTarget.open)}>
        <summary>高级设置</summary>
        <label className="ym-field" htmlFor={`${id}-max-output`}>
          最大输出（tokens）
          <input id={`${id}-max-output`} aria-label="最大输出（tokens）" type="number" min="1" step="1" required
            value={maxOutputTokens} aria-invalid={Boolean(maxOutputError)}
            aria-describedby={`${id}-output-note${maxOutputError ? ` ${id}-max-output-error` : ""}`}
            onChange={event => { setMaxOutputTokens(event.target.value); setLocalError(null); }} />
          {maxOutputError && <span id={`${id}-max-output-error`} role="alert">{maxOutputError}</span>}
        </label>
        <label className="ym-field" htmlFor={`${id}-context-window`}>
          上下文容量（tokens）
          <input id={`${id}-context-window`} aria-label="上下文容量（tokens）" type="number" min="1" step="1"
            placeholder="自动" value={contextWindow} aria-invalid={Boolean(contextWindowError)}
            aria-describedby={`${id}-context-note${contextWindowError ? ` ${id}-context-window-error` : ""}`}
            onChange={event => { setContextWindow(event.target.value); setLocalError(null); }} />
          <span id={`${id}-context-note`} className="ym-page-note">留空自动读取模型能力；没有可靠资料时显示未知。填写正整数可手动覆盖。</span>
          {contextWindowError && <span id={`${id}-context-window-error`} role="alert">{contextWindowError}</span>}
        </label>
        <p id={`${id}-output-note`} className="ym-page-note">默认最大输出 32768 tokens，是一次生成的上限，不是目标长度。token 不等于字数，推理模型可能把思考计入额度。实际输出还会受模型能力和本次剩余上下文限制。</p>
      </details>}

      <div className="ym-inline-form">
        <button className="ym-button" type="submit">{busy ? "正在保存…" : "保存"}</button>
        <button className="ym-button" type="button" onClick={onCancel}>取消</button>
      </div>
    </fieldset>
  </form>;
}
