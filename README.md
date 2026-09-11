# 颜喵 YanMew

一个仅面向 Windows 的常驻桌面颜文字桌宠。主窗口透明、置顶、可拖动，形象全部使用颜文字；支持多 API 提供商、多 AI 人设、流式聊天、会话记忆、记事本、提醒、天气定位、节日彩蛋，以及旅行、写信、伴手礼、亲密度与饰品等养成内容，并支持备份、导入导出和数据迁移。

> 当前状态：第一阶段桌面壳代码已实现，包括透明置顶宠窗、拖动、托盘和隐藏/恢复。前端与 Rust 自动测试通过；实机隐藏/恢复已验证，连续拖动、下层点击穿透、托盘恢复及不同 DPI 仍待人工验收。下列核心特性描述完整产品规划，尚未全部实现。

当前阶段的“设置”托盘入口暂不可用；聊天、数据库、AI 和养成能力按后续阶段实现。

## 核心特性

- 透明置顶的颜文字桌宠窗口，支持拖动、隐藏/显示、位置记忆。
- 多 API 提供商配置：名称、base URL、API Key、模型名；聊天和记忆整合可分别选择 provider。
- 多 AI 人设：可切换当前人设、保存初始设定、开启/关闭人设自调、按人设绑定桌宠本体颜色和聊天窗背景。
- 分类表情库：点击（开心/惊讶/害羞）、饿、吃东西、困、无聊、思考；聊天回复时由 AI 实时生成表情。
- 流式聊天：AI 说话前先切换表情，支持暂停/中断；会话和聊天记录在聊天窗内管理。
- 会话级记忆与记事本：短期记忆精确关联原消息，编辑或删除消息时删除对应短期记忆；长期记忆手动维护，支持手动合并与记事本工具调用。
- 提醒系统：一次性、每天/每周/每月重复，支持手动编辑和状态管理。
- 天气与定位：Open-Meteo 优先，和风天气免费版兜底。
- 养成系统：亲密度、喵币、每日签到与节日彩蛋、石头剪刀布、饰品制作/佩戴/卸载/卖出。
- 旅行系统：当前会话在家空闲时累计出行时间；旅行期间写信与回信，正常返回带回伴手礼和总结记忆；切换会话取消旅行与待回信。
- 节日彩蛋：内置节日母表 + 会话级节日子表，农历与节气日期每年查内置表自动更新；命中节日固定发放 10 喵币并展示该节日的祝福词，全程离线、不依赖日历接口。
- 桌宠活动：无聊时按概率主动发消息，并在固定宠窗画布内移动颜文字、偶尔靠近画布内的鼠标位置；拖动、点击、聊天、睡觉、出门中或隐藏宠窗时暂停。
- 错误处理：无 Token 或主流程连接失败时进入休眠状态，红色感叹号显示中文错误。
- 导入导出：全量备份、人设资料包、人设完整包、会话包、用户设定包；API Key 不导出。

## 技术栈

- 桌面框架：Tauri 2
- 后端核心：Rust
- 前端：React 18 + TypeScript + Vite
- 数据存储：SQLite（rusqlite）
- 密钥保护：Windows DPAPI
- 发布形态：Windows 便携版 zip，解压即用

## 目录结构

```text
yanwenzi/
├─ src/                      # React + TypeScript 前端
├─ src-tauri/                # Tauri 2 + Rust 核心
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

## 快速开始

安装前端依赖：

```bash
npm install
```

启动开发模式：

```bash
npm run tauri dev
```

构建 Windows 便携版：

```bash
npm run tauri build
```

构建产物默认位于 `src-tauri/target/release/`，实际输出路径以 Tauri 构建日志为准。

打包便携版 zip（解压即用）：

```powershell
New-Item -ItemType Directory -Force -Path dist | Out-Null
Compress-Archive -Path "src-tauri/target/release/颜喵 YanMew.exe" -DestinationPath "dist/颜喵 YanMew-portable.zip"
```

## 首次使用

1. 启动应用后，默认会使用内置的“喵喵桌宠”人设。
2. 右键桌宠打开菜单，进入“设置”。
3. 在“全局设置 > API 提供商”中添加自己的 OpenAI 兼容接口或官方接口，并选择一个聊天当前提供商。
4. 按需创建人设、用户设定、提醒、天气 API Key 等。
5. 点击桌宠右上角“聊天”按钮开始聊天。

## 数据与隐私

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

## 常用命令

```bash
npm run tauri dev     # 启动 Tauri 开发环境
npm run build         # 构建前端
npm run test          # 运行前端测试
cargo test --manifest-path src-tauri/Cargo.toml  # 运行 Rust 测试
cargo check --manifest-path src-tauri/Cargo.toml # 检查 Rust 编译
```
