# 四位预置图片人设

GPT白龙娘、Claude理工大小姐、Gemini猫娘、DeepSeek鲸鱼少女的透明桌宠资源。角色和安装逻辑在 `src-tauri/src/preset_personas.rs`，产品规则见[桌宠自定义创作设计](../../docs/桌宠自定义创作设计.md)和[聊天表情与生图设计](../../docs/聊天表情与生图设计.md)。

每位31张静态透明PNG，总124张：

- `emotion_surprised.png`：普通点击的惊讶反应及聊天中的惊讶语境。各自参考base，分别表现白龙娘失去镇定、Claude克制掩嘴、Gemini竖耳举手、DeepSeek走神后愣住；精确提示见[surprised-generation-prompts.json](surprised-generation-prompts.json)。
- `wait_pet.png`：追到鼠标后期待摸摸，四位角色分别采用抱臂偏头、端庄俯首、站姿举爪凑近和站姿夹本示意头顶的独立反应；提示及参考见[wait-pet-generation-prompts.json](wait-pet-generation-prompts.json)。
- `toss_up.png`、`toss_down.png`、`landing.png`：快速上甩后的上升、下落和落地；四位角色分别生成独立反应，出行时不使用，不参与普通聊天选图。提示及参考见[toss-generation-prompts.json](toss-generation-prompts.json)。
- `error.png`：实际错误提示期间的超载／蔫掉表情，按人设差分，问题解除后恢复；不参与普通聊天选图。生成记录见[error-generation-prompts.json](error-generation-prompts.json)。
- `lifted.png`：二次元手拎后颈，各人设独立反应；拖动期间使用。最终提示与参考见[lifted-generation-prompts.json](lifted-generation-prompts.json)。
- `base.png`：用户提供的初始形象，原图不改。
- `standby.png`、`click.png`、`hungry.png`、`eating.png`：待机、互动、饥饿、普通咀嚼。
- `craving.png`、`craving_favorite.png`、`eating_favorite.png`：嘴馋、喜好嘴馋、喜好品尝；咀嚼图不含食物，食物另由 `resources/foods` 显示。
- `sleepy.png`、`bored.png`、`thinking.png`、`chat.png`：睡眠、无聊、思考、自然交谈。自然交谈只在没有更合适情绪时兜底。
- `peek_left.png`、`peek_right.png`、`rest_sit.png`、`rest_lie.png`：两侧探头、坐下、趴下。
- `walk_left.png`、`walk_right.png`：两向行走；运行时叠加轻微身体起伏，并非连续帧动画。
- `emotion_happy.png`、`emotion_angry.png`、`emotion_sad.png`、`emotion_shy.png`、`emotion_afraid.png`、`emotion_speechless.png`：新增六种聊天情绪，每位以自己的base为参考，体现不同性格。

基础互动提示见[generation-prompts.json](generation-prompts.json)，停靠见[docking-generation-prompts.json](docking-generation-prompts.json)，行走见[walking-generation-prompts.json](walking-generation-prompts.json)。六种新情绪的精确提示、参考图和透明参数分别记录于[gpt](gpt/emotion-prompts.json)、[claude](claude/emotion-prompts.json)、[gemini](gemini/emotion-prompts.json)、[deepseek](deepseek/emotion-prompts.json)。生成使用imagegen，用户运行程序不需要生成工具或生成Key；这些内置图片直接随包分发。

开发从本目录读取，发布资源位于EXE旁 `resources/personas`。`scripts/build-portable.ps1` 检查每位31张并复制完整目录，安装版复用便携版流程。首次安装依既有规则等比居中至256×256透明画布，将正式图保存到 `data/assets/accessories` 并建立可编辑/可删除的普通人设和默认会话；不替换当前人设或同名用户人设。

已有预置的六种聊天情绪只补缺一次，通过初始设定和base核对身份。已有同名情绪的图片、说明、启停保持；已删角色不恢复，补齐后删除的情绪不再补回。用户正式图和预览随既有资料包/全量备份携带，源图片仍在资源目录。资源安装与重开、透明通道及可见像素由预置测试覆盖，Windows原生显示效果仍需新构建实机验收。

抛接图通过独立的一次性`toss`标记只为初始设定及base匹配的旧内置人设补缺，不覆盖编辑、停用、已有分类或已删除角色；之后删除的动作不会再次补回。

等摸摸通过独立的`wait_pet`一次性标记补缺，沿用初始设定及base核验，不覆盖用户已有分类、编辑、停用或删除人设。上抛和下落图片保持正立，运动沿屏幕竖直平面；落地图沿用现有版本。

GPT、Claude、Gemini的`toss_up.png`和`toss_down.png`共6张已按各自base重新生成，保持默认立绘的头身比例与画面占比，保留角色独立反应；DeepSeek和所有落地图不变。最终提示、参考与新图SHA256在[toss-generation-prompts.json](toss-generation-prompts.json)。[toss-proportion-replacements.json](toss-proportion-replacements.json)保存旧版归一化正式图指纹，用于只替换仍使用这些内置旧图的记录。匹配初始设定、base和旧图后才更新；不覆盖自定义图，不恢复删除动作，名称、说明与启停保持。资源更新使用独立本机标记，并纳入全量恢复的保留项。

惊讶资源沿用chat分类，通过独立本机标记`preset_personas_surprise_seeded_v1`一次性补缺。只处理初始设定和base均匹配的内置人设；已有同名图片、说明、启停及已删角色保留。用户后续删除惊讶项不再次补回。该标记不导出，全量恢复保留目标本机标记。新安装直接包含此图，无需数据库结构升级。
