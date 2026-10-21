import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { CurrentContext, LetterTarget, RecordId } from "../types";

export type ApplicationPage = "chat" | "settings" | "reminders" | "memory" | "travel" | "mailbox" | "accessories" | "realmoney";
export type SettingsDetailRequest = { page: "persona" | "userProfile"; id: RecordId };
export type SettingsNavigationTarget = SettingsDetailRequest | {page:"travel";mailbox:boolean} | { page: "reminders" | "accessories" } | {page:"realMoney";date?:string|null};
/** 共享页面只请求语义导航，由宿主决定如何呈现。 */
export const navigation = {
  openRealMoney: (date:string|null=null)=>invoke<void>("focus_or_create_window",{page:"settings",detail:{page:"realMoney",date}}),
  open: (page: ApplicationPage) => invoke<void>("focus_or_create_window", { page }),
  openSettingsDetail: (page: SettingsDetailRequest["page"], id: RecordId) => invoke<void>("focus_or_create_window", { page: "settings", detail: { page, id } }),
  openLetter: (context: CurrentContext, travelId: RecordId, replyToId: RecordId | null = null) => invoke<void>("open_letter_window", {...context,travelId,replyToId}),
  takeLetterRequest: () => invoke<LetterTarget | null>("take_letter_navigation"),
  listenLetter: (receive: () => void) => listen("letter-navigation", receive),
  takeTravelRequest: () => invoke<boolean | null>("take_travel_navigation"),
  listenTravel: (receive: () => void) => listen("travel-navigation", receive),
  takeSettingsRequest: () => invoke<SettingsNavigationTarget | null>("take_settings_navigation"),
  takeReminderRequest: () => invoke<boolean>("take_reminder_navigation"),
  listenSettings: (receive: () => void) => listen("settings-navigation", receive),
};
