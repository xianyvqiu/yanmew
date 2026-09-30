import { useId, useState, type FormEvent } from "react";
import type { AppError, Provider, ProviderInput, SecretUpdate } from "../../types";

type ProviderFormProps = {
  provider: Provider | null;
  busy: boolean;
  error: AppError | null;
  onSave: (input: ProviderInput) => Promise<void>;
  onCancel: () => void;
};

export default function ProviderForm({ provider, busy, error, onSave, onCancel }: ProviderFormProps) {
  const id = useId();
  const [name, setName] = useState(provider?.name ?? "");
  const [baseUrl, setBaseUrl] = useState(provider?.baseUrl ?? "");
  const [model, setModel] = useState(provider?.model ?? "");
  const [newApiKey, setNewApiKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [apiKeyUpdate, setApiKeyUpdate] = useState<SecretUpdate>({ action: "keep" });

  const fieldError = (field: string) => error?.field === field ? error.message : undefined;
  const nameError = fieldError("name");
  const baseUrlError = fieldError("baseUrl");
  const modelError = fieldError("model");
  const apiKeyError = fieldError("apiKey");

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (busy) return;
    await onSave({
      name: name.trim(),
      baseUrl: baseUrl.trim() || null,
      model: model.trim(),
      apiKey: apiKeyUpdate,
    });
  };

  return <form className="ym-provider-form" onSubmit={submit}>
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
          value={baseUrl}
          aria-invalid={Boolean(baseUrlError)}
          aria-describedby={`${id}-base-url-note${baseUrlError ? ` ${id}-base-url-error` : ""}`}
          onChange={event => setBaseUrl(event.target.value)}
        />
        <span id={`${id}-base-url-note`} className="ym-page-note">
          留空使用 https://api.openai.com/v1；提交时会自动补上 /chat/completions。
        </span>
        {baseUrlError && <span id={`${id}-base-url-error`} role="alert">{baseUrlError}</span>}
      </label>

      <label className="ym-field" htmlFor={`${id}-model`}>
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
      </label>

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

      <div className="ym-inline-form">
        <button className="ym-button" type="submit">{busy ? "正在保存…" : "保存"}</button>
        <button className="ym-button" type="button" onClick={onCancel}>取消</button>
      </div>
    </fieldset>
  </form>;
}
