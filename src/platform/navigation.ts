import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type ApplicationPage = "chat" | "settings" | "reminders" | "memory";
/** 共享页面只请求语义导航，由宿主决定如何呈现。 */
export const navigation = {
  open: (page: ApplicationPage) => invoke<void>("focus_or_create_window", { page }),
  takeReminderRequest: () => invoke<boolean>("take_reminder_navigation"),
  listenSettings: (receive: () => void) => listen("settings-navigation", receive),
};
