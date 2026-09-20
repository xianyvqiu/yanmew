# 颜喵 YanMew 实现计划

> 本计划按阶段组织，复选框用于记录实施与验收进度。开始某个阶段前应再次核对产品规格、数据库设计和对应专题文档，以最新已确认规则为准。

**目标：** 构建一个 Windows 常驻桌宠应用“颜喵 YanMew”：透明置顶可拖动的颜文字形象，支持多 API 提供商、多人设、用户设定、流式聊天（含暂停中断）、会话记忆与记事本、表情库、提醒、天气定位、人设自调、喵币与饰品、小游戏插件、旅行/写信/伴手礼/旅行游记、导入导出与数据迁移。

**架构：** Tauri 2（Rust 核心）负责窗口、SQLite、调度、天气定位、AI 客户端、喵币、小游戏插件宿主、饰品、旅行、信件、伴手礼与旅行游记；React + TypeScript 前端负责宠窗、聊天窗、设置窗、游戏中心与插件安装确认的渲染和交互。小游戏运行在受限 WebView，通过版本化桥接访问插件私有存档、游戏局和钱包服务。Rust 暴露 Tauri commands 给前端，AI 流式回复通过 events 推送到前端。数据用 SQLite 持久化，API Key 用 Windows DPAPI 加密。

**技术栈：** Tauri 2、Rust、React 18、TypeScript、Vite、SQLite（rusqlite）、windows crate（DPAPI、定位）。

---

## 约定

- 所有命令在项目根目录 `C:\my_program\my_idea\yanwenzi` 执行。
- 前端包管理器：npm。
- 提交信息用中文，格式如 `feat: 添加xxx`、`fix: 修复xxx`、`test: 添加xxx测试`。
- Rust 测试：`cargo test --manifest-path src-tauri/Cargo.toml`；前端测试：`npm run test`（Vitest）。
- 小 Task 完成后不单独提交；一个较大功能或完整阶段通过测试和人工验收后，再统一创建一次可审阅提交并按需推送。本计划不安排逐 Task Git 操作。
- Windows 换行：仓库用 LF，Git 已配置自动转换，不要手动改 CRLF。
- 本计划分 19 个阶段（阶段 0 到阶段 18），每个阶段结束时应用可编译运行。
- 数据库结构只维护 `db/schema.sql`，初始化阶段建立当前已确认的完整 V1 结构，其中已经包含小游戏插件表和通用插件账单字段；后续任务补齐模型、命令和交互，不重复增加已存在表或列。事务与备份协议统一遵守 `docs/数据库与持久化设计.md`。
- AI 请求不持有数据库锁或事务；读取快照、请求完成后短事务校验并提交。
- 阶段 0–18 交付 Windows；为未来 Android 预留《项目架构与代码职责》的平台边界。共享业务不接收 Tauri 窗口句柄或调用 Windows API，Windows 实现经平台接口注入；本计划中的 DPAPI、托盘、多窗口和便携路径均是 Windows 适配，不是跨平台业务规则。Android 工程与跨设备同步不在本阶段创建。

### 平台边界随阶段落地

- 阶段 2：创建 `runtime.rs`、`platform/mod.rs`、`platform/contracts.rs`、`platform/windows/{mod,paths,secrets}.rs`；`config.rs` 使用 `AppPaths`，`crypto.rs` 使用 `SecretProtector`。系统依赖按目标隔离，保留现有桌面壳，不提前迁移已完成文件。
- 阶段 3–5：`events.rs` 定义普通事件数据和 `EventSink`，Windows 适配负责 Tauri 发送；`platform/lifecycle.rs` 向唯一 `jobs` 调度器提供运行许可，计时使用可注入时钟。用假暂停/恢复验证不累计停机时间、不自动重发聊天。
- 阶段 6–8：定位、提醒投递和文件交互通过平台接口；Windows 提醒仍使用既定气泡。创建共享 `pages/*Page.tsx` 与薄 `windows/*Window.tsx`，新增 `src/platform/capabilities.ts`、`navigation.ts`；共享页面不直接调用窗口 API。
- 阶段 9、13：模型加载从受管资源位置取得文件；插件桥接绑定可信宿主实例，WebView 创建与业务授权分离；不把 Win32 路径或窗口标签写进跨平台 SDK。
- 阶段 17–18：逻辑备份不携带原生密钥或绝对资源路径。完成 Windows 验收；Android 编译、原生悬浮服务、手机模型资源和跨端备份往返留给后续专项，不标记为已支持。

## 执行顺序与交付门槛

详细接缝、恢复与用例见 [实施接口与验收约定](../../实施接口与验收约定.md)。各 Task 的编译检查只是必要条件，还必须通过该文档对应阶段的行为验收。

1. 阶段 0 已完成；阶段 1 已实现但人工验收尚未全部完成。先核对现状，不能重新覆盖现有工程。
2. 阶段 2–4 建立配置、传输和状态服务；阶段 5 接持久化回合，阶段 6–7 完成提醒及环境服务，阶段 8 才打通可用聊天和设置界面。阶段 2 的“前端设置可用”属于阶段 8 的联调目标。
3. 阶段 9–12 完成记忆、工具、消息操作和人设。阶段 9 可读取尚为空的旅行/节日表，但未实现的工具不得注册；分别在阶段 16、18 完成接入验收。
4. 阶段 13 完成钱包和插件；阶段 14 先实现旅行状态与信件。正常返回会依赖阶段 15 的饰品和阶段 16 的伴手礼/游记，三者联合验收前不向正式用户开放完整旅行功能。
5. 阶段 17 先实现备份框架与已有数据往返，阶段 18 接入节日后必须重跑全量与部分备份用例。语音始终不在本次 0–18 范围内。
6. 每阶段交付：代码与文档、接口清单、迁移/资源变更（如有）、自动测试结果、人工验收记录和未解决项。失败用例或未确认规则阻塞对应能力，不把它标为完成。

## 项目结构（现有文件增量修改，缺失文件按阶段创建）

详细文件职责、内部辅助模块、事务所有者及通信时序见[项目架构与代码职责](../../项目架构与代码职责.md)。下列清单为阶段入口，AI 工具注册、请求调度、记忆检索和设置子面板按架构文档在相应阶段增量创建；首版使用 Rust 自建 AI 编排，不引入 LangChain/LangGraph 运行时。

- `package.json`、`vite.config.ts`、`tsconfig.json`、`tsconfig.node.json`、`index.html`：前端脚手架。
- `src/`：React 前端。
  - `src/main.tsx`：入口。
  - `src/App.tsx`：根据窗口类型路由到宠窗/聊天窗/设置窗。
  - `src/types.ts`：共享类型。
  - `src/api.ts`：Tauri command/event 封装。
  - `src/windows/PetWindow.tsx`：宠窗。
  - `src/windows/ChatWindow.tsx`：聊天窗。
  - `src/windows/SettingsWindow.tsx`：设置窗。
  - `src/windows/MemoryWindow.tsx`：记忆小窗。
  - `src/windows/TravelWindow.tsx`：出行/邮箱/伴手礼管理窗。
  - `src/windows/LetterWindow.tsx`：写信界面。
  - `src/components/GameCenterPanel.tsx`：设置窗内的游戏中心与启动入口。
  - `src/components/PluginManagerPanel.tsx`：设置窗内的插件安装、权限、存档与来源管理。
  - `src/components/Kaomoji.tsx`、`Bubble.tsx`、`ContextMenu.tsx`、`ErrorBadge.tsx`、`AccessoryLayer.tsx`。
- `src-tauri/`：Rust 核心。
  - `src-tauri/Cargo.toml`、`tauri.conf.json`、`build.rs`、`capabilities/default.json`。
  - `src-tauri/src/main.rs`、`lib.rs`、`error.rs`、`state.rs`。
  - `src-tauri/src/db.rs`、`models.rs`、`config.rs`、`crypto.rs`。
  - `src-tauri/src/providers.rs`、`persona.rs`、`user_profiles.rs`、`expressions.rs`、`sessions.rs`、`memory.rs`、`memory_commands.rs`、`notebook.rs`、`context.rs`、`proactive.rs`、`festivals.rs`、`lunar.rs`。
  - `src-tauri/src/ai/mod.rs`、`ai/client.rs`、`ai/streaming.rs`。
  - `src-tauri/src/pet.rs`、`reminders.rs`、`weather.rs`、`location.rs`、`currency.rs`、`game_plugins.rs`、`game_host.rs`、`accessories.rs`、`travel.rs`、`letters.rs`、`souvenirs.rs`、`import_export.rs`、`commands.rs`。
- `plugins/`：插件 SDK、模板、统一校验/打包工具、随包清单和第一方小游戏。
  - `plugins/sdk/`、`plugins/template/`、`plugins/build/`。
  - `plugins/bundled-games.json`：随便携版提供的插件清单。
  - `plugins/first-party/rock-paper-scissors/`：石头剪刀布插件源码。

---

## 阶段 0：脚手架（已完成）

目标：保留现有 React、TypeScript、Vite、Tauri 2 工程，不重新初始化或覆盖入口。

### Task 0.1: 前端基线

- [x] 前端工程与 Vitest 已建立。
- [ ] 接手时检查 `package.json` 与锁文件，运行 `npm run test`、`npm run build`，记录结果。已有依赖优先复用，不照旧模板重装另一套版本。

### Task 0.2: Tauri 基线

- [x] Rust 入口、应用配置、托盘及 Windows 依赖已建立。
- [ ] 接手时运行 `cargo test --manifest-path src-tauri/Cargo.toml`；扩展现有 `lib.rs` 和命令注册，不替换整份文件。

## 阶段 1：桌面壳（已实现，人工验收待补齐）

目标：保护现有透明、置顶、可拖动、隐藏恢复和托盘行为。

### Task 1.1: 宠窗与拖动

实施位置：`src/windows/PetWindow.tsx`、`usePetWindow.ts`、样式和现有 Rust 窗口命令。

- [x] 可运行桌宠和拖动逻辑已实现。
- [ ] 实机连续拖动、从不同位置按下/释放，验证不会卡住拖动状态，颜文字右侧不被裁切。
- [ ] 在 100%、125%、150% 缩放及跨显示器情况下检查点击区域、文字边界与拖动，记录实际环境与结果。

### Task 1.2: 托盘

- [x] 托盘退出、隐藏恢复入口已实现。
- [ ] 人工验证：隐藏后托盘可恢复；重复恢复只有一个宠窗；退出后无遗留进程或托盘图标。当前设置入口保持未实现状态，阶段 8 接入后启用。

### Task 1.3: 点击穿透与隐藏恢复

实施位置：现有 `src-tauri/src/pet_region.rs`、窗口命令与前端命中区域上报。

- [x] Windows 命中区域与隐藏/恢复逻辑已实现。
- [ ] 宠窗透明空白处能点击下层应用；颜文字、按钮、气泡交互处不能穿透。
- [ ] 恢复后重新同步命中区域；后续增加饰品、气泡和游走时复用该机制，不能直接对整个宠窗启用鼠标忽略。

阶段完成条件：自动检查通过，并补齐上述人工记录；“代码已实现”不能替代实机验收。

## 阶段 2：配置存储（SQLite、DPAPI、提供商与人设 CRUD）

目标：Rust 端可持久化提供商、人设和参数，API Key 加密；提供前端增删改查与切换的类型化接口，完整设置页在阶段 8 接入。

当前实施进度：建库、版本识别、默认值、DPAPI、提供商与人设 CRUD、当前项选择、数值/主题设置、独立天气凭据接口和连接测试已实现。`src/api.ts` 可调用上述接口；设置窗口尚未实现。人设切换已具备旅行与待回信取消事务，未来聊天请求注册器接入后补取消进行中请求的验收。Embedding 当前仅初始化内置配置，尚未运行模型；插件包未构建，因此不登记可运行插件。

当前命令约定：数据库 ID 使用十进制字符串；DTO 字段用 camelCase。`get_settings/save_settings` 只处理已登记的数值和主题键，批量更新原子提交；`get_weather_settings/save_weather_settings` 单独处理 Host 和密钥三态更新，返回 `apiHost/hasApiKey`。`current_session_id` 与窗口位置属于内部状态，不允许从通用设置写入。密钥输入为 `{ action: "keep" }`、`{ action: "clear" }` 或 `{ action: "set", value: "..." }`，省略时保留；列表不返回明文或密文。

`test_provider_connection` 对保存的模型发送一次非流式短提示，连接超时 10 秒、总超时 30 秒，拒绝重定向，响应体上限 256 KiB；验证 HTTP 状态和实际聊天内容，不创建消息、不重试、不把上游正文作为错误展示。真实付费提供商需用户自行执行连接测试，本地自动测试只使用假服务。

### Task 2.1: 数据模型与数据库初始化

实施位置：`db.rs`、`models.rs`、`state.rs`、`config.rs`、现有 `lib.rs` 与 `Cargo.toml`。

