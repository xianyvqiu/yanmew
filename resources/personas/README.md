# 四位预置图片人设

GPT白龙娘、Claude理工大小姐、Gemini猫娘、DeepSeek鲸鱼少女的透明桌宠资源。角色和安装逻辑在 `src-tauri/src/preset_personas.rs`，产品规则见[桌宠自定义创作设计](../../docs/桌宠自定义创作设计.md)和[聊天表情与生图设计](../../docs/聊天表情与生图设计.md)。

每位25张静态透明PNG，总100张：

- `lifted.png`：二次元手拎后颈，各人设独立反应；拖动期间使用。最终提示与参考见[lifted-generation-prompts.json](lifted-generation-prompts.json)。
- `base.png`：用户提供的初始形象，原图不改。
- `standby.png`、`click.png`、`hungry.png`、`eating.png`：待机、互动、饥饿、普通咀嚼。
- `craving.png`、`craving_favorite.png`、`eating_favorite.png`：嘴馋、喜好嘴馋、喜好品尝；咀嚼图不含食物，食物另由 `resources/foods` 显示。
- `sleepy.png`、`bored.png`、`thinking.png`、`chat.png`：睡眠、无聊、思考、自然交谈。自然交谈只在没有更合适情绪时兜底。
- `peek_left.png`、`peek_right.png`、`rest_sit.png`、`rest_lie.png`：两侧探头、坐下、趴下。
- `walk_left.png`、`walk_right.png`：两向行走；运行时叠加轻微身体起伏，并非连续帧动画。
- `emotion_happy.png`、`emotion_angry.png`、`emotion_sad.png`、`emotion_shy.png`、`emotion_afraid.png`、`emotion_speechless.png`：新增六种聊天情绪，每位以自己的base为参考，体现不同性格。

基础互动提示见[generation-prompts.json](generation-prompts.json)，停靠见[docking-generation-prompts.json](docking-generation-prompts.json)，行走见[walking-generation-prompts.json](walking-generation-prompts.json)。六种新情绪的精确提示、参考图和透明参数分别记录于[gpt](gpt/emotion-prompts.json)、[claude](claude/emotion-prompts.json)、[gemini](gemini/emotion-prompts.json)、[deepseek](deepseek/emotion-prompts.json)。生成使用imagegen，用户运行程序不需要生成工具或生成Key；这些内置图片直接随包分发。

开发从本目录读取，发布资源位于EXE旁 `resources/personas`。`scripts/build-portable.ps1` 检查每位25张并复制完整目录，安装版复用便携版流程。首次安装依既有规则等比居中至256×256透明画布，将正式图保存到 `data/assets/accessories` 并建立可编辑/可删除的普通人设和默认会话；不替换当前人设或同名用户人设。

已有预置的六种聊天情绪只补缺一次，通过初始设定和base核对身份。已有同名情绪的图片、说明、启停保持；已删角色不恢复，补齐后删除的情绪不再补回。用户正式图和预览随既有资料包/全量备份携带，源图片仍在资源目录。全部100张已通过实际透明通道、可见像素和安装后重开验证，Windows原生显示效果仍需新构建实机验收。
