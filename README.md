# 颜喵 YanMew

项目目标是实现一个仅面向 Windows 的常驻桌面颜文字桌宠。主窗口透明、置顶、可拖动，形象全部使用颜文字；完整产品规划支持多 API 提供商、多 AI 人设、流式聊天、会话记忆、记事本、提醒、天气定位、节日彩蛋，以及旅行、写信、伴手礼、亲密度与饰品等养成内容，并支持备份、导入导出和数据迁移。

> 当前状态：第一阶段桌面壳代码已实现，包括透明置顶宠窗、拖动、托盘和隐藏/恢复；聊天回合、短期记忆、长期记忆来源与回退、混合检索上下文、旅行游记和小游戏插件规则已确认。前端与 Rust 自动测试通过；实机隐藏/恢复已验证，连续拖动、下层点击穿透、托盘恢复及不同 DPI 仍待人工验收。下列核心特性描述完整产品规划，尚未全部实现。

当前阶段的“设置”托盘入口暂不可用；聊天、数据库、AI 和养成能力按后续阶段实现。

## 完整产品规划

- 透明置顶的颜文字桌宠窗口，支持拖动、隐藏/显示、位置记忆。
- 多 API 提供商配置：聊天和记忆整合可分别选择 provider；Embedding 默认使用随便携版打包的 `BAAI/bge-small-zh-v1.5` 本地模型，也可切换到用户配置的联网 OpenAI-compatible Embedding API。
- 多 AI 人设：可切换当前人设、保存初始设定、开启/关闭人设自调、按人设绑定桌宠本体颜色和聊天窗背景。
- 分类表情库：点击（开心/惊讶/害羞）、饿、吃东西、困、无聊、思考；聊天回复时由 AI 实时生成表情。
- 流式聊天：AI 说话前先切换表情，支持暂停/中断；会话和聊天记录在聊天窗内管理。
- 会话级记忆与记事本：一轮完整对话对应一条永久保留的短期记忆；长期记忆带简短标题、明确来源与触发回合，支持精确回退、AI 重新整合、SQLite 全文检索与 Embedding 混合 RAG。
- 提醒系统：一次性、每天/每周/每月重复，支持手动编辑和状态管理。
- 天气与定位：Open-Meteo 优先，和风天气免费版兜底。
- 养成系统：亲密度、喵币、每日签到与节日彩蛋、饰品制作/佩戴/卸载/卖出；小游戏从第一版使用统一插件协议，石头剪刀布作为第一方插件随便携版提供，并支持从本地插件包安装其他小游戏。
- 旅行系统：当前会话在家空闲时累计出行时间；旅行期间写信与回信，正常返回带回伴手礼并生成会话级旅行游记；切换会话取消旅行与待回信。
- 节日彩蛋：内置节日母表 + 会话级节日子表，农历与节气日期每年查内置表自动更新；命中节日固定发放 10 喵币并展示该节日的祝福词，全程离线、不依赖日历接口。
- 桌宠活动：无聊时按概率主动发消息，并在固定宠窗画布内移动颜文字、偶尔靠近画布内的鼠标位置；拖动、点击、聊天、睡觉、出门中或隐藏宠窗时暂停。
- 错误处理：无 Token 或主流程连接失败时进入休眠状态，红色感叹号显示中文错误。
- 导入导出：全量备份、人设资料包、人设完整包、会话包、用户设定包；API Key 不导出。

## 技术栈

- 桌面框架：Tauri 2
- 后端核心：Rust
- 前端：React 18 + TypeScript + Vite
- 数据存储：SQLite（rusqlite）
- 长期记忆检索：SQLite FTS5 + 本地或联网 Embedding 混合 RAG
- 密钥保护：Windows DPAPI
- 发布形态：Windows 便携版 zip，解压即用

## 目录结构

```text
yanwenzi/
├─ src/                      # React + TypeScript 前端
├─ src-tauri/                # Tauri 2 + Rust 核心
├─ plugins/                  # 计划中的插件 SDK、模板、构建工具与第一方小游戏
│  └─ first-party/
│     └─ rock-paper-scissors/ # 随便携版提供的石头剪刀布插件
├─ db/
│  └─ schema.sql             # SQLite 建库脚本
├─ docs/
│  ├─ 前端设计文档.md
│  └─ superpowers/
│     ├─ specs/              # 产品设计规格
│     └─ plans/              # 分阶段实现计划
├─ my_doc/                   # 本地工作记录，已被 git 忽略
├─ package.json
└─ README.md
```

## 环境要求