1. 添加计划采用的 SQLite、日期依赖，锁定实际验证的版本；SQLite 必须包含 FTS5 与 trigram 支持。先在测试中执行完整 schema 并查询 trigram 索引，再接应用启动。
2. Windows 路径适配以可执行文件所在目录为基准提供 `AppPaths`，业务据此写入 `data/yanwenzi.db`，不依赖启动工作目录。不可写时显示可操作的中文错误并停止业务初始化，不悄悄创建第二个数据库。共享业务不能自己调用 executable-path API；Android 将来由另一适配提供私有目录。
3. 每个连接在事务外开启外键并确认值为 1。新库在一个事务内执行 `include_str!("../../db/schema.sql")`、种子和 `user_version` 写入。
4. schema 一次建立完整 V1 表、索引和触发器。唯一内置人设、全局钱包、默认设置、内置 Embedding 配置幂等初始化；当前项必须存在且归属正确。设置模型逐项对应 schema 注释和专题设计，不使用只包含部分字段的样例代替完整配置。
5. 阶段 2 只登记 Embedding 配置，模型推理由阶段 9 验收；插件资源由阶段 13 构建。只有实际存在且校验通过的随包插件才能登记为可用，不提前制造可启动的石头剪刀布条目。完整 V1 发布必须通过随包资源检查。
6. 已有库按 `user_version` 执行明确的版本化迁移；迁移前生成一致性备份。未知版本或无法识别的无版本数据库停止升级并提示，不删库、不猜测修复。当前未发布数据库功能，不虚构历史版本迁移脚本。
7. 保留现有宠窗共享状态，增量接入数据库服务。仓储锁只覆盖同步数据库工作；不跨 AI、HTTP、文件下载或用户确认持有锁。
8. 分离内部持久化模型与前端 DTO。提供商 DTO 只返回 `hasApiKey`，不得返回密钥明文或 DPAPI 密文。

验收：空库启动、重复启动、目录不可写、外键开启、非法关联被拒绝、事务故障整体回滚、已支持旧版本升级、未知版本拒绝；每个用例检查实际行数及当前项，不只检查编译。

### Task 2.2: DPAPI 加解密

实施位置：`crypto.rs` 门面、`platform/contracts.rs` 的 `SecretProtector`、`platform/windows/secrets.rs`、提供商仓储与现有 Windows 依赖。DPAPI 原生调用只在 Windows 适配内。

1. 复用当前 `windows = 0.61` 的 Windows 专属依赖声明，按实际 API 补齐密码学和内存释放 features；保留已有窗口命中区域需要的 features，不另加旧版依赖覆盖它。
2. 封装当前 Windows 用户范围的加密/解密；统一编码格式和所有权，系统分配内存必须正确释放。错误返回中文说明，日志、DTO 和错误内容不得包含密钥。
3. 提供商更新区分“不修改密钥”“设置新密钥”“清除密钥”，不能用空输入框意外覆盖已保存值。
4. 解密失败只标记该配置不可用并提示重新填写；不删除其他配置、不以空密钥发请求。备份不携带任何主程序密钥的明文或密文。

验收：加解密往返、中文/特殊字符、损坏密文、清除与保留、前端列表无密钥字段。Windows API 调用以当前锁定依赖实际编译和测试为准。

### Task 2.3: 提供商、人设与设置仓储

**Files:** `src-tauri/src/providers.rs`、`persona.rs`、`config.rs`、`models.rs`、`commands.rs`、`lib.rs`、`src/api.ts`。

- [x] **Step 1: 提供商仓储与 DTO**

实现列表、新增、编辑、删除、测试、聊天当前项及记忆专用项选择。后端内部模型保存加密密钥；前端列表 DTO 只返回 `has_api_key`，不返回密文或明文。编辑时未提交密钥表示保留，清除使用独立标记。DPAPI 密文编码后持久化。

当前项切换先验证目标存在，再在同一事务中清除旧项、启用新项；目标不存在时旧项保持不变。`set_memory_provider(None)` 清除独立记忆选择并跟随聊天。正被聊天或记忆使用的提供商禁止删除。唯一索引作为最终约束。

- [ ] **Step 2: 人设仓储与当前上下文**

按完整 schema 读取和保存人设字段；创建时 `initial_prompt=prompt`。切换人设在统一上下文服务中执行：校验目标、中断旧聊天请求、按旅行规则取消离开会话的旅行与待回信、切换当前人设并恢复其有效当前会话，事务提交后广播上下文变化。后续旅行阶段扩展此服务，不新增一条只改 `is_active` 的旁路。

默认人设禁止删除。其他人设删除在同一事务中先删除其会话范围内的长期记忆，再删除人设及剩余级联数据；删除当前项时切回默认人设并修正 `settings.current_session_id`。有默认人设会话时恢复最近更新项，无会话时清空当前会话设置。事务失败保留旧当前项，已取消的请求不自动重发。

- [x] **Step 3: 设置仓储**

实现 `get_settings` 与 `save_settings`；缺失键使用已确认默认值，非法值返回可定位的中文错误，不把解析失败静默变成零。保存前校验数值范围、最小值不大于最大值及主题枚举。批量修改使用同一事务，失败不部分生效。后续增加设置项时扩展同一模型与校验入口。

- [x] **Step 4: 注册命令与界面契约**

命令名与《前端设计文档》第 13.3 节统一：提供商使用 `list_providers`、`create_provider`、`update_provider`、`delete_provider`、`test_provider_connection`、`set_active_chat_provider`、`set_memory_provider`；人设使用 `list_personas`、`create_persona`、`update_persona`、`delete_persona`、`set_active_persona`。同时注册设置读取与保存命令。异步 AI 调用与连接测试不得跨网络等待持有数据库锁。

- [x] **Step 5: 验证**

验证密钥不出现在前端 DTO、切换不存在目标不丢失旧当前项、当前提供商不可删、默认人设不可删、删除当前人设恢复默认上下文、事务失败回滚和设置校验。运行 Rust 测试及编译检查。

---

## 阶段 3：AI 引擎（流式传输与结构化生成）

前置：阶段 2 配置读取、密钥解密和错误返回可用。本阶段测试传输服务；持久化聊天命令待阶段 5、8 接入，不能依赖尚未实现的会话模块。

### Task 3.1: HTTP 客户端与取消

实施位置：`ai/client.rs`、`ai/streaming.rs`、`error.rs`、共享请求管理。

- [ ] 复用异步 HTTP 客户端，设置连接、首事件等待与流空闲超时；区分 HTTP 错误、协议错误、用户取消。超时参数集中管理并用假服务测试，不能无限等待。
- [ ] 每个请求持有取消信号；表情生成、正文和工具循环共用同一请求生命周期。
- [ ] 不自动重发可能已执行工具或输出部分正文的聊天请求；后台只读生成的重试另行调度。

### Task 3.2: OpenAI-compatible 流式客户端

- [ ] 每个响应只创建一个持续读取的字节流。保留跨块缓冲；UTF-8 字符、SSE 行、事件和 JSON 均可能跨网络块，不能逐块有损解码后按行直接解析。
- [ ] 处理 CRLF/LF、空行事件边界、注释/心跳、多条 data 行、单块多个事件；仅完整事件交给 JSON 解析器。
- [ ] 按 choice 与 tool call index/ID 拼接正文、工具名和参数；参数完成并校验前不执行工具。
- [ ] 区分正常终止、工具调用轮结束、长度截断、用户取消与异常断流。TCP EOF 本身不证明回复完成；正常完成判据按适配器协议验证，不能把缺少终止标志的断流算成 completed。
- [ ] 非流式结构化生成使用任务对应的输出预算与 schema 校验；不把所有表情、标题、长期记忆统一限制为 512 token。非法 JSON/空字段/截断不得写入有效业务结果。

验收：假服务器逐字节拆分中文、拆分 JSON、一次返回多事件、工具参数分块、空心跳、HTTP 401/429/500、半途断流、取消后迟到事件；检查输出不丢字、不重复，不提前执行工具。

### Task 3.3: 人设表情库生成

- [ ] 根据产品规格生成完整分类，逐项校验非空颜文字数组及可显示长度；缺少分类或解析失败保留旧库。
- [ ] 固定目标人设及生成快照，生成后检查目标仍存在且配置未失效。无提供商时保留默认显示并提示，不阻止创建人设。
- [ ] 自动生成与按钮生成复用同一服务，防止并发覆盖较新结果。

### Task 3.4: 先表情、后正文的聊天服务接缝

- [ ] 定义后端服务输入为已验证的 provider、人设与会话快照；前端不提交权威人设提示词或整份历史来替代数据库读取。
- [ ] 先生成并校验本轮表情，再发正文流；统一 request ID 与取消信号。
- [ ] 本阶段用固定测试上下文验证事件顺序；阶段 5 保存回合、阶段 8 注册完整 UI 命令、阶段 9 接入记忆与工具。不存在的功能不能先注册空实现向用户宣称可用。

## 阶段 4：桌宠状态机与活动

前置：阶段 1 窗口、阶段 2 参数、阶段 3 请求状态。实施位置：`pet.rs`、共享状态、互动命令与宠窗事件。

### Task 4.1: 桌宠状态机核心

1. 分开保存上次喂食、最近互动、睡眠结束时刻、当前活动、主流程错误和出行状态；饥饿不使用互动时间代替喂食时间。
2. 饥饿默认 120 分钟，无聊默认 45 分钟；睡眠持续时间默认 60–120 分钟，进入睡眠时只抽取一次。无互动入睡阈值 `sleep_idle_minutes` 默认 45 分钟，与无聊默认阈值相同，到点随机选择无聊或睡眠，用户可在全局“时间与桌宠”设置中修改；与睡眠持续时间分开保存。
3. Dormant 只能由显式恢复流程解除；定时循环不覆盖它。出门、聊天/工具处理中和用户互动等约束按产品规格判断；一次循环只提交一次有效状态变化，不每秒重复弹同一气泡。
4. 睡眠结束或合法唤醒事件结束本次睡眠；短时互动表情结束后重新计算状态，不能固定恢复 Normal 而覆盖出行或休眠。
5. 仅在状态允许时调用后续无聊主动消息与游走服务；未接入阶段使用测试替身，不提前产生后台 AI 请求。
6. 时钟与随机数可注入，使用推进时间的测试覆盖边界，测试不等待真实的几十分钟。

验收：喂食与普通点击计时分离、睡眠只抽一次、手动唤醒、无聊进入只通知一次、休眠不被循环解除、聊天/出门期间不游走；验证默认无互动 44 分钟不触发、45 分钟用固定随机源分别进入无聊和睡眠；每段空闲只抽一次，睡眠分支不发无聊主动消息，修改阈值后按新值判断。

无聊/睡眠实现与验收统一遵守产品规格“无聊与睡眠的随机分支”：默认同时到 45 分钟时各 50% 二选一，每段空闲只抽一次；不同阈值先到先进入，互动或睡眠结束开启下一周期。测试使用固定随机源分别覆盖两个分支，不能只等待真实随机结果。

## 阶段 5：聊天会话与记忆

目标：会话按人设隔离、新建/切换/继续、消息持久化与历史查询。

### Task 5.1: 会话、聊天回合与消息仓储

**Files:** `src-tauri/src/sessions.rs`、`models.rs`、`commands.rs`、`lib.rs`。

- [ ] 按 `db/schema.sql` 建立 `Session`、`ChatRound`、`Message` 模型；消息包含可空 `round_id`。
- [ ] 提供会话新建、列表、标题更新和按 ID 顺序读取消息的仓储接口，所有操作验证会话归属。
- [ ] 用户发送时在一个事务内建立 `pending` 回合、插入用户消息并更新会话时间与 `memory_revision`。
- [ ] AI 正常完成时在一个事务内插入该回合唯一 AI 消息并把回合改为 `completed`；中断时按有无正文保存一次并标记 `interrupted`。
- [ ] 无聊主动消息直接写入 `messages.round_id=NULL`，不创建回合。
- [ ] 注册所需 Tauri 命令；验证同一回合不能插入两个相同角色、跨会话 round/message 关联被 Rust 拒绝。
- [ ] 运行 `cargo check --manifest-path src-tauri/Cargo.toml`，通过后完成本任务验收。

---
## 阶段 6：提醒系统

目标：一次性/重复提醒、运行中独立提醒气泡、离线到期启动汇总和每次触发记录的手动状态管理。

### Task 6.1: 提醒规则与每次触发记录

**Files:** `src-tauri/src/reminders.rs`、`models.rs`、`commands.rs`、`lib.rs`。

- [ ] 从完整 schema 接入 reminders 与 reminder_occurrences 模型；提醒规则包含非空 name 和详情 content，触发记录保存 scheduled_at、status、delivery_state。
- [ ] 实现规则 CRUD 和下一次时间计算。日/周/月规则统一本地日期计算、UTC 保存；月度不存在的日期跳过。重复规则 status 保持 pending，删除才永久停止。
- [ ] 在线调度事务：插入唯一 occurrence(queued) 并推进 fire_at；一次性 fire_at 可置 NULL，但原时间已经存到 occurrence。后台 AI 润色在事务外执行，失败固定回退为“`{提醒名}事件提醒：{提醒详情}{提醒时间}`”。
- [ ] 重启先恢复已存在但尚未展示的 queued occurrence，再处理 missed 汇总；不得把已入队记录丢弃或重复创建。投递事件丢失时前端可主动查询，展示确认接口只更新 delivery_state。
- [ ] 启动补偿先于常规循环：所有错过的一次性提醒分别写 missed occurrence；每个重复规则只为最近一次错过时间写一条 missed occurrence 并推进到未来，其余跳过。两者均用唯一约束避免重复。
- [ ] 暴露 `get_missed_reminder_summary`、`ack_missed_reminder_summary`、`list_reminder_occurrences`、`update_reminder_occurrence_status` 和 `complete_reminder_occurrence`。完成重复提醒只改本次，完成一次性同时更新规则。
- [ ] 前端启动后主动读取 missed 汇总，显示“你离开这段时间，{提醒名称1-n}过期了！”。确认展示后更新 delivery_state，避免下次启动重复弹出；未修改状态的记录继续保留在提醒页面。
- [ ] 验证：启动时监听未就绪、多个一次性提醒、同一循环错过多次、汇总只展示一次、手动修改状态、重复点击完成、调度与补偿同时发生、月末无日期、删除规则级联记录。事务失败不推进游标。

### Task 6.2: 聊天提醒工具

**Files:** `src-tauri/src/reminder_extract.rs`、`chat.rs`、`commands.rs`、`lib.rs`。

