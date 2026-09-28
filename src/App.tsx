import PetWindow from "./windows/PetWindow";
import ChatWindow from "./windows/ChatWindow";
import SettingsWindow from "./windows/SettingsWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";

export default function App() {
  // 原生窗口标签决定入口，URL 参数不会赋予主程序身份。
  const label = getCurrentWindow().label;
  if (label === "chat") return <ChatWindow />;
  if (label === "settings") return <SettingsWindow />;
  if (label === "main") return <PetWindow />;
  return <p role="alert">此窗口不属于颜喵主程序。</p>;
}