- Windows 10/11
- Node.js 20+ 与 npm
- Rust stable 与 MSVC 构建工具链
- WebView2 Runtime（通常 Windows 10/11 已自带）

## 启动与关闭（当前第一阶段）

所有命令都在本 README 和 `package.json` 所在的项目根目录执行。

### 直接运行已构建的测试版

在资源管理器中打开本目录下的 `src-tauri\target\debug\yanwenzi.exe`，双击启动。也可以在 PowerShell 中执行：

```powershell
Set-Location "C:\my_program\my_idea\yanwenzi"
.\src-tauri\target\debug\yanwenzi.exe
```

测试版无需启动 Vite。调试版本可能带有控制台窗口；尚未构建时，先执行下面的测试版构建命令。每次只启动一个实例，避免出现多个宠窗和托盘图标。

### 开发模式

首次安装依赖后启动：

```powershell
npm install
npm run tauri dev
```

开发模式会同时启动前端开发服务和桌面程序。`npm run dev` 只启动前端服务，不会打开完整桌面应用。

### 隐藏、恢复和退出

- 点击宠窗的“隐藏宠窗”只会收起颜文字，留下“显示宠窗”按钮，应用仍在运行。
- 点击“显示宠窗”按钮，或左击系统托盘图标，可以恢复宠窗。
- 完全关闭应用：在 Windows 任务栏右下角找到本应用的托盘图标，右键选择“退出”。图标可能位于向上箭头展开的隐藏图标区。
- 开发模式结束时，在运行 `npm run tauri dev` 的终端按 `Ctrl+C` 停止开发命令；如果前端服务仍在运行，也在对应终端按 `Ctrl+C`。
- “设置”目前置灰，聊天、API 提供商配置和养成功能尚未实现。

重新构建前先退出旧程序，否则 Windows 可能因可执行文件被占用而拒绝替换。

## 构建

构建本地测试版：

```powershell
npm run tauri build -- --debug --no-bundle
```

产物：`src-tauri/target/debug/yanwenzi.exe`。

构建 release 可执行文件：

```powershell
npm run tauri build -- --no-bundle
```

产物：`src-tauri/target/release/yanwenzi.exe`，实际路径以构建输出为准。正式发布与便携版验收在后续阶段完成。

如需将 release 程序压缩为 zip：

```powershell
New-Item -ItemType Directory -Force -Path dist | Out-Null
Compress-Archive -Path "src-tauri/target/release/yanwenzi.exe" -DestinationPath "dist/颜喵 YanMew-portable.zip"
```

## 后续扩展

- 语音功能放在首版完成后开发，不属于当前阶段 0–18。
- 规划包含按人设绑定的本地唤醒词、实时流式语音识别、两个内置声线、兼容 TTS 模型导入、联网 ASR/TTS 提供商，以及后续本地音色克隆。
- 语音模型与克隆参考音频存放在 `data/models/voice/`，不进入普通全量备份；详细规则见 `docs/语音扩展设计.md`。

## 数据与隐私（后续实现规划）

- 便携版运行数据位于应用目录下的 `data/`：
  - `data/yanwenzi.db`：SQLite 数据库。
  - `data/chat_background/`：聊天窗背景图片。
- API Key 与和风天气 API Key 使用 Windows DPAPI 加密后存入 SQLite，不保存明文。
- 导入导出不会包含 API Key / 和风天气 Key；全量导入替换全局钱包、账单和设置等数据，新导入提供商密钥需要重新填写，部分包不改变现有全局配置。
- 删除人设或会话时，相关会话级数据按设计执行级联删除；执行前界面会二次确认。

## 文档索引

- 产品设计规格：`docs/superpowers/specs/2026-09-04-yanwenzi-desktop-pet-design.md`
- 实现计划：`docs/superpowers/plans/2026-09-04-yanwenzi-desktop-pet.md`
- 前端设计：`docs/前端设计文档.md`
- 数据库结构：`db/schema.sql`
- 数据库与持久化设计：`docs/数据库与持久化设计.md`
- 记忆与旅行游记设计：`docs/记忆与旅行游记设计.md`
- 小游戏插件生态设计：`docs/小游戏插件生态设计.md`
- 语音扩展设计：`docs/语音扩展设计.md`

## 常用命令

```bash
npm run tauri dev     # 启动 Tauri 开发环境
npm run build         # 构建前端
npm run test          # 运行前端测试
cargo test --manifest-path src-tauri/Cargo.toml  # 运行 Rust 测试
cargo check --manifest-path src-tauri/Cargo.toml # 检查 Rust 编译
```