- [ ] 提供 `create_reminder` function calling 工具，支持一次性及 daily/weekly/monthly 结构，输入上下文包含当前本地时间与时区。
- [ ] 解析结果交由 Rust 校验日期、循环规则和内容；没有明确时间时请用户补充，不保存无法触发的空时间提醒。
- [ ] 工具调用写入规则后返回 ID；阶段 9 工具循环注册该工具，前端设置页显示结果。不要依靠前端额外猜测用户消息是否要求提醒。
- [ ] 数据库锁仅用于取配置和保存结果，不跨 AI await 持有；模型解析失败不写半成品。
- [ ] 验证一次性、每天、每周和每月聊天创建，以及缺失时间、非法规则；执行 Rust 测试与编译。

---

## 阶段 7：天气与定位

前置：阶段 2 非密钥配置与 DPAPI；天气失败不能让聊天进入休眠。

### Task 7.1: 天气与定位服务

实施位置：`location.rs`、`weather.rs`、配置模型、设置页与上下文服务。

1. 在后台调用 Windows 定位，处理权限拒绝、服务不可用、取消和超时；不在 UI 线程阻塞等待。拿不到位置时返回“位置不可用”，不使用 (0,0) 假坐标。
2. 以有效经纬度查询 Open-Meteo；失败且用户已配置和风凭据时调用和风。统一结果包含地点、观测时间、来源、天气描述及可选气温，缺失数值保持空而不是伪造 0℃。
3. 和风请求使用用户控制台分配的 API Host，不硬编码旧 `devapi.qweather.com`。配置需保存非密钥 Host 和 DPAPI 加密 Key；开发接入前在设置设计中落实 Host 输入与校验。API Host 必须为合法 HTTPS 主机，不能把 Key 写入日志。
4. 两个服务的天气代码分别映射到统一文本，不能把和风代码按 Open-Meteo/WMO 编号解释。
5. 缓存结果连同观测时间和来源；刷新失败时明确标示缓存或不可用。上下文使用有时效标识的数据，不把旧观测说成当前事实。
6. 对定位与天气分别做可取消的有界请求，避免天气拖住聊天；联网测试单独执行，自动测试用固定响应，不依赖真实位置与在线服务。

验收：定位拒绝、主服务成功、主服务失败后兜底成功、两者失败、未配置和风、字段缺失、缓存过期、跨会话迟到响应；失败均不改变回合状态或钱包。

