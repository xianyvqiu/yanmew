import "./ProviderTestStatus.css";

export type ConnectionTestFeedback = { phase: "testing" | "success" | "error"; message: string };

export default function ProviderTestStatus({ feedback }: { feedback?: ConnectionTestFeedback }) {
  if (!feedback) return null;
  return <div className="ym-provider-test" data-state={feedback.phase} role={feedback.phase === "error" ? "alert" : "status"}>
    <span className="ym-provider-test-icon" aria-hidden="true">{feedback.phase === "success" ? "✓" : feedback.phase === "error" ? "!" : ""}</span>
    <p><strong>{feedback.phase === "testing" ? "正在测试连接" : feedback.phase === "success" ? "测试成功" : "测试失败"}</strong><span>{feedback.message}</span></p>
  </div>;
}
