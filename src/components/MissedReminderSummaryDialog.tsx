import type { ReminderOccurrence } from "../types";
import "./ReminderNotifications.css";

export type MissedReminderSummaryDialogProps = {
  occurrences: ReminderOccurrence[];
  busy: boolean;
  error: string;
  onDismiss: () => void;
  onOpenReminders: () => void;
};

export default function MissedReminderSummaryDialog({
  occurrences,
  busy,
  error,
  onDismiss,
  onOpenReminders,
}: MissedReminderSummaryDialogProps) {
  const names = [...new Map(occurrences.map((occurrence) => [occurrence.reminderId, occurrence.name])).values()];
  const summary = `你离开这段时间，${names.join("、")}过期了！`;

  return (
    <div className="ym-reminder-notification" role="dialog" aria-labelledby="ym-missed-reminder-title" data-pet-interactive>
      <header className="ym-reminder-notification__header">
        <svg className="ym-reminder-notification__icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
          <path d="M18 9a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4" />
        </svg>
        <h2 className="ym-reminder-notification__title" id="ym-missed-reminder-title">离线提醒汇总</h2>
      </header>
      <div className="ym-reminder-notification__body">
        <p className="ym-reminder-notification__message">{summary}</p>
        {error && <p className="ym-reminder-notification__error" role="alert">{error}</p>}
      </div>
      <footer className="ym-reminder-notification__actions">
        <button className="ym-button ym-reminder-notification__action" type="button" disabled={busy} onClick={onDismiss}>知道了</button>
        <button className="ym-button ym-reminder-notification__action" type="button" disabled={busy} onClick={onOpenReminders}>查看提醒</button>
      </footer>
    </div>
  );
}