接口依据：[和风 API Host 配置](https://dev.qweather.com/docs/configuration/api-host/)、[身份认证](https://dev.qweather.com/docs/configuration/authentication/)。供应商接入时再次核对官方接口，不把第三方旧示例当作稳定契约。

## 阶段 8：多窗口与前端交互

目标：宠窗、聊天窗、设置窗可用；事件与命令在前端打通。除固定画布的主宠窗外，主要二级窗口可拖动、可缩放并按容器宽度响应式重排。

### Task 8.1: 窗口创建与上下文恢复

实施位置：现有 Rust 窗口命令、`lib.rs`、capabilities 和前端路由。

- [ ] 使用固定窗口标识创建聊天、设置等主程序二级窗口；已有窗口只显示并聚焦，不新建重复实例。窗口标识与前端路由集中维护。
- [ ] 按前端设计配置可缩放、拖动、最小尺寸和关闭行为；关闭二级窗口只关闭该窗口，应用退出只走明确的退出命令。
- [ ] 窗口先订阅再读取 `get_current_context`，按 contextRevision 过滤乱序响应；切换人设/会话后刷新所有会话级页面。
- [ ] 已有主宠窗的透明、命中区域、拖动与托盘注册保持不变；增量注册命令，不复制一套新的 run/setup。
- [ ] 主程序窗口与插件窗口使用不同能力边界，插件不得复用主程序全部 capabilities。
- [ ] 验证重复打开、关闭重开、切换过程中返回旧请求、事件丢失后查询恢复、最小窗口布局与 DPI。

### Task 8.2: 前端类型与 API 封装

**Files:**
- Create: `src/types.ts`
- Create: `src/api.ts`

- [ ] **Step 1: 编写类型**

`src/types.ts` 对齐实际 Rust 返回 DTO 与 schema；内部存储模型不能直接当作前端接口。

- `Provider` 包含名称、模型、地址、当前项标记与 `has_api_key`；不得包含 `api_key_encrypted` 或已保存的明文密钥。
- `Persona` 包含当前/初始设定、自调开关、颜色、背景配置、内置与当前标记及表情库。
- `Session` 包含人设、可选用户设定、标题、时间及版本；`ChatRound` 包含状态；`Message` 包含可空 `round_id`，主动消息使用 NULL。
- `Reminder` 包含非空 `name`、规则和下一次时间；`ReminderOccurrence` 独立包含原定时间、完成状态与展示状态。
- `AppSettings` 严格按照《数据库与持久化设计》第 2.7 节的 `settings` 完整键清单统一表达数值设置与 `theme_mode=system/light/dark`；键名、默认值、缺省行为和成对校验只维护一份，后续阶段新增键必须先更新该节与迁移。
- `PetState` 区分正常、饿、无聊、睡觉、吃东西、说话、思考、休眠；旅行使用独立状态。

- [ ] **Step 2: 编写 API 封装**

`src/api.ts` 按《前端设计文档》第 13 节封装命令和事件；API 名称以该节为准，Rust 命令、前端封装和注册列表一起维护。

- 聊天只发送会话、请求标识和正文，不从前端提供权威人设提示词或历史，不提供通用消息写库接口。
- 流式事件必须携带会话、回合、请求和序号；所有监听返回可释放句柄，窗口卸载时清理。
- 提醒规则 CRUD 与 occurrence 状态命令分开；展示离线汇总后调用 `ack_missed_reminder_summary`。
- 提供商 DTO 只含密钥存在标记；所有会话级命令验证上下文归属。
- 验证命令注册、类型对应、错误显示、重复事件与跨窗口刷新。

### Task 8.3: 窗口路由与宠窗

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/windows/PetWindow.tsx`
- Create: `src/components/Kaomoji.tsx`
- Create: `src/components/ErrorBadge.tsx`
- Create: `src/components/ContextMenu.tsx`

- [ ] **Step 1: 编写窗口路由**

`src/App.tsx`:
```tsx
import ChatWindow from "./windows/ChatWindow";
import PetWindow from "./windows/PetWindow";
import SettingsWindow from "./windows/SettingsWindow";

export default function App() {
  const windowType = new URLSearchParams(window.location.search).get("window");
  if (windowType === "chat") return <ChatWindow />;
  if (windowType === "settings") return <SettingsWindow />;
  return <PetWindow />;
}
```

- [ ] **Step 2: 编写基础组件**

`Kaomoji` 接受表情与当前人设颜色；`ErrorBadge` 点击显示应用内中文气泡，并在休眠时提供刷新入口；`ContextMenu` 支持禁用项、主题颜色和边界定位。复用已实现的宠窗组件与命中区域更新机制，菜单和错误气泡不能超出可用画布。不得用系统 alert 代替已设计的错误气泡。

- [ ] **Step 3: 接入宠窗业务交互**

在现有第一阶段 `PetWindow.tsx`、`usePetWindow.ts` 与 Rust 窗口命令上增量接入表情、聊天入口、设置入口、菜单和错误气泡，保留已完成的拖动阈值、隐藏恢复、原生命中区域和托盘逻辑。按钮不得冒泡触发拖动或亲密度；动态气泡和菜单出现后同步更新命中区域。错误使用可见中文气泡，不使用系统 alert。组件颜色复用主题变量。

- [ ] **Step 4: 运行测试与构建**

Run: `npm test -- PetWindow.test.tsx`
Expected: PASS（若测试断言默认颜文字需改为 `(・ω・)` 仍然成立）。
Run: `npm run build`
Expected: 无错误。

### Task 8.4: 聊天提交与表情生成接口

**Files:** `src-tauri/src/commands.rs`、`src-tauri/src/chat.rs`、`src-tauri/src/lib.rs`、`src/api.ts`。

- [ ] 前端聊天接口只提交当前会话、请求标识和用户正文；Rust 校验会话归属并从数据库读取人设与历史，结合阶段 5 的回合仓储统一保存用户消息与 AI 回复。
- [ ] 不向前端开放任意角色的 `append_message` 命令。用户发送、AI 完成、中断和主动消息的保存全部由后端业务流程负责，窗口只渲染事件并在结束后刷新权威消息列表。
- [ ] 流式事件携带 `request_id`、`session_id`、`round_id` 和事件序号，取消命令按请求标识执行。切换会话后丢弃旧事件。
- [ ] `generate_expressions` 读取指定人设快照并在事务外生成，保存前校验人设仍存在且输入未变化；失败不覆盖现有表情库。
- [ ] 注册命令并封装前端 API；验证消息不重复写入、跨会话提交被拒绝和迟到事件不串入新会话。

### Task 8.5: 聊天窗

**Files:** `src/windows/ChatWindow.tsx`、`src/components/Bubble.tsx`、`src/api.ts`。

- [ ] 实现当前人设的会话列表、新建、切换与消息查看；当前会话以 Rust 返回值为准，不在前端另选一个默认会话。
- [ ] 消息区使用稳定消息 ID 与请求 ID 区分已保存消息和流式临时内容；发送只调用 Task 8.4 的统一命令，不再次保存用户或 AI 消息。
- [ ] 发送期间提供暂停按钮；中断后刷新后端已保存的正文和回合状态。错误保留中文提示，不把失败当作正常完成。
- [ ] 接入响应式布局、主题、输入区和背景层。会话切换、窗口关闭前取消当前请求，卸载事件监听；关闭窗口不会删除已经保存的数据。
- [ ] 记忆上下文在阶段 9 接入同一后端流程；删除、编辑与重说在阶段 10 按确认规则接入，不另建消息保存入口。
- [ ] 验证重复发送、切换、关闭、失败和中断；运行前端测试与构建。

### Task 8.6: 设置窗

**Files:**
- Create: `src/windows/SettingsWindow.tsx`
- Create: `src/theme.ts`
- Create: `src/styles/theme.css`
- Create: `src/styles/base.css`
- Create: `src/styles/components.css`
- Create: `src/styles/motion.css`

- [ ] **Step 1: 编写设置窗**

按《前端设计文档》第 7 节建立完整两级导航，游戏中心与插件管理使用独立入口。各页复用 Rust 权威状态和结构化错误，不从示例数据推断当前项。

- 提供商：新增、编辑、测试、聊天当前项和记忆专用项选择；使用中禁止删除，列表仅返回 `has_api_key`，不返回明文或密文密钥。
- 人设：新建、编辑、切换、默认项保护与删除影响提示；删除当前项后使用后端返回的默认人设和会话刷新全部窗口。
- 提醒：规则编辑与本次触发记录操作分开；完成/过期通过 occurrence ID 提交，不直接把重复规则设为 done 或 expired。补提醒汇总由启动查询获取，并在展示后确认。
- 其余页面按各阶段接入，尚未实现的入口明确展示不可用状态；不伪造功能完成结果。
- `settings-layout`、`settings-navigation` 和 `settings-content` 使用容器宽度响应式重排，支持窄窗折叠导航、表单换行和固定主要按钮。

- [ ] **Step 1.1: 补齐提供商设置与提醒状态交互**

提供商页面分为“聊天与记忆整合模型”和“Embedding 模型”。聊天提供商列表接入 update_provider，编辑已保存项目；密钥留空保留、清除明确区分。添加独立记忆提供商选择器和“跟随聊天提供商”。正在被聊天或记忆整合使用的提供商禁止删除；当前联网 Embedding 必须先切换，内置本地 `BAAI/bge-small-zh-v1.5` 永远不可删除。没有用户提供商时显示新增引导。切换和索引重建逻辑由 Task 9.5 完成。
提醒页面接入规则和 occurrence：列出提醒名、详情、本次原定时间、下一次时间；完成和手动状态修改必须传 occurrence ID，删除才停止重复规则。启动后从数据库读取离线过期汇总，不能仅监听事件。

- [ ] **Step 1.2: 实现主程序亮色与暗色主题**

通用页提供“跟随系统”“亮色”“暗色”，对应 `theme_mode=system/light/dark`，默认 `system`。`src/theme.ts` 在每个主程序窗口初始化时读取设置：固定模式直接应用，`system` 使用 Windows/WebView 系统主题并监听变化。保存设置后广播 `theme_changed`，主宠窗、聊天窗、设置窗、记忆窗、提醒气泡和确认弹窗立即刷新，不要求重启。

`src/styles/theme.css` 作为主题入口，定义并导出主程序语义变量，包括背景、表面、主要文字、次要文字、边框、强调色、危险色和阴影；`base.css`、`components.css`、`motion.css` 分别承载基础排版、公共组件和动效规则。组件只使用这些公共定义，不散写仅适合单一主题的颜色，具体 Token、状态颜色和组件外观遵守《前端美化风格设计》。聊天背景图与人设本体颜色继续按原规则叠加，并在两套主题下验证文字可读性。游戏中心作为主程序页面可以使用主程序主题，但打开后的小游戏插件 WebView 不接收 `theme_mode`、颜色变量或主题变化事件，插件界面由插件自行设计。

- [ ] **Step 2: 构建验证**

Run: `npm run build`
Expected: 无错误；三个主题模式可即时切换，跟随系统时响应 Windows 主题变化，各主程序窗口一致，小游戏插件视觉不受影响。


---

## 阶段 9：记忆与记事本（会话级）

目标：实现聊天回合对应短期记忆、`k+1` 长期整合、可追溯来源与回退、长期记忆混合 RAG、会话级上下文和记忆小窗。完整规则以 `docs/记忆与旅行游记设计.md` 为准。

### Task 9.1: 扩展记忆模型与全局设置

**Files:** `src-tauri/src/models.rs`、`db.rs`、`config.rs`。

- [ ] 按 `db/schema.sql` 增加 `ShortMemory`、带 `title` 的 `LongMemory`、`LongMemorySource`、`LongMemoryEmbedding`、`Notebook` 模型，并让消息模型携带 `round_id`。
- [ ] `ShortMemory` 包含来源回合和 `pending/integrated` 状态；`LongMemory` 包含简短标题、实际来源数量和触发回合。覆盖边界从 `long_memory_sources` 最大 `source_order` 对应的短期记忆及回合推导，不保存重复字段。
- [ ] `AppSettings` 按《数据库与持久化设计》第 2.7 节增加短期记忆字数范围（默认 `12–100`）、长期记忆字数范围（默认 `150–250`）、`memory_merge_count`（默认 20）、`recent_message_count`（默认 30）、`recent_long_memory_count`（默认 30）、旅行游记字数范围（默认 `250–350`）、工具轮数和记事本上限；初始化、恢复默认值、前端表单与后端成对校验使用同一组数值。
- [ ] 设置读写统一校验正整数与最小值不大于最大值；修改 `memory_merge_count` 只作用于以后尚未整合的短期记忆。Rust 不暴露任意 key/value 写入命令，前端 DTO 使用明确字段。
- [ ] 运行 Rust 编译与设置往返测试。

### Task 9.2: 记事本与只读旅行游记工具

**Files:** `src-tauri/src/ai/client.rs`、`ai/streaming.rs`、`notebook.rs`、`travel_journal_tools.rs`、`chat.rs`、`lib.rs`。

- [ ] 保留 `read_notebook` / `update_notebook` 结构化工具和现有工具轮次上限；工具结果提供 `limit_chars`、`current_chars`、`remaining_chars` 和 `over_limit`，字符数按 Unicode 字符计算。
- [ ] `notebook_max_chars` 默认 `4000`，仅作为 AI 维护软限制：系统提示要求整份记事本保持在范围内，接近上限先合并和精简；超限内容照常完整保存且不截断，后续 AI 修改优先压缩。前端显示当前字数、建议上限和超限提示。
- [ ] 增加只读工具：列出当前会话旅行游记、按条件查找以前旅行、读取指定游记正文。
- [ ] 所有工具由后端注入当前 `session_id` 并校验归属，模型参数不能绕过会话边界。
- [ ] 流式解析正确拼接跨数据块的 tool_calls，按调用 ID 返回工具结果。

### Task 9.3: 每轮短期记忆生成

**Files:** `src-tauri/src/memory.rs`、`chat.rs`、`models.rs`、`commands.rs`、`lib.rs`。

- [ ] 每个 `completed` 回合生成且只生成一条短期记忆；中断回合和主动消息不生成。
- [ ] 生成输入固定为该回合的用户消息和 AI 回复，并使用以下原文作为系统约束：**短期记忆和长期记忆是中性事实记录，不使用人设口吻，不需要任何氛围总结或主观评价，必须只是如同剧本一样的叙事描述。**
- [ ] 读取回合、消息与 `memory_revision` 快照后释放锁请求 AI；返回后重新校验并用短事务写入。快照过期时拒绝写入并立即按最新正文重新排队；若此时关闭应用，启动扫描仍要通过“completed 回合没有 short_memory”发现并补做，不能让该轮永久缺少记忆。
- [ ] 短期记忆初始为 `pending`，整合后只改为 `integrated`，不删除。
- [ ] 验证失败重试、重复调度、消息在生成期间被编辑和会话被删除；按回合顺序补做缺失短期记忆，不跨过未补齐的完整回合进行长期整合。

### Task 9.4: `k+1` 长期整合与回退关系

**Files:** `src-tauri/src/memory.rs`、`models.rs`、`commands.rs`。

- [ ] 当同一会话出现第 `k+1` 条 `pending` 短期记忆时，按来源回合顺序固定取此前连续 `k` 条生成长期记忆；第 `k+1` 条保持 `pending`。积压逐组处理，不按 AI 完成时间分组；提交前校验本次采用的 k、来源与版本。
- [ ] 同一次 AI 请求生成长期记忆的非空标题与正文；在一个事务中插入长期记忆、按顺序写入 `long_memory_sources`，并把来源短期记忆改为 `integrated`；保存 `source_count` 和 `activated_by_round_id`。实际覆盖边界按最大 `source_order` 的来源短期记忆推导。
- [ ] 删除触发回合时，在同一事务先读取来源 ID、恢复来源短期记忆为 `pending`，再删除对应长期记忆及级联关联，最后删除触发回合。
- [ ] 正常整合不删除任何短期或旧长期记忆。不得实现长期记忆的自动上限、二次合并或上下文全量注入；全文索引由现有触发器在同一事务更新；提交后只调度向量生成。
- [ ] 验证 `k=20` 默认值、修改 k 后的未来行为、触发回退、快照过期和事务回滚。

### Task 9.5: Embedding 提供商与长期记忆混合检索

**Files:** `src-tauri/src/embedding_providers.rs`、`embedding.rs`、`memory_search.rs`、`models.rs`、`db.rs`、`commands.rs`、`lib.rs`、`src/windows/SettingsWindow.tsx`、`src/api.ts`，以及随应用打包的 `resources/models/bge-small-zh-v1.5/`。

- [ ] 首次启动幂等写入并启用不可删除的内置本地 `BAAI/bge-small-zh-v1.5`；模型资源随便携版打包，验证离线向量生成、维度与相同输入稳定性。
- [ ] 增加 Embedding 提供商列表、新增、编辑、删除、切换和测试命令；联网项使用 OpenAI-compatible `/embeddings`，API Key 经 DPAPI 加密，当前项切换使用事务。
- [ ] 创建并维护 `long_memories_fts`，使用 FTS5 `trigram` 索引标题与正文；为少于三个字符的中文检索词提供普通匹配兜底，并验证插入、编辑、重新整合、删除与 rebuild。
- [ ] 创建 `long_memory_embeddings`，以归一化 float32 BLOB 保存当前模型向量和内容哈希；新增长期记忆后异步生成，标题/正文变化使旧向量失效，模型切换和导入后支持可恢复的后台重建。
- [ ] 每次在当前会话的最近 `m` 条之外并行执行 FTS5 与向量候选检索，使用排名融合、ID 去重、相关阈值、数量上限和上下文预算筛选；第一版直接在 Rust 中计算当前会话向量相似度，不引入向量数据库。
- [ ] 联网 Embedding 失败时本次降级为 FTS5，记录可见错误但不让聊天进入休眠；测试本地/联网切换、部分向量待建、跨会话隔离和降级路径。

### Task 9.6: 上下文组装与深层回忆工具

**Files:** `src-tauri/src/context.rs`、`memory_tools.rs`、`chat.rs`、`commands.rs`、`lib.rs`。

- [ ] 所有上下文查询限定当前 `session_id`。日常聊天和无聊主动消息复用同一组会话级上下文，主动消息只增加主动触发指令。
- [ ] 最近消息原文按 `recent_message_count` 读取，默认 30，并确保当前用户消息只出现一次。
- [ ] 只加入 `pending` 且其原回合消息已离开最近原文范围的短期记忆；`integrated` 短期记忆不直接加入。
- [ ] 最近长期记忆按 `recent_long_memory_count` 读取，默认 30；标题和正文直接加入。较旧长期记忆只加入混合检索命中项，并与最近项按 ID 去重。
- [ ] 加入当前会话最近 5 篇旅行游记标题，正文只允许通过只读工具按需读取。
- [ ] 提供只读 `search_long_memories`、`open_long_memory`、`open_long_memory_sources`、`open_chat_round`；后端注入并校验当前 `session_id`，支持从候选标题逐层打开长期记忆、来源短期记忆和原始回合。
- [ ] 验证普通聊天、一次混合检索后回复和多轮深层回忆三条路径，确保前两条只调用一次聊天 AI。

- [ ] 上下文超过容量时，按《记忆与旅行游记设计》第 6 节缩减本次较旧内容并通知前端，不改数据库记忆或用户设置；测试固定部分超限、缩减后可发送以及最低必要内容仍超限。

### Task 9.7: 记忆命令与管理小窗

**Files:** `src-tauri/src/memory_commands.rs`、`commands.rs`、`src/windows/MemoryWindow.tsx`、`src/App.tsx`、`src/api.ts`。

- [ ] 短期/长期记忆提供列表和手动编辑；长期记忆卡片显示标题并可编辑标题与正文；不提供手动新建、删除或多条长期记忆合并命令。
- [ ] 短期记忆提供“重新生成”，严格读取绑定回合；长期记忆提供“重新整合”，严格读取原 `long_memory_sources` 并同时生成标题和正文。
- [ ] AI 结果先返回可编辑预览；确认时携带记录、来源和版本，过期则要求重新预览，覆盖后 ID 与来源不变。
- [ ] 编辑或重新生成已整合短期记忆时提示长期记忆不会自动变化，并定位可重新整合的相关长期记忆。
- [ ] 记忆小窗跟随 Rust 当前会话，短期项显示回合与整合状态，长期项显示来源范围、由最大 `source_order` 推导的来源末轮和触发回合；旅行游记不在该窗口展示。
- [ ] 长期记忆编辑与重新整合在提交事务内通过触发器同步 FTS5，并使旧向量失效，提交后异步补建；向量未完成时仍可使用关键词检索。
- [ ] 运行前端构建和 Rust 检查。

### Task 9.8: 聊天请求、中断与可靠保存

**Files:** `src-tauri/src/chat.rs`、`ai/client.rs`、`ai/streaming.rs`、`state.rs`、`commands.rs`、`src/windows/ChatWindow.tsx`、`src/api.ts`。

- [ ] 为聊天请求分配 request_id，同一会话只允许一个执行中的请求；事件携带 session_id 与 request_id。
- [ ] 暴露取消命令并覆盖表情请求、正文流与工具边界；切换/删除会话或关闭聊天窗时取消当前请求。
- [ ] 正常流程：事务建立 pending 回合并保存用户消息 → 组装上下文 → 生成表情和正文 → 事务保存 AI 消息并完成回合 → 增加亲密度 → 后台生成本轮短期记忆。
- [ ] 中断保留已经收到的正文一次，无正文不插空消息；回合不算完整，不增加亲密度、不生成人设自调或短期记忆，已提交工具操作不回滚。
- [ ] 前端不重复保存用户或 AI 消息；窗口事件丢失不影响后端持久化。
- [ ] 验证取消时机、晚到事件、重复发送、工具循环和正文仅保存一次。

---
## 阶段 10：会话与消息操作、默认人设、自启

目标：补全会话删除/标题、消息删除/编辑/重说、用户设定 CRUD 与会话绑定、默认人设、开机自启与正式应用名。

### Task 10.1: 会话删除与标题

**Files:**
- Modify: `src-tauri/src/sessions.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/windows/ChatWindow.tsx`
- Modify: `src/api.ts`
- Modify: `src/types.ts`

- [ ] **Step 1: 添加删除会话与标题命令**

`src-tauri/src/sessions.rs` 增加可序列化的 `SessionDeleteResult { current_session_id: Option<i64> }`、`delete_session(conn: &mut Connection, session_id: i64) -> Result<SessionDeleteResult, String>` 和 `rename_session(conn: &Connection, session_id: i64, title: &str) -> Result<(), String>`。

`delete_session` 使用一个事务校验会话存在并读取所属人设，先删除目标会话的长期记忆以解除短期来源上的 RESTRICT 引用，再删除会话及剩余级联数据；若删的是 `settings.current_session_id`，则从同一人设剩余会话中选择最近更新项并写回，没有剩余项时删除该设置键。小游戏插件、授权、设置、存档、游戏局与喵币账单均为全局数据，不参与删除事务。数据库函数返回新的当前会话 ID；命令提交后递增内存中的 context revision，并广播 `window_context_changed` 供所有已打开窗口刷新。调用命令前先中断该会话正在执行的聊天请求。

- [ ] **Step 2: 暴露命令**

`src-tauri/src/commands.rs` 暴露 `delete_session(session_id) -> SessionDeleteResult` 与 `rename_session(session_id, title)` 并注册到 `lib.rs`。删除命令依次中断目标会话的聊天请求、调用上述事务函数、递增 context revision、广播 `window_context_changed`，再把新的当前会话 ID 返回前端；任一步在数据库提交前失败都不得改变当前会话。

- [ ] **Step 3: 会话标题 AI 生成**

首次完整回复后，若标题仍为“新会话”，读取会话 ID、首条用户消息和当前标题快照，再在锁外请求不超过 12 个中文字符的标题。使用明确的文本响应或结构化 `{title}`，不能把 JSON 文本直接作为标题。提交前重查会话存在、标题仍等于快照；用户已经重命名则丢弃生成结果。空结果、取消或请求失败保留原标题，不影响已完成聊天。

- [ ] **Step 4: 聊天窗会话与聊天记录查看**

`ChatWindow.tsx` 展示当前人设下的会话列表，可切换会话、查看历史消息、重命名、删除。删除前确认框明确列出会话级聊天、记忆、旅行、节日、饰品、信件和伴手礼会被删除，并说明全局小游戏插件及其数据不受影响。确认后先中断该会话流式请求，再调用删除命令并用返回的当前会话刷新全部会话级窗口。

- [ ] **Step 5: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。


### Task 10.2: 消息编辑、尾部删除与重说

**Files:** `src-tauri/src/sessions.rs`、`memory.rs`、`chat.rs`、`commands.rs`、`src/windows/ChatWindow.tsx`、`src/api.ts`。

- [ ] 编辑任意历史消息只更新原文和 `memory_revision`，不自动修改或删除短期记忆、长期记忆或旅行游记；尚未生成短期记忆的回合以后使用编辑后正文。
- [ ] 删除只允许从聊天底部执行。最后完整回合在 AI 消息下显示“删除本轮”，一个事务删除两条消息、回合和对应短期记忆；底部只有待回复用户消息时删除该消息与未完整回合。
- [ ] 底部为中断回合时允许整轮删除用户消息、部分 AI 回复与回合；底部为主动消息（round_id=NULL）时只删除该条。两者均不修改已有记忆，不得越过底部主动消息删除更早回合。删除前取消未结束请求，事务内重验底部、会话与版本，成功后递增 memory_revision；迟到结果不得重新写入。
- [ ] 验证有部分正文的中断回合、无 AI 正文的未完整回合、底部主动消息、越过底部删除被拒绝，以及删除与迟到回复竞态；确认中断回合与主动消息删除均保留已有记忆。
- [ ] 若被删回合是 `activated_by_round_id`，同一事务先读取 `long_memory_sources` 的来源 ID 并恢复短期记忆为 `pending`，再删除相应长期记忆及其级联关联，最后删除目标回合；操作成功后刷新消息与记忆窗口。
- [ ] `regenerate_last_reply` 只接受最后完整回合，组装上下文时排除旧 AI 回复并保留用户消息一次。
- [ ] 重说成功后保持原回合和消息 ID，更新 AI 回复、表情和本轮短期记忆；通过 `long_memory_sources` 找到并重新生成受影响长期记忆，提交前校验版本，最终替换使用一个短事务。
- [ ] 重说失败或中断保留旧结果，不重复增加亲密度，不重放已有有副作用工具。
- [ ] 验证尾部限制、触发回退、编辑不联动、重说涉及/不涉及长期记忆、生成失败和并发版本变化。

### Task 10.3: 开机自启与默认人设

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `package.json`
- Create: `src-tauri/src/autostart.rs`

- [ ] **Step 1: 添加 autostart 插件**

使用与当前 Tauri 2 兼容的 autostart 插件，按实际锁定版本的 Windows API 注册并配置最小 capabilities；不复制其他平台的初始化示例。前端封装启用、关闭和查询，界面保存值以系统实际结果为准。便携目录移动后检查注册路径并提示修复，不继续显示无效的“已开启”。验证开启后重新登录启动、关闭后不启动、系统写入失败不假报成功。

- [ ] **Step 2: 默认人设种子**

复用阶段 2 的种子服务，以 `is_builtin=1` 校验内置默认人设，而不是只判断 personas 总行数是否为零。发现有普通人设却没有默认人设时补齐默认项；多个内置项属于数据不一致，不随意删除。默认人设不可删除，删除当前非内置人设须调用统一删除服务并切回默认。导入新建副本不得复制内置身份。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。


### Task 10.4: 正式应用名

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `index.html`
- Modify: `package.json`

- [ ] **Step 1: 更新产品名与标识**

`src-tauri/tauri.conf.json`：`productName` 改为 `"颜喵 YanMew"`，`identifier` 改为 `"com.yanmew.pet"`，主窗口 `title` 改为 `"颜喵 YanMew"`。
`index.html` 的 `<title>` 改为 `颜喵 YanMew`。
`package.json` 的 `name` 改为 `"yanmew"`。

- [ ] **Step 2: 构建验证**

Run: `npm run tauri build`
Expected: 构建成功；可执行文件名由 Cargo target 决定，当前为 `src-tauri/target/release/yanwenzi.exe`，修改 productName 不等于修改二进制文件名。完整便携包按收尾流程另行组装。

---

### Task 10.5: 宠窗默认位置与位置记忆

实施位置：窗口服务、`config.rs` 和现有 `lib.rs`。

- [ ] 使用一致的物理像素坐标保存和恢复窗口移动事件；不能把 `WindowEvent::Moved` 的物理坐标直接交给逻辑坐标 API。
- [ ] 首次启动按当前显示器工作区与真实窗口尺寸定位，避开任务栏；允许多显示器负坐标，不把所有坐标简单截为非负。
- [ ] 恢复前验证窗口与当前显示器工作区相交；原显示器移除时将窗口移回可见区域。尺寸、DPI 和工作区均从运行环境获取。
- [ ] x/y 在同一事务保存；拖动时节流保存，退出补存。保留现有拖动和隐藏恢复逻辑，宠窗内部游走不更新窗口位置。
- [ ] 验收：默认位置、重新启动、负坐标副屏、不同缩放、拔掉副屏、任务栏位置改变，确认恢复后可见且可拖动。

### Task 10.6: 用户设定 CRUD 与会话绑定

**Files:**
- Create: `src-tauri/src/user_profiles.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/sessions.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/windows/SettingsWindow.tsx`
- Modify: `src/windows/ChatWindow.tsx`
- Modify: `src/api.ts`
- Modify: `src/types.ts`

- [ ] **Step 1: 建表与迁移**

user_profiles 与 sessions.user_profile_id 已由完整 schema 建立；本 Task 接入模型和 CRUD。旧库升级仅由 Task 2.1 版本化迁移处理，不在功能模块重复 ALTER TABLE。

`models.rs` 增加 `UserProfile`；`sessions.rs` 的 `Session` 增加 `user_profile_id: Option<i64>`，`list_sessions` 同步读取该字段。

- [ ] **Step 2: 实现用户设定 CRUD**

`src-tauri/src/user_profiles.rs` 提供：

```rust
pub fn create(conn: &Connection, name: &str, content: &str) -> Result<i64, String>
pub fn list(conn: &Connection) -> Result<Vec<UserProfile>, String>
pub fn update(conn: &Connection, id: i64, name: &str, content: &str) -> Result<(), String>
pub fn delete(conn: &Connection, id: i64) -> Result<(), String>
```

`delete` 依靠 `sessions.user_profile_id` 的 `ON DELETE SET NULL` 自动把绑定它的会话置为“未绑定”，不删除会话。`commands.rs` 暴露 `create_user_profile`、`list_user_profiles`、`update_user_profile`、`delete_user_profile`，并在 `lib.rs` 注册。

- [ ] **Step 3: 会话绑定**

`create_session(conn, persona_id, user_profile_id: Option<i64>)` 增加可选绑定参数；`list_sessions` 返回 `user_profile_id`。`commands.rs` 增加 `set_session_user_profile(session_id, user_profile_id)`，允许用户在聊天窗修改绑定；未绑定传 `None`。

- [ ] **Step 4: 设置窗用户设定页**

`SettingsWindow.tsx` 将“用户设定（Task 10.6 补齐）”占位替换为 `<UserProfilesTab />`，实现新建、编辑、删除、导出用户设定；删除前提示绑定它的会话会变为“未绑定”。

- [ ] **Step 5: 聊天窗绑定与展示**

`ChatWindow.tsx` 新建会话时允许选择用户设定或留空；会话行显示“用户设定：未绑定/名称”；会话详情中提供“修改绑定”。`api.ts`、`types.ts` 同步命令和类型。

- [ ] **Step 6: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Run: `npm run build`
Expected: 无错误。


---

## 阶段 11：体验完善

目标：落实表情库自动生成、喂食计时、饿/无聊气泡、无聊主动消息、桌面游走、提醒编辑、测试连接、短期记忆游标。

### Task 11.1: 表情库自动生成与手动生成

**Files:**
- Modify: `src-tauri/src/persona.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: 创建人设后自动生成**

`src-tauri/src/commands.rs` 的 `create_persona` 改为 async，创建成功后调用 `crate::expressions::generate_persona_expressions`（使用聊天当前 provider；若无 provider 则跳过不报错），结果写入该人设 `expressions`。设置页的“生成表情库”按钮保留，复用 `generate_expressions` 命令。

- [ ] **Step 2: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。


### Task 11.2: 喂食计时与饿/无聊气泡

**Files:**
- Modify: `src-tauri/src/pet.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: 分离喂食计时**

`PetRuntime` 增加 `last_feed: Mutex<Instant>`。`register_interaction` 的 `feed` 分支同时更新 `last_feed` 与 `last_interaction`。饥饿判断使用 `last_feed.elapsed()`，无聊/睡觉使用 `last_interaction.elapsed()`。

- [ ] **Step 2: 状态气泡事件**

`start_pet_loop` 中当状态变为 `Hungry` 或 `Bored` 时，若与上一次发出的状态不同，通过 `EventSink` 发布 `pet-state-bubble` 业务事件，Windows 事件适配器负责 Tauri 推送；前端只显示一次状态提示；无聊主动消息统一由 Task 11.3 后台概率任务触发，避免两条路径重复请求 AI。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。


### Task 11.3: 无聊主动消息

**Files:**
- Modify: `src-tauri/src/pet.rs`
- Create: `src-tauri/src/proactive.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 编写无聊主动消息生成**

`src-tauri/src/proactive.rs` 增加 `generate` 业务入口，接收固定上下文、取消信号及数据库/AI/事件等所需服务引用，不接收 `AppHandle`；平台事件适配负责实际推送：
1. 取当前人设（`personas.is_active`）。
2. 只取 `current_session`；没有当前会话时本次不触发，不回退到该人设最近会话。
3. 有当前会话时调用日常聊天共用的 `context::assemble`，读取会话所属人设、绑定用户设定、消息、短期与长期记忆、记事本、最近 `5` 篇旅行游记标题、`get_environment()` 得到的环境文本和当天节日；仅把触发指令替换为主动消息指令，不伪造用户消息。
4. 用聊天 provider 生成简短消息；提交前复核当前会话、上下文版本和可主动发送状态，在事务内保存 `round_id=NULL` 的 AI 消息并更新版本，提交后再发 `proactive-message`。切换会话后的迟到结果丢弃，不显示或写入新会话。

- [ ] **Step 2: 按无聊状态与概率触发**

`src-tauri/src/pet.rs` 的桌宠循环中，状态为 `Bored` 时按固定概率尝试触发一次 `proactive::generate`；用冷却时间与“同一段无聊状态只发一条”避免刷屏。休眠、出门中、隐藏宠窗、正在聊天时不触发。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。


### Task 11.4: 提醒编辑、测试连接与待生成记忆恢复

**Files:**
- Modify: `src-tauri/src/reminders.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/providers.rs`
- Modify: `src-tauri/src/memory.rs`
- Modify: `src-tauri/src/db.rs`

- [ ] **Step 1: 提醒编辑命令**

`src-tauri/src/reminders.rs` 增加 `update_reminder(conn, id, name, kind, schedule, content, fire_at, status)`；`commands.rs` 暴露 `update_reminder`。

- [ ] **Step 2: 提供商测试连接**

`src-tauri/src/providers.rs` 增加 `test_connection(target: &ApiTarget) -> Result<(), String>`：发送使用所选模型的最小 chat 请求并验证响应；仅 models 列表可访问不能证明模型可用。`commands.rs` 暴露 `test_provider_connection(input)` 临时构造 target 测试。

- [ ] **Step 3: 待生成记忆恢复**

`memory.rs` 扫描尚无短期记忆的 `completed` 回合，并按回合 ID 顺序补生成；`short_memories.round_id` 唯一约束阻止重复。中断回合与主动消息不参与。恢复任务继续遵守 `memory_revision` 快照校验；版本冲突不是完成状态，工作器立即重排最新任务，启动扫描负责补偿进程退出造成的遗漏。长期整合冲突时来源短期记忆保持 `pending`，后续重新整合。

- [ ] **Step 4: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。


### Task 11.5: 桌面游走

**Files:**
- Modify: `src-tauri/src/pet.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/windows/PetWindow.tsx`

- [ ] **Step 1: 小范围游走**

pet.rs 低频计算固定画布内的颜文字偏移，通过事件发送给前端；前端 transform 只移动颜文字及随附饰品、气泡。禁止用窗口 set_position 实现游走，窗口位置仅由用户拖动改变。

- [ ] **Step 2: 避开前台窗口与靠近鼠标**

将鼠标与前台矩形转换到画布坐标；仅在画布内有可用位置时避让，鼠标靠近画布时才在边界内靠近。无可用位置则停止，不移动整个窗口。位移后同步点击穿透命中区域。

- [ ] **Step 3: 暂停条件**

用户拖动、点击、打开聊天、睡觉、出门中、隐藏宠窗时暂停游走；条件解除后自动继续。

- [ ] **Step 4: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Run: `npm run build`
Expected: 无错误。


---


---

### Task 11.6: 休眠恢复与错误分流

**Files:** `src-tauri/src/pet.rs`、`providers.rs`、`commands.rs`、`src/components/ErrorBadge.tsx`、`src/windows/SettingsWindow.tsx`。

- [ ] 普通计时循环保持 Dormant，不用 Normal/Hungry 等状态覆盖；后台记忆和导入错误仅提示，不进入休眠。
- [ ] 暴露 refresh_pet/recover_pet，错误气泡显示“刷新”；配置有效聊天提供商或其连接测试成功触发同一恢复入口。
- [ ] 休眠期间设置、刷新、测试连接可用；恢复失败保留中文错误，成功清除主流程错误并恢复活动，不自动重发历史请求。
- [ ] 连接测试必须实际验证所选模型可用，不能只凭 models 列表返回成功判断聊天恢复。
- [ ] 验证计时不会解除休眠、刷新成功/失败、切换有效提供商、后台记忆失败不休眠。

---

## 阶段 12：人设自调与初始设定

目标：当前人设与初始设定分离；聊天回复完成后由 AI 自判是否调整当前人设；用户可保存和恢复初始设定，并在前端查看当前人设相对初始设定的累计差异。

### Task 12.1: 人设数据模型与 CRUD 扩展

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/persona.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: 更新人设建表与模型**

personas 中 initial_prompt、self_evolve_enabled、last_evolved_at、color 已由完整 schema 创建，本 Task 接入模型与命令；模型字段与 `db/schema.sql` 保持一致。`create_persona` 时把 `initial_prompt` 设为用户填写的 `prompt`，`self_evolve_enabled` 默认 1，`color` 默认 `#ffb6c1`。

- [ ] **Step 2: 增加人设命令**

`commands.rs` 暴露：获取人设详情、更新当前人设、保存当前人设为初始设定、从初始设定恢复当前人设、读取和更新当前人设颜色。恢复操作在 Rust 层做事务更新，不调用 AI。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。


### Task 12.2: 聊天后人设自调

**Files:**
- Modify: `src-tauri/src/chat.rs`
- Modify: `src-tauri/src/ai/client.rs`
- Modify: `src-tauri/src/persona.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: 增加人设自调工具**

为完整回复后的自调阶段提供 `update_persona_prompt(new_prompt)`；不在尚可能中断的正文工具循环中直接修改人设。读取固定目标人设及当前 prompt 快照，提交前复核目标仍存在、开关仍开启、prompt 未被用户修改；成功才写入 prompt 与 last_evolved_at，initial_prompt 不变。

- [ ] **Step 2: 接入回复完成流程**

仅正常 completed 后执行自调阶段，AI 自行判断是否调整；失败不撤销聊天结果。写入成功后发出 `persona-updated` 并通知设置窗刷新；目标已非当前人设时不把提示展示成当前人设发生了变化。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。


### Task 12.3: 人设自调设置界面

**Files:**
- Modify: `src/windows/SettingsWindow.tsx`
- Modify: `src/api.ts`
- Modify: `src/types.ts`
- Modify: `src/components/Kaomoji.tsx`
- Modify: `src/windows/PetWindow.tsx`
- Create: `src/components/PersonaPromptDiff.tsx`
- Create: `src/utils/personaPromptDiff.ts`
- Test: `src/utils/personaPromptDiff.test.ts`

- [ ] **Step 1: 展示人设当前值与初始设定**

人设卡片显示当前人设、初始设定、自调开关和最近调整时间，并提供“查看与初始设定的差异”入口。差异只使用详情中已有的 `prompt` 和 `initial_prompt`，不增加后端接口或数据库字段。

- [ ] **Step 2: 实现本地差异计算与差异视图**

`personaPromptDiff.ts` 先按段落和句子对齐，再细化发生变化的片段；统一换行并忽略只由行尾空白造成的变化。`PersonaPromptDiff.tsx` 提供行内对比和左右对比，窄屏自动使用行内或上下布局。新增片段使用绿色背景并带可读的“新增”标记，删除片段使用红色背景、删除线并带可读的“删除”标记，未改变片段正常显示。颜色不得是唯一识别方式，标记提供屏幕阅读器文本，亮色和暗色主题均验证可读性。

视图标题使用“当前与初始设定的累计差异”，说明其中也包含用户手动编辑，不能称为 AI 修改历史。两个文本一致时显示“当前人设与初始设定一致”。计算不调用 AI、不持久化差异、不修改 `last_evolved_at`。

- [ ] **Step 3: 增加保存与恢复操作按钮**

提供“保存为初始设定”“从初始设定恢复”按钮，两者均在执行前弹确认框。“保存为初始设定”明确提示会把当前内容设为新的比较基准并清空累计差异；恢复操作提示会以初始设定覆盖当前内容。成功后刷新人设详情，差异视图应变为一致状态。

- [ ] **Step 4: 增加颜色选择与渲染**

人设卡片提供预设色板 + 自定义颜色选择器；`Kaomoji.tsx` 和 `PetWindow.tsx` 使用当前会话所属人设的 `color` 渲染基础颜文字，饰品层、气泡和窗口背景不受影响。

- [ ] **Step 5: 测试与编译验证**

单元测试覆盖：完全一致、纯新增、纯删除、中文句内替换、多段文本、换行格式差异、空文本和超长文本；组件测试检查“新增/删除”文字标记、删除线语义、主题 class、窄屏布局与键盘可访问性。不得仅通过像素颜色断言含义。

Run: `npm run build`
Expected: 无错误。


---

### Task 12.4: 聊天窗背景设置与渲染

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/persona.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src/windows/ChatWindow.tsx`
- Modify: `src/windows/SettingsWindow.tsx`
- Modify: `src/api.ts`
- Modify: `src/types.ts`

- [ ] **Step 1: 扩展人设数据模型**

personas 的 chat_bg_file、chat_bg_opacity、chat_bg_blur 已存在，本 Task 接入模型与命令；默认无背景图，透明度默认 85，模糊度默认 0。

- [ ] **Step 2: 实现背景图上传命令**

Rust 命令接收文件名和字节内容，校验扩展名及实际可解码格式为 `jpg / png / webp`、单张不超过 10MB，使用后端生成的不冲突文件名写入 `data/chat_background/`，校验成功后在 `personas` 中保存相对文件名；提交前不删除旧图，失败不改变旧引用。前端只显示预览，不读原始路径。

- [ ] **Step 3: 实现背景设置读取与更新**

`persona.rs` 和 `commands.rs` 暴露当前人设聊天窗背景的读取与更新命令，包括透明度（0–100）和模糊度（0–40px）。

- [ ] **Step 4: 设置页增加背景配置**

设置 > 人设 > 对应人设卡片增加“聊天窗背景”区域：上传、预览、透明度滑块、模糊度滑块、恢复默认。

- [ ] **Step 5: 聊天窗渲染背景**

`ChatWindow.tsx` 在消息层下方渲染背景图，按当前人设的透明度和模糊度显示；无背景图时使用默认背景。

- [ ] **Step 6: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Run: `npm run build`
Expected: 无错误。

## 阶段 13：亲密度、全局签到、喵币与小游戏插件

目标：会话亲密度、应用唯一的全局每日签到、全局喵币和可安装的小游戏插件系统；石头剪刀布作为标准第一方插件随便携版提供。全局签到不属于小游戏插件。

### Task 13.1: 亲密度与货币数据层

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/models.rs`
- Create: `src-tauri/src/currency.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: 更新建表与初始化**

user_state、currency_ledger、sessions.intimacy 与 craft_tier_1_used/2_used/3_used 已由完整 schema 初始化。本 Task 接入模型；钱包单行由初始化事务写入。

- [ ] **Step 2: 实现喵币流水**

`currency.rs` 提供 `get_balance`、`apply_delta`、`count_today_reason`。所有余额变更与 `currency_ledger` 在同一事务中完成。

- [ ] **Step 3: 暴露钱包查询命令**

`commands.rs` 暴露 `get_wallet` 和 `get_session_intimacy`。

- [ ] **Step 4: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。


### Task 13.2: 互动增加亲密度

**Files:**
- Modify: `src-tauri/src/pet.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src/windows/PetWindow.tsx`

- [ ] **Step 1: 更新互动命令**

点击、摸摸、喂食命令成功时给当前会话亲密度 +1；AI 出门中时不增加。

- [ ] **Step 2: 聊天轮数增加亲密度**

一轮完整用户消息与 AI 回复完成时，在同一事务保存 AI 消息、completed 状态，并给该回合所属会话亲密度 +1。只允许 pending 到 completed 的首次转移发放；中断、重复完成通知和重说不增加。短期记忆任务在提交后启动。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。


### Task 13.3: 应用全局每日签到（非小游戏）

**Files:**
- Modify: `src-tauri/src/currency.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src/windows/SettingsWindow.tsx`
- Modify: `src/api.ts`

- [ ] **Step 1: 实现每日签到**

颜喵只保留这一套应用级签到。签到命令按本地日期判断当天是否已签到；无节日命中时随机发放 1–10 喵币，写入全局流水和余额。命中节日时的固定 10 喵币与祝福词在阶段 18 接入。“桌宠状态”显示余额、亲密度与签到入口；游戏中心和插件协议不提供签到入口、签到状态或签到流水写入能力。

- [ ] **Step 2: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Run: `npm run build`
Expected: 无错误。


### Task 13.4: 插件数据、权限与钱包事务

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/currency.rs`
- Create: `src-tauri/src/game_plugins.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: 接入现有插件 schema**

复用完整 schema 中已经存在的 `game_plugins`、`game_permissions`、`game_runs`、`game_plugin_settings`、`game_plugin_data` 和 `currency_ledger` 通用插件字段，补齐 Rust 模型、查询和仓储。阶段 2 已负责新库与迁移库结构一致；本步骤不得再写重复建表或临时 `ALTER TABLE`。首次启动幂等登记随包石头剪刀布插件，保留缺包插件的 `missing_package` 状态。

设置键只接受 manifest `settings_schema` 中声明的稳定键，值按其 boolean/integer/number/string/enum 类型、范围和选项校验；缺少数据库覆盖值时使用 manifest 默认值。私有存档使用插件自己的 `data_key → JSON value`，Rust 校验合法 JSON、容量、插件命名空间和 `storage_version`，不解释进度、统计、纪录、解锁或教程等业务结构。游戏局 `details_json` 仅保存战绩/诊断明细，不能替代钱包、账单或授权事实。

- [ ] **Step 2: 实现插件登记与权限仓储**

实现插件列表、版本登记、启用/停用、权限读取与修改。登记快照保存 manifest 声明的 `network_origins`；权限以 `(plugin_id, permission, permission_scope)` 唯一，状态为 `pending/allowed/denied`，`wallet.modify` 的 scope 为空，`network.fetch` 的 scope 是单个 HTTPS origin。`pending` 必须使用 `decided_at=NULL` 且宿主拒绝对应调用；允许或拒绝后才写决定时间。manifest 权限只是申请；用户授权独立保存。新增权限、origin 或独立插件数据导入不能继承旧授权。

- [ ] **Step 3: 实现游戏局生命周期**

提供创建、读取和完成游戏局的内部服务。每局固定绑定插件 ID、插件版本和开始时的钱包授权状态，不保存人设或会话 ID；重复完成保持幂等。

- [ ] **Step 4: 实现插件钱包接口**

`wallet.changeCoins()` 校验插件启用状态、版本、`wallet.modify` 授权、游戏局归属、唯一 `request_id`、整数 `delta` 和非负余额。在同一事务更新游戏局、插入账单和修改全局余额；重复请求不重复到账或扣款。`wallet.getBalance()` 只返回全局余额。

- [ ] **Step 5: 验证事务与迁移**

覆盖允许、拒绝、撤销、余额不足、重复请求、错误游戏局、插件版本不一致、权限 scope 隔离、迁移后约束和纯玩游戏局不写账单。


### Task 13.5: 插件宿主与本地安装

**Files:**
- Create: `src-tauri/src/game_host.rs`
- Modify: `src-tauri/src/game_plugins.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`
- Create: `plugins/sdk/`
- Create: `plugins/template/`
- Create: `plugins/build/`

- [ ] **Step 1: 固定 v1 manifest 与包格式**

实现 `.yanmew-game` 校验：必填字段、稳定插件 ID、语义版本、`api_version`、入口路径、权限、`network_origins`、文件类型、解压大小和路径越界检查。所有插件存档固定为应用全局，插件作者无需声明存档范围。联网来源只接受无路径、无通配符、无凭据的 HTTPS origin。社区分发包必须提供 `repository`，可选 `homepage` 与 `release_page`；本地开发或私人插件可以不提供来源链接。提供 SDK 类型、模拟宿主、模板和统一打包校验命令；不提供 `ai.generate()`。

- [ ] **Step 2: 实现原子安装与更新**

插件先解压到临时目录，完整校验后登记并原子切换。更新失败保留旧版本；新增权限或联网 origin 等待用户确认。普通本地插件卸载时可保留或清除私有存档，历史游戏局和账单始终保留。

- [ ] **Step 3: 实现受限 WebView 宿主**

只向插件暴露 `game.ready/getContext/random/complete/close`、`storage.get/set/delete`、`wallet.getBalance/changeCoins` 和受限 `network.fetch`。`game.getContext()` 返回 manifest 默认值与用户覆盖值合并后的只读插件设置；`storage.*` 只操作当前插件的数据键。不得暴露任意 Tauri command、SQLite、系统命令、API Key、聊天、记忆、记事本、人设、会话或主程序全局设置。按插件 ID 隔离全局存档和窗口实例。插件上下文不包含当前人设、当前会话、颜喵主题模式或颜色变量，插件 WebView 的视觉由插件自行负责。WebView CSP 禁止直接联网；宿主代发时校验 HTTPS origin、逐 origin 授权、重定向、超时、请求及响应大小，并拒绝本机回环与局域网私有地址。

- [ ] **Step 4: 暴露管理命令并验证**

覆盖合法包安装、损坏包、路径越界、不兼容 API、更新回滚、权限拒绝、新增联网 origin、未授权请求、跨 origin 重定向、存档隔离、窗口关闭和插件异常不影响主程序。


### Task 13.6: 随包石头剪刀布插件与游戏中心

**Files:**
- Create: `plugins/bundled-games.json`
- Create: `plugins/first-party/rock-paper-scissors/`
- Create: `src/components/GameCenterPanel.tsx`
- Create: `src/components/PluginManagerPanel.tsx`
- Modify: `src/App.tsx`
- Modify: `src/windows/SettingsWindow.tsx`
- Modify: `src/api.ts`
- Modify: `src/types.ts`
- Modify: `src-tauri/src/game_plugins.rs`
- Modify: `src-tauri/src/game_host.rs`
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: 实现标准石头剪刀布插件**

在 `plugins/first-party/rock-paper-scissors/` 内以固定插件 ID `com.yanmew.game.rock-paper-scissors` 实现界面、出拳、通过 `game.random()` 取得系统出拳、胜负计算和结果展示。每日最多 20 局。插件申请 `wallet.modify`；获授权时赢 +5、输 -4、平局 0 且开始前余额 ≥ 4，拒绝时进入不增减喵币且不要求最低余额的纯玩模式。

- [ ] **Step 2: 接入随包构建与首次登记**

统一插件构建流程生成 `.yanmew-game`，`bundled-games.json` 将石头剪刀布列为随包插件，Tauri 便携版资源包含同一个产物。启动时登记或升级随包版本；界面显示“随应用提供”，允许停用和清除私有存档，不允许永久卸载。

- [ ] **Step 3: 实现首次权限确认**

随包插件第一次打开时显示 `wallet.modify` 说明并保存允许或拒绝结果。撤销权限后拒绝尚未发生的钱包请求；已经成功提交的账单不回滚。

- [ ] **Step 4: 实现游戏中心**

在设置窗“游戏中心与插件管理”独立导航入口中展示随包和本地安装插件的名称、作者、版本、来源、简介、源码链接、全局存档状态、喵币权限、联网 origin 与逐项授权；支持启动、启停、本地安装/更新、权限管理、存档清理、可卸载插件卸载、战绩与相关账单，并提供“备份插件数据”“导入插件数据”入口。对备份恢复但缺少程序包的插件显示保留数据、插件 ID、原版本及来源链接，允许调用系统浏览器打开来源页并从本地重新安装；第一版不请求 GitHub API、不自动下载，也不显示“官方收录”。

- [ ] **Step 5: 完整验证**

验证开发构建、便携版资源、首次登记、应用升级、首次授权、纯玩模式、正常结算、每日次数、余额不足、停用、清除存档、本地插件安装和主程序重启恢复。

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Run: `npm run test`
Run: `npm run build`
Expected: 全部通过，石头剪刀布与外部本地插件都由同一宿主运行。


---

## 阶段 14：旅行与写信

目标：会话级旅行状态、出门/返回、写信与回信。

阶段 14–16 使用 travels 已定义的固定出行事实与实际礼物快照，输入、恢复和事务顺序遵守《数据库与持久化设计》第 6.1 节。

### Task 14.1: 当前会话旅行计时与状态

**Files:** `src-tauri/src/travel.rs`、`models.rs`、`commands.rs`、`state.rs`。

- [ ] 接入完整 schema 的 travels（出行记录表）和会话计时字段，不重复 ALTER TABLE。游记、信件、伴手礼与旅行饰品分别通过 travel_id 关联同一次出行，各子项之间不建立删除级联；删除子项不删除出行记录或其他子项。
- [ ] 会话只保存 `travel_trigger_remaining_seconds`，不同时保存累计值和目标阈值。创建会话、正常返回或取消旅行后按用户设置抽取一次，默认 2–4 小时；仅当前会话在家空闲时递减，聊天、互动、睡觉、休眠、隐藏宠窗、出门中暂停，其他会话和应用关闭期间暂停。
- [ ] 以 facts_state/destination/activities_json/facts_version/facts_ready_at 实现一次性出行事实生成；事务外请求，提交前验证状态，失败重试、取消丢弃、成功后不重造，途中只读取已到时间的活动。
- [ ] 出发事务建立 away 旅行，保存计划返回时间与 midway_letter_at；前端事件提交后发出。
- [ ] 正常返回事务设 returned，取消未完成途中信与待回信，确定 gift_kind，并抽取保存下一轮 2–4 小时倒计时。回来信、礼物、总结使用各自 pending/done 状态。
- [ ] 切换会话/人设时，尚未出发的旧会话只暂停并保留剩余倒计时；旧会话已有 away 旅行时，事务取消该旅行与待回信、记录 cancelled_at、把未完成旅行任务设 cancelled，并保存下一轮倒计时，不发返回结果。已存在记录保留。
- [ ] 验证多次重复返回、并发创建旅行、切换取消、仅当前会话计时；唯一索引防止同会话两条 away。

### Task 14.2: 关闭与启动恢复

**Files:** `src-tauri/src/travel.rs`、`lib.rs`、`config.rs`。

- [ ] 分批持久化剩余有效在线秒数，正常退出再保存；关闭期间不递减。扣减将到零时在同一事务创建 away 旅行并把会话倒计时设为 NULL，不单独保存零值。
- [ ] 启动先恢复 current_session_id，再恢复任务。未到期 away 继续，已到期先正常返回，实际返回时间采用计划返回时间；关闭不等于切换取消。
- [ ] 扫描所有 returned 的未完成生成状态进行补做，cancelled 不恢复；即使结果被用户删除，done 也不得重置。
- [ ] 返回与取消先于待回信补偿；旅行已到期就取消待回信，不补生成过期回复。
- [ ] 验证关闭未到期、关闭已到期、返回生成一半崩溃、取消后重启、结果删除后重启。

### Task 14.3: 信件与回信保存

**Files:** `src-tauri/src/letters.rs`、`travel.rs`、`models.rs`、`commands.rs`。

- [ ] 接入 reply_due_at、reply_state、has_been_replied、letter_kind；用户信到期时间首次随机抽取并保存，重启不重抽。
- [ ] midway/return 信分别在旅行到中点/正常返回后生成，均读取固定出行事实；return 信等待实际礼物快照，途中信仅描述已到时间的活动。结果和对应 done 标记在同一事务内提交，AI 请求在事务外。
- [ ] 用户写信/回复只允许当前会话 away 时；原信已回复过就拒绝，删除回复也不重置 has_been_replied。
- [ ] 用户回复 AI 信时，在事务内写用户信、标记原信已回复并设新信自动回信期限；AI 回信事务再次校验原信存在、旅行 away、状态 pending，再写回复并将原信标记 completed。
- [ ] 旅行返回、会话切换取消待回信；删除原信使任务失效，迟到响应不写入。重启仅补仍有效且已到期回信。
- [ ] 验证删除回复不能再次回、删除原信期间 AI 返回、回信与返回竞态、期限重启不变。

### Task 14.4: 出行与邮箱界面

**Files:** `src/windows/TravelWindow.tsx`、`LetterWindow.tsx`、`ChatWindow.tsx`、`PetWindow.tsx`、`SettingsWindow.tsx`、`src/api.ts`、`src/types.ts`、Rust 窗口命令与 capabilities。

- [ ] 出门中主宠窗显示“{人设名}出门啦”，聊天入口变写信；聊天窗显示状态面板与切换其他会话入口，注明切换取消旅行和待回信。
- [ ] 接入写信正文、回复目标、发送/取消、跳转邮箱；窗口路由、命令注册和能力声明一并完成。
- [ ] 出行页跟随当前会话，无独立会话选择器；历史展示出门中/已返回/已取消，取消不显示返回奖励。
- [ ] 邮箱支持查看、收藏、删除；写信/回信仅 away 时可用，已回复永不恢复资格，待回信取消显示明确状态。
- [ ] 验证切换人设/会话时窗口同步、取消文案、返回后按钮禁用、历史与邮箱状态；前后端构建通过。

---

## 阶段 15：饰品

目标：会话级饰品库、AI 制作请求、佩戴/卸载/删除和前端饰品层。

### Task 15.1: 饰品数据与命令

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/models.rs`
- Create: `src-tauri/src/accessories.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: 创建饰品表和模型**

按 `db/schema.sql` 创建 `accessories`；模型包含 `display_text`、`description`、`cost`、`source`、`is_worn`。

- [ ] **Step 2: 实现饰品 CRUD**

暴露当前会话饰品列表、佩戴、卸载、卖出未佩戴饰品、删除未佩戴饰品命令；卖出固定获得 10 喵币并写入 `currency_ledger`，删除后不可恢复。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。


### Task 15.2: AI 制作饰品流程

**Files:**
- Modify: `src-tauri/src/chat.rs`
- Modify: `src-tauri/src/accessories.rs`
- Modify: `src-tauri/src/currency.rs`

- [ ] **Step 1: 增加制作请求工具**

聊天 function calling 增加 `request_accessory_craft(name, display_text, description)`；达到亲密度档位且对应 sessions.craft_tier_1_used/2_used/3_used 尚未使用时可请求；第四档不限。不能以现存饰品数量或可变价格判断历史档位。

- [ ] **Step 2: 处理用户确认与扣款**

用户点“给你买”时重新验证请求绑定会话、档位和余额，在同一事务中扣款、写流水、写饰品、标记对应档位；不足或拒绝不创建。卖出/删除不清除档位，调整阈值或价格不重置。拒绝后至少 1 小时或下次启动后才能再次请求。

- [ ] **Step 3: 生成人设口吻简介**

扣款前准备并校验按人设口吻生成的 description；成功事务使用已确认内容，不在扣款后再等待 AI 生成必填简介。

- [ ] **Step 4: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。


### Task 15.3: 前端饰品层与管理页

**Files:**
- Modify: `src/components/Kaomoji.tsx`
- Create: `src/components/AccessoryLayer.tsx`
- Modify: `src/windows/PetWindow.tsx`
- Modify: `src/windows/SettingsWindow.tsx`

- [ ] **Step 1: 实现饰品层**

`AccessoryLayer` 渲染当前会话佩戴的 `display_text`，限制长度和字符集；不影响基础颜文字颜色和表情。

- [ ] **Step 2: 增加饰品管理入口**

设置 > 桌宠与内容 > 饰品 显示当前会话饰品列表、简介、佩戴/卸载/卖出/删除；卖出未佩戴饰品固定获得 10 喵币，右键菜单提供快捷入口。

- [ ] **Step 3: 编译验证**

Run: `npm run build`
Expected: 无错误。


---

## 阶段 16：伴手礼与旅行游记

目标：返回礼物进入伴手礼管理；正常返回生成独立的会话级旅行游记。

### Task 16.1: 伴手礼数据与管理

**Files:** `src-tauri/src/db.rs`、`models.rs`、`souvenirs.rs`、`commands.rs`、`src/windows/TravelWindow.tsx`。

- [ ] 按 `db/schema.sql` 接入 `souvenirs`；只保存普通文字礼物，可佩戴饰品进入 `accessories`。
- [ ] 仅正常返回生成 1 件伴手礼；返回事务固定 gift_kind（10% accessory、90% text），完整 gift_snapshot_json、完成状态与礼物/饰品结果同事务写入，失败重试不重复抽取或发放；生成前事实必须 ready。
- [ ] 伴手礼支持查看、收藏、删除；不增加背包、数量、容量或使用机制。
- [ ] 运行 Rust 检查与前端构建。

### Task 16.2: 旅行游记生成与恢复

**Files:** `src-tauri/src/travel.rs`、`travel_journals.rs`、`models.rs`、`ai/client.rs`、`commands.rs`。

- [ ] 正常返回且 facts_state=ready、gift_state=done 后，从出行记录读取地点、活动时间线、实际时间和 gift_snapshot_json；仍存在的信件作为可选细节，使用人设口吻生成游记。不依赖回来信或礼物展示行仍存在，取消旅行不生成。
- [ ] 标题必须用一句话极精炼概括什么时候去了哪里、做了什么、有什么伴手礼、去了多久；正文使用独立的游记字数范围。
- [ ] 游记写入 `travel_journals`，通过 `travel_id` 唯一关联，不写入短期记忆，不参与长期记忆整合；结果和 `journal_state=done` 同事务提交。
- [ ] 启动、配置有效提供商或测试连接成功时扫描 returned 且 `journal_state=pending` 的任务；失败保留 pending，提交前复核状态。
- [ ] 用户可编辑标题与正文，并可确认后单独删除；不提供 AI 重新生成。删除命令校验 session_id/travel_id，只删除目标游记并保留 journal_state=done；不影响其他出行子项，重启不补生成。会话删除时仍按外键级联。
- [ ] 验证正常返回、取消、离线返回、失败重试、重复调度、删除与编辑竞态，以及 done 但无游记行的重启/备份恢复；删除不得修改其他子项。

### Task 16.3: 出行页与游记查询工具

**Files:** `src/windows/TravelWindow.tsx`、`src-tauri/src/travel_journal_tools.rs`、`src/api.ts`、`context.rs`。

- [ ] 出行页跟随 Rust 当前会话，增加旅行游记列表、详情、编辑和删除入口；记忆小窗不展示游记。
- [ ] 注册 list_travels、list_travel_journals、get_travel_journal、update_travel_journal、delete_travel_journal，DTO 与前端设计第 13.6 节一致；删除后列表、详情、只读工具与后续上下文均不再返回该游记。
- [ ] 上下文只直接加入当前会话最近 5 篇仍存在的游记标题。
- [ ] 提供只读工具列出当前会话游记、按条件查找以前旅行、读取指定游记正文，后端强制校验会话归属。
- [ ] 切换会话后同时刷新邮箱、伴手礼、旅行游记和记忆入口，不增加独立会话选择器。
- [ ] 运行前端构建与 Rust 检查。

---
## 阶段 17：导入导出与数据迁移

目标：提供全量备份、指定会话、指定用户设定、人设资料包（分享用）、人设完整包（迁移用）和插件使用数据包的导出与导入。

### Task 17.1: 统一导出协议

**Files:** `src-tauri/src/import_export.rs`、`commands.rs`、`lib.rs`。

- [ ] 按数据库与持久化设计第 12 节实现 manifest.json、data.json、files/，禁止混用按表分文件格式；manifest 记录格式/结构版本、类型、数量和文件清单。
- [ ] 全量导出覆盖所有非密钥全局与会话数据，包括 settings、聊天/记忆/Embedding 提供商非密钥配置、occurrences、聊天回合、短期记忆、带标题的长期记忆及来源、档位标记、旅行完成状态、旅行游记、信件回复状态、插件登记/权限/设置/存档/游戏局与账单。插件登记保存 GitHub 仓库、项目主页、Release 页面和声明的联网 origin；授权按权限作用域导出。节日母表、FTS5 索引、长期记忆向量、内置 Embedding 模型文件和用户安装的 `.yanmew-game` 程序包不导出，子表母表引用导出稳定 code。
- [ ] 插件使用数据包只导出所选插件的登记快照、来源链接、manifest 权限与联网 origin 声明、设置和私有存档；不导出 `.yanmew-game`、游戏局、插件账单或钱包余额。
- [ ] 人设资料包仅定义和外观；人设完整包/会话包/带会话用户设定包带全所引用的人设定义、用户设定、背景图及会话级子记录，支持独立导入。
- [ ] 人设资料包、人设完整包、会话包和用户设定包不带钱包、账单、全局设置、提醒、提供商或任何小游戏插件登记、授权、设置、存档与游戏局。插件使用数据包只携带其明确范围内的插件数据。任何包都不带 API Key 或和风密钥明文/密文。
- [ ] 使用一致性读取快照与文件清单完成 ZIP；验证备份内容没有密钥，六种包关联均闭合。

### Task 17.2: 导入、映射与原子提交

**Files:** `src-tauri/src/import_export.rs`、`commands.rs`、`src/windows/SettingsWindow.tsx`、`src/api.ts`。

- [ ] 导入摘要展示范围和全局替换影响。默认新建副本；资料包仅新建，其他覆盖模式明确选择同类型/父级下的目标，不能用名称匹配后自动覆盖多条。
- [ ] 全量包用备份替换全局钱包、账单、settings、提醒与 occurrences、提供商配置，以及小游戏插件登记、逐 scope 权限、设置、存档和游戏局；不叠加，不给人设复制钱包或插件数据。人设资料包、人设完整包、会话包和用户设定包不动任何插件数据、其他全局数据或现有密钥；独立插件使用数据包按下一条处理。
- [ ] 插件使用数据包按插件 ID 展示覆盖摘要，用户确认后在同一事务替换所选插件的设置与存档；不导入游戏局或插件账单，不修改钱包，不重放历史喵币变化。将 `wallet.modify` 和每个 `network.fetch` origin 写为 `pending`、`decided_at=NULL`，用户重新确认前拒绝对应调用。
- [ ] 按表建立 ID 映射，处理 session_id、round_id、message_id、short_memory_id、long_memory_id、activated_by_round_id、travel_id、travel_journal_id、reply_to_id 与 settings.current_session_id；festival_id 通过 code 映射。保留来源顺序、整合状态、已回复、档位已用、取消及任务完成状态。
- [ ] 新导入聊天、记忆及联网 Embedding 提供商密钥为 NULL；全量密钥项清空并提示重新填写。保证内置本地 Embedding 项存在并重建当前项，保证部分唯一索引不冲突。
- [ ] 先验证版本、结构、字段、关系、图片大小和路径，暂存并使用唯一文件名；暂停后台写入，数据库同一事务完成覆盖、全局替换及所有关联写入。失败回滚并清理本次新文件，旧引用文件提交前不删除。
- [ ] 提交后先重建 FTS5，再异步重建长期记忆向量，然后恢复其他调度。缺少第三方插件程序包时导入其使用数据并登记为 `missing_package`，游戏中心显示来源链接；重新安装相同插件 ID 且兼容检查通过后关联旧设置和存档。测试六种包往返、同名歧义、ID 重映射、密钥排除、全局替换、独立插件导入不改钱包/账单/游戏局、权限重置为待确认、索引重建、插件缺包恢复、文件失败与数据库失败回滚，不能只运行编译检查。

---

## 阶段 18：节日母表、内置农历与签到彩蛋

目标：内置节日母表与会话级节日子表，完全离线，不接日历接口；签到命中节日时固定 10 喵币并展示祝福词。

### Task 18.1: 内置农历与节气数据表

**Files:**
- Create: `src-tauri/src/lunar.rs`

- [ ] **Step 1: 内置农历数据与转换**

在 `lunar.rs` 内置 1950–2100 年的农历数据表，提供：`solar_to_lunar(date) -> Option<LunarDate>`（含是否闰月）、`lunar_to_solar(year, month, day, is_leap) -> Option<NaiveDate>`、`solar_term_date(year, term_name) -> Option<NaiveDate>`（支持 清明、冬至）。超出范围返回 `None`，不 panic、不报错。数据表与转换逻辑全部内置在项目内，不引入第三方农历库。

- [ ] **Step 2: 单元测试**

对已知年份抽样验证：春节、中秋、清明、冬至的公历日期；闰月年份验证 `is_leap` 正确。


### Task 18.2: 节日母表内置数据与每年查表更新

**Files:**
- Modify: `src-tauri/src/db.rs`
- Create: `src-tauri/src/festivals.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 建表与内置数据**

按 `db/schema.sql` 创建 `festival_catalog`；首次启动按 `code` 幂等写入内置节日（公历：元旦、劳动节、国庆节；农历：腊八、小年、除夕、春节、元宵、端午、七夕、中秋、重阳；节气：清明、冬至），写入时用 `lunar.rs` 直接把当年的公历月日算好一起写入，并写入内置祝福词与 `is_memorial`。

- [ ] **Step 2: 每年查表更新**

启动、跨年或系统日期变化时比较母表中的 year 与当前公历年份：不一致则只挑 `calendar_type` 为 `lunar` 或 `solar_term` 的行，查 `lunar.rs` 里的内置表推出 `solar_month`、`solar_day`，连同 `year` 一起 `UPDATE` 回该行；`calendar_type` 为 `solar` 的行只更新 `year`，不动月日。全程不删除、不重建、不改动 `code`。除夕这类“腊月最后一天”的节日，其农历日（廿九或三十）同样由内置农历表推出后写回 `lunar_day`。


### Task 18.3: 节日子表 CRUD 与 AI 工具

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/festivals.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/context.rs`
- Modify: `src-tauri/src/chat.rs`

- [ ] **Step 1: 建表与命令**

按 `db/schema.sql` 创建 `session_festivals`；提供按会话的列表/新增/编辑/删除命令；year 为本年；lunar 行按目标公历年查内置表换算（年初腊月可能属于上一农历年），无对应日期清空旧换算值，本年不命中。公历行只更新 year，月日不变。

- [ ] **Step 2: AI 工具**

提供 function calling 工具：查询母表、增删改查子表；AI 可记录生日/纪念日和删除不需要的行，新增/编辑必须提供非空祝福词，日期以小型内置日历校验。

- [ ] **Step 3: 上下文注入**

日常聊天与无聊主动消息共用上下文组装，注入当天母表与当前会话子表节日；纪念性标记从母表读取，关联母表的子表继承；无母表关联不猜测纪念性质。


### Task 18.4: 签到彩蛋与设置页

**Files:**
- Modify: `src-tauri/src/currency.rs`
- Modify: `src-tauri/src/commands.rs`
- Create: `src/windows/FestivalPanel.tsx`
- Modify: `src/windows/SettingsWindow.tsx`
- Modify: `src/api.ts`

- [ ] **Step 1: 签到彩蛋**

签到命令先查当天母表与子表命中：命中任一则固定发放 10 喵币并返回该节日的祝福词，不调用 AI；同时命中则合并祝福词、金额仍为 10；都不命中则保持随机 1–10。纪念性节日使用纪念向文案，不出现“快乐”。

- [ ] **Step 2: 设置页**

在“桌宠与内容”新增“节日”页：系统节日只读列表 + 当前会话子表可新增/编辑/删除；桌宠状态页的签到结果显示祝福词。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Run: `npm run build`
Expected: 无错误。


### Task 18.5: 导出导入覆盖节日子表

**Files:**
- Modify: `src-tauri/src/import_export.rs`

- [ ] **Step 1: 纳入导出范围**

所有含会话的包都携带 session_festivals；关联母表改用稳定 code 传输并在目标端映射 ID。母表是内置数据，不导出。导入后按当前年份重新计算日期，不沿用过期换算值。


---

## 收尾与交付检查

- [ ] **Step 1: 全量测试与构建**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS。
Run: `npm test`
Expected: PASS。
Run: `npm run tauri build -- --no-bundle`
Expected: 成功生成 release 可执行文件，当前 Rust 包名对应 `src-tauri/target/release/yanwenzi.exe`；最终以构建输出为准，不能仅根据 productName 推断产物文件名。

- [ ] **Step 2: 组装完整便携版资源**

由发布脚本把 release 可执行文件、本地 Embedding 模型、随包石头剪刀布插件、随包清单、所需运行资源及许可证文件放入独立暂存目录，保持运行时相对路径，再压缩为 `颜喵 YanMew-portable.zip`。不能仅压缩 exe，也不能把开发者的数据库、密钥和个人存档带入发布包。

- [ ] **Step 3: 干净目录验收**

解压到未使用过的目录，验证启动、`data/yanwenzi.db` 初始化、本地 Embedding 离线推理、随包插件首次登记与授权、退出后再次启动。需要联网提供商的聊天独立验证；可选语音组件不纳入首版打包。


---

## 交付前检查

- 规格覆盖：窗口/托盘、亮色/暗色/跟随系统主题、提供商、人设、人设本体颜色、聊天窗背景、用户设定、表情库（点击：开心/惊讶/害羞、饿、吃东西、困、无聊、思考）、流式聊天与暂停中断、function calling 记事本、会话回合与记忆、桌宠状态、节日母表与节日子表、签到彩蛋、无聊主动消息、桌面游走、提醒、天气定位、错误休眠、默认人设、开机自启、人设自调、喵币与游戏、亲密度、饰品、旅行、写信、伴手礼、旅行游记、导入导出与数据迁移均有对应 Task。
- 已确定：节日彩蛋完全离线——母表是内置静态数据，农历与节气日期每年查内置表推出后写回，不接任何日历接口，不做放假与调休。
- 已确定：签到命中节日固定 10 喵币并直接展示节日祝福词（不调用 AI）；同时命中母表与子表时合并祝福词、金额仍为 10；纪念性节日不使用“快乐”文案。
- 已确定：无聊主动消息取代原空闲气泡，触发概率与上下文由代码层决定；桌面游走在用户拖动、点击、聊天、睡觉、出门中、隐藏宠窗时暂停。
- 实现覆盖：每项有明确命令契约、事务边界和验收条件；示例代码需按后续任务同步更新，不将描述性步骤宣称为已完成实现。
- 类型一致性：数据库 snake_case 与前端 camelCase 通过明确序列化映射；DTO、命令名和事件契约前后一致，密钥不进入 DTO。
- 应用图标使用用户提供的 `cat_icon.png`，已转为 `src-tauri/icons/icon.png`。
- 已确定：天气采用 Open-Meteo 优先、和风天气兜底，和风 API Host 与 API Key 在设置中配置（DPAPI 加密存储）；`create_persona` 自动生成表情库依赖 provider 可用，无 provider 时静默跳过并在设置页提示手动生成。
- 已确定：默认人设不可删除；删除其他人设级联删除其会话与会话级数据，删除当前人设后切回默认人设。提醒到点由 AI 用人设语气润色，失败使用固定默认提醒词；关闭期间全部过期一次性提醒与每个循环提醒一次统一汇总。宠窗默认右下角并记忆位置。
- 已确定：喵币为全局用户级，不随会话删除；亲密度、节日子表、饰品、旅行、信件和伴手礼为会话级，删除会话时级联删除。小游戏插件、授权、设置、存档、游戏局与插件账单全部为应用全局数据，与人设和会话无关。
- 已确定：石头剪刀布从第一版就是主仓库第一方插件，并随便携版提供；第一版支持本地 `.yanmew-game` 安装，不显示“官方收录”、不直接连接 GitHub、不提供插件 AI 接口。第三方社区仓库以后单独建立，通过 Pull Request、自动检查和人工审查后供用户从浏览器下载。
- 已确定：第三方插件程序包不进入任何备份。全量备份保存登记、权限、设置、存档、游戏局、账单与来源链接；独立插件使用数据包只保存登记快照、权限申请、设置、存档与来源链接。独立导入不改钱包、账单或游戏局，钱包与联网权限统一待确认；缺包时显示数据卡片，由用户从浏览器重新下载并安装。
- 已确定：插件可以申请 `network.fetch`。manifest 预先声明 HTTPS origin，安装或更新时按 origin 由用户授权；WebView 不直接联网，只能由宿主代发已授权请求。
- 已确定：主程序只内置亮色和暗色两套主题，设置可跟随系统或固定其中一套；主题全局保存并随全量备份迁移。小游戏插件视觉独立，不接收或跟随主程序主题。
- 已确定：只有当前会话在家空闲时累计；切换取消旅行和待回信；正常关闭后启动按真实时间续算，取消任务不恢复。
- 已确定：密钥不导出，新导入提供商重新填写；全量导入替换全局数据，人设资料包、人设完整包、会话包和用户设定包不改目标全局配置；独立插件使用数据包只恢复指定插件设置、存档并重置授权；人设资料包只新建副本并自动重命名，且包含聊天窗背景图和表情库。

---

## 后续扩展：本地语音与人设声线（不属于阶段 0–18）

语音功能在首版完成后另立实现计划，本文件当前不增加可执行 Task，也不提前修改 `db/schema.sql`。扩展范围包括当前人设专属唤醒词、本地流式 ASR、实时字幕、两个内置声线、`.yanmew-voice` 兼容模型导入、联网 ASR/TTS、通过标准包或向导导入别人已经制作完成的 GPT-SoVITS 声线，以及第二阶段本地音色克隆。GPT-SoVITS 导入不包含应用内训练。

实施前必须先完成模型许可证与再分发核对、Windows CPU 性能和误唤醒测试、语音包协议、数据库迁移与备份兼容设计。完整已确认产品规则见 `docs/语音扩展设计.md`。
