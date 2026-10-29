import OnboardingHost, {OnboardingLayer} from "./components/settings/OnboardingHost";
import FoodCursorWindow from "./windows/FoodCursorWindow";
import PetWindow from "./windows/PetWindow";
import ChatWindow from "./windows/ChatWindow";
import SettingsWindow from "./windows/SettingsWindow";
import TravelWindow from "./windows/TravelWindow";
import LetterWindow from "./windows/LetterWindow";
import MemoryWindow from "./windows/MemoryWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";

function ApplicationWindow() {
  // 原生窗口标签决定入口，URL 参数不会赋予主程序身份。
  const label = getCurrentWindow().label;
  if (label === "onboarding") return <OnboardingLayer/>;
  if (label === "food-cursor") return <FoodCursorWindow/>;
  if (label === "chat") return <ChatWindow />;
  if (label === "settings") return <SettingsWindow />;
  if (label === "travel") return <TravelWindow />;
  if (label === "letter") return <LetterWindow />;
  if (label === "memory") return <MemoryWindow />;
  if (label === "main") return <PetWindow />;
  return <p role="alert">此窗口不属于颜喵主程序。</p>;
}

export default function App(){const label=getCurrentWindow().label;return <>{!["onboarding","food-cursor"].includes(label)&&<OnboardingHost/>}<ApplicationWindow/></>;}
