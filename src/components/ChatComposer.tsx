import type { FormEvent, KeyboardEvent } from "react";
import "./ChatComposer.css";

type ChatComposerProps = {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  onCancel: () => void;
  disabled: boolean;
  busy: boolean;
  cancelling: boolean;
};

export default function ChatComposer({
  value,
  onChange,
  onSend,
  onCancel,
  disabled,
  busy,
  cancelling,
}: ChatComposerProps) {
  const canSend = !disabled && !busy && Boolean(value.trim());

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (canSend) onSend();
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter" || event.shiftKey) return;
    if (event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return;

    event.preventDefault();
    if (canSend) onSend();
  };

  return (
    <form className="chat-composer" onSubmit={submit}>
      <label className="chat-composer__label" htmlFor="chat-composer-message">
        消息
      </label>
      <textarea
        id="chat-composer-message"
        className="chat-composer__input"
        aria-label="聊天消息"
        placeholder="想和我聊些什么？"
        value={value}
        onChange={event => onChange(event.currentTarget.value)}
        onKeyDown={handleKeyDown}
        disabled={disabled}
        rows={3}
      />
      <div className="chat-composer__actions">
        {busy ? (
          <button
            className="ym-button chat-composer__submit"
            type="button"
            onClick={onCancel}
            disabled={cancelling}
          >
            {cancelling ? "正在暂停…" : "暂停回复"}
          </button>
        ) : (
          <button
            className="ym-button chat-composer__submit"
            type="submit"
            disabled={!canSend}
          >
            发送
          </button>
        )}
      </div>
    </form>
  );
}
