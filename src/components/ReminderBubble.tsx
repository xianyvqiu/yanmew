import type { ReminderOccurrence } from "../types";
import "./ReminderNotifications.css";

export type ReminderBubbleProps = {
  occurrence: ReminderOccurrence;
  busy: boolean;
  error: string;
  onComplete: () => void;
  onDismiss: () => void;
  onOpenReminders: () => void;
};

function formatScheduledAt(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

export default function ReminderBubble({
  occurrence,
  busy,
  error,
  onComplete,
  onDismiss,
  onOpenReminders,
}: ReminderBubbleProps) {
  if (occurrence.status === "done") return null;

  const displayedTime = formatScheduledAt(occurrence.scheduledAt);

  return (
    <section className="ym-reminder-notification" role="region" aria-label="到点提醒" data-pet-interactive>
      <header className="ym-reminder-notification__header">
        <svg className="ym-reminder-notification__icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
          <path d="M18 9a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4" />
        </svg>
        <h2 className="ym-reminder-notification__title" title={occurrence.name}>{occurrence.name}</h2>
      </header>
      <div className="ym-reminder-notification__body">
        <p className="ym-reminder-notification__message">
          <span>{`${occurrence.name}事件提醒：${occurrence.content}`}</span>
          <time dateTime={occurrence.scheduledAt}>{displayedTime}</time>
        </p>
        {error && <p className="ym-reminder-notification__error" role="alert">{error}</p>}
      </div>
      <footer className="ym-reminder-notification__actions">
        <button className="ym-button ym-reminder-notification__action" type="button" disabled={busy} onClick={onComplete}>完成</button>
        <button className="ym-button ym-reminder-notification__action" type="button" disabled={busy} onClick={onDismiss}>收起</button>
        <button className="ym-button ym-reminder-notification__action" type="button" disabled={busy} onClick={onOpenReminders}>查看提醒</button>
      </footer>
    </section>
  );
}
