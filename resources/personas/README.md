# 四位预置图片人设

GPT（白龙娘）、Claude（理工大小姐）、Gemini（猫娘）、DeepSeek（鲸鱼少女）的透明桌宠资源。角色设定与一次性安装逻辑见 `src-tauri/src/preset_personas.rs`；产品约定见 `docs/桌宠自定义创作设计.md`。

每位角色一个目录，均有以下九张静态透明 PNG：

- `base.png`：用户提供的初始形象原图，未作修改。
- `standby.png`：安静待机，区别于无聊或睡眠。
- `click.png`：开心回应点击、摸摸。
- `hungry.png`：饥饿、期待投喂。
- `eating.png`：开心进食。
- `sleepy.png`：闭眼安稳睡觉。
- `bored.png`：无聊、打哈欠。
- `thinking.png`：认真思考。
- `chat.png`：自然交谈。

八张状态图以对应初始图为参考，使用内置图像生成工具逐张生成。每张的最终提示、参考文件和透明要求保存在 [generation-prompts.json](generation-prompts.json)。四套共36张、约63.3 MiB；均检查有真实 alpha 透明区域、可见像素，文件内容互不重复。它们是静态状态图片，不是连续动画。

开发启动从本目录读取，完整发布目录为 EXE 旁的 `resources/personas/`。`scripts/build-portable.ps1` 会检查36张文件并复制本目录，安装版复用便携版组装流程。用户运行程序不需要图像生成 Key 或访问生成服务。

首次初始化按既有透明图规则等比居中至256×256画布，将正式图发布到 `data/assets/accessories/`，写入普通可编辑、可删除的人设与创作条目。不会替换当前人设、会话或用户同名角色。后续修改、删除以数据库中的用户内容为准，不从本目录自动恢复。源素材随程序分发；已接受的用户图片及修改按既有备份流程携带。
