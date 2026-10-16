import type { ReactNode } from "react";
import "./ResponsiveWindowShell.css";

export type ResponsiveWindowShellProps = {
  title: string;
  compact?: boolean;
  subtitle: string;
  contextLabel: string;
  contextContent?: ReactNode;
  loading: boolean;
  error: string;
  onRetry: () => void;
  children: ReactNode;
  sidebar?: ReactNode;
};

export default function ResponsiveWindowShell({
  title,
  compact = false,
  subtitle,
  contextLabel,
  contextContent,
  loading,
  error,
  onRetry,
  children,
  sidebar,
}: ResponsiveWindowShellProps) {
  return (
    <div className={`ym-window-shell${compact ? " ym-window-shell--compact" : ""}`}>
      <header className="ym-window-shell__header">
        <span className="ym-window-shell__brand" aria-label="颜喵">(・ω・)</span>
        <div className="ym-window-shell__heading">
          <h1 className="ym-window-shell__title">{title}</h1>
          <p className="ym-window-shell__subtitle">{subtitle}</p>
        </div>
        <p className="ym-window-shell__context">
          <span className="ym-window-shell__context-caption">当前人设与会话</span>
          <span className="ym-window-shell__context-value">{contextContent ?? contextLabel}</span>
        </p>
      </header>

      <div className="ym-window-shell__layout">
        {sidebar != null && (
          <aside className="ym-window-shell__sidebar" aria-label="侧栏">
            {sidebar}
          </aside>
        )}
        <main className="ym-window-shell__content" aria-busy={loading && !error}>
          {error ? (
            <div className="ym-window-shell__notice ym-window-shell__notice--error" role="alert">
              <h2>内容加载失败</h2>
              <p>{error}</p>
              <button className="ym-button" type="button" onClick={onRetry}>重试</button>
            </div>
          ) : loading ? (
            <div className="ym-window-shell__notice" role="status">
              <p>正在加载，请稍候…</p>
            </div>
          ) : children}
        </main>
      </div>
    </div>
  );
}
