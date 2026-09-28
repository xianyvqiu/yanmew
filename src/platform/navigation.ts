import { invoke } from "@tauri-apps/api/core";

export type ApplicationPage = "chat" | "settings";
/** 共享页面只请求语义导航，由宿主决定如何呈现。 */
export const navigation = {
  open: (page: ApplicationPage) => invoke<void>("focus_or_create_window", { page }),
};
