import type { CSSProperties } from "react";
import type { ChatAppearance } from "../types";

export const DEFAULT_CHAT_APPEARANCE: ChatAppearance = {
  bubbleStyle: "solid", userTextColor: null, assistantTextColor: null,
};

/** 预览、已保存消息和流式消息共用；只允许颜色值，不接收任意CSS。 */
export function chatAppearanceAttributes(appearance = DEFAULT_CHAT_APPEARANCE) {
  const color = (value: string | null) => value && /^#[0-9a-f]{6}$/i.test(value) ? value : undefined;
  return {
    "data-bubble-style": appearance.bubbleStyle,
    style: {
      "--ym-chat-user-text": color(appearance.userTextColor) ?? "var(--ym-text)",
      "--ym-chat-assistant-text": color(appearance.assistantTextColor) ?? "var(--ym-text)",
    } as CSSProperties,
  };
}
