-- 人设级聊天外观；旧人设保持原气泡和跟随主题的正文颜色。
ALTER TABLE personas ADD COLUMN chat_bubble_style TEXT NOT NULL DEFAULT 'solid'
    CHECK (chat_bubble_style IN ('solid','transparent','outline','frosted')); -- solid默认；transparent透明；outline仅描边；frosted毛玻璃。
ALTER TABLE personas ADD COLUMN chat_user_text_color TEXT
    CHECK (chat_user_text_color IS NULL OR (length(chat_user_text_color)=7 AND substr(chat_user_text_color,1,1)='#' AND substr(chat_user_text_color,2) NOT GLOB '*[^0-9A-Fa-f]*')); -- 用户正文颜色，#RRGGBB；NULL跟随主题。
ALTER TABLE personas ADD COLUMN chat_assistant_text_color TEXT
    CHECK (chat_assistant_text_color IS NULL OR (length(chat_assistant_text_color)=7 AND substr(chat_assistant_text_color,1,1)='#' AND substr(chat_assistant_text_color,2) NOT GLOB '*[^0-9A-Fa-f]*')); -- AI正文颜色，#RRGGBB；NULL跟随主题。
