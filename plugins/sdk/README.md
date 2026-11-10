# 颜喵小游戏插件开发与发布教程

协议版本：**1.0.0**。面向 Windows 版颜喵，按当前源码接口编写。这里的插件是独立窗口中的 HTML/CSS/JavaScript 小游戏，不是聊天 Skill、模型工具、桌宠皮肤或可执行扩展。无需为插件安装 Tauri、编写 Rust 或调用聊天模型；从源码编译打包工具时才需要主项目的 Rust 开发环境。

阅读顺序：

1. [开发前的边界](#开发前的边界)：先知道宿主提供什么。
2. [从零做一个猜数字插件](#从零做一个猜数字插件)：复制四个文件，做出能保存进度的游戏。
3. [打包与实际安装](#打包与实际安装)：在真实颜喵中验证。
4. [manifest 逐项说明](#manifest-逐项说明)与[全部宿主接口](#全部宿主接口)：添加设置、钱包或网络。
5. [存档与运行](#存档与运行)、[钱包和网络](#钱包和网络)、[升级与数据迁移](#升级与数据迁移)：处理长期数据和失败恢复。
6. [调试与常见问题](#调试与常见问题)、[发布与社区投稿](#发布与社区投稿)、[发布前验收](#发布前验收)：交给其他玩家前逐项确认。

相关文件：[完整类型](index.d.ts)、[manifest 编辑器提示](manifest.schema.json)、[RPC 结构](rpc.schema.json)、[管理页摘要结构](summary.schema.json)、[无玩法的最小模板](../template/)、[打包工具说明](../build/README.md)、[现有自带游戏源码](../first-party/README.md)。JSON Schema 只帮助编辑，最终以与安装器共用的 Rust 校验器为准。

## 开发前的边界

- 游戏有自己的窗口和界面，不使用颜喵的 React 页面，也不自动跟随颜喵主题。默认窗口为 800×600，可调整大小，最小 320×240。
- 原生宿主自动注入 `window.yanmew`。不要把 `bridge.js` 复制进包，也不要导入 `@tauri-apps/api`、读取 `__TAURI_INTERNALS__` 或自己指定插件身份。
- 可以读取自己的设置和存档、取得随机数、记录运行结果；声明并获准后可修改全局喵币或通过宿主请求指定网站。
- 不能读取聊天、人设、记忆、API Key、数据库、其他插件存档，不能执行系统命令、任意读写文件或获得主程序的全部接口。当前没有 AI 生成、应用签到、文件选择、打开外部浏览器等插件 API。
- 网页、脚本、字体和素材随包提供。脚本放独立 `.js` 文件，用 `addEventListener` 绑定；内联 `<script>`、`onclick`、`eval`、CDN 脚本、iframe 和 Worker 不在当前宿主允许范围内。联网使用 `network.fetch()`，不能用页面原生 `fetch`、XHR、WebSocket 或远程图片绕过权限。
- `storage.*` 才是正式存档接口。不要依赖浏览器 localStorage、IndexedDB、Cookie 或缓存跨原生窗口保留数据。

## 从零做一个猜数字插件

这个示例在 1～10 中猜一个整数，每次猜测都保存进度；猜中后累计胜场并开始下一局。它不申请钱包和网络权限。先用你自己的稳定插件 ID；下文的 `org.example.guess-number` 仅作示例，发布时替换。

在主仓库外或本机临时目录新建下列目录，不要直接改 `plugins/first-party/`：

```text
guess-number/
  manifest.json
  ui/
    index.html
    style.css
    main.js
```

### 1. 写 manifest.json

```json
{
  "id": "org.example.guess-number",
  "name": "猜数字",
  "description": "猜一个整数，关闭重开后继续，保存累计胜场。",
  "author": "Example",
  "version": "1.0.0",
  "api_version": "1.0.0",
  "entry": "ui/index.html",
  "storage_version": 1,
  "permissions": [],
  "network_origins": [],
  "settings_schema": [],
  "repository": null,
  "homepage": null,
  "release_page": null,
  "migration": null
}
```

私人试做时 `repository` 可为 null；公开分发前填自己的真实 GitHub 源码仓库，并补充 README 和许可。不要把示例作者、ID 或空仓库地址当作实际发布资料。

### 2. 写 ui/index.html

```html
<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>猜数字</title>
  <link rel="stylesheet" href="style.css">
</head>
<body>
  <main>
    <h1>猜数字</h1>
    <p>猜一个 1～10 的整数，进度会保存在颜喵中。</p>
    <p id="status" role="status">正在连接宿主…</p>
    <form id="game">
      <label>你的答案<input id="guess" type="number" min="1" max="10" step="1" required></label>
      <button id="submit" type="submit" disabled>猜一下</button>
    </form>
    <button id="close" type="button" disabled>关闭游戏</button>
  </main>
  <script type="module" src="main.js"></script>
</body>
</html>
```

### 3. 写 ui/style.css

```css
body { margin: 0; background: #f6f7fb; color: #243041; font: 16px/1.7 system-ui, sans-serif; }
main { max-width: 600px; margin: auto; padding: 24px; }
input { display: block; box-sizing: border-box; width: 100%; padding: 10px; }
button { padding: 10px 16px; margin-top: 16px; cursor: pointer; }
button:disabled { cursor: wait; }
#status { overflow-wrap: anywhere; }
```

### 4. 写 ui/main.js

```javascript
const api = window.yanmew;
const status = document.querySelector("#status");
const form = document.querySelector("#game");
const input = document.querySelector("#guess");
const submit = document.querySelector("#submit");
const close = document.querySelector("#close");
const message = error => error?.message ?? String(error);
let state;
let busy = true;
function lock(value) {
  busy = value;
  input.disabled = submit.disabled = close.disabled = value;
}
const secret = async () => Math.floor(await api.game.random() * 10) + 1;
async function save(next) {
  await api.storage.set("state.main", next);
  state = next;
  // 摘要只是管理页展示；失败不撤销已经保存的实际进度。
  try {
    await api.storage.set("ui.summary", {
      version: 1,
      status: "playing",
      metrics: [{label: "猜中次数", value: String(next.wins)}]
    });
    return "";
  } catch { return "（游戏已保存，管理页摘要暂未更新）"; }
}
async function start() {
  lock(true);
  try {
    if (!api) throw new Error("请从颜喵安装并打开，或使用开发模拟宿主。");
    await api.game.ready();
    await api.game.getContext();
    const stored = await api.storage.get("state.main");
    if (stored === null) {
      const note = await save({version: 1, secret: await secret(), attempts: 0, wins: 0});
      status.textContent = `准备好了，请猜 1～10 的整数。${note}`;
    } else {
      if (stored.version !== 1 || !Number.isInteger(stored.secret) || stored.secret < 1 || stored.secret > 10 ||
          !Number.isSafeInteger(stored.attempts) || stored.attempts < 0 || !Number.isSafeInteger(stored.wins) || stored.wins < 0) {
        throw new Error("存档格式不兼容，请联系作者；没有覆盖原存档。");
      }
      state = stored;
      status.textContent = `已恢复：本局猜了 ${state.attempts} 次，累计猜中 ${state.wins} 次。`;
    }
    lock(false);
  } catch (error) { status.textContent = message(error); }
}
form.addEventListener("submit", async event => {
  event.preventDefault();
  if (busy) return;
  const guess = Number(input.value);
  if (!Number.isInteger(guess) || guess < 1 || guess > 10) {
    status.textContent = "请输入 1～10 的整数。";
    return;
  }
  lock(true);
  try {
    const won = guess === state.secret;
    const next = won
      ? {version: 1, secret: await secret(), attempts: 0, wins: state.wins + 1}
      : {...state, attempts: state.attempts + 1};
    const note = await save(next);
    status.textContent = `${won ? "猜对了！已开始下一局。" : guess < state.secret ? "猜小了。" : "猜大了。"} 累计猜中 ${state.wins} 次。${note}`;
    input.value = "";
  } catch (error) { status.textContent = `操作未完成：${message(error)}`; }
  finally { lock(false); }
});
close.addEventListener("click", async () => {
  if (busy) return;
  lock(true);
  try { await api.game.close(); }
  catch (error) { status.textContent = message(error); lock(false); }
});
void start();
```

这段代码先等存档写入成功再展示结果，按钮忙时禁用；存档损坏时保留旧值，不擅自清空。示例在同一个窗口中连续玩多局，因此不在每次猜中时调用 `game.complete()`。每局成绩属于 `state.main`，窗口的运行记录仍是同一条。

## 打包与实际安装

安装试玩需要颜喵Windows主程序，开发工具包不包含它；请使用提供给作者的当前测试包，主项目开源后的获取方式以[主项目README](https://github.com/xianyvqiu/yanmew)为准。本教程使用宿主协议1.0.0，社区浏览入口已包含在20261110-145616及后续构建中。

推荐下载 [Windows x64 开发工具包](https://github.com/xianyvqiu/yanmew-community-games/releases/tag/devkit-v1.0.1)，核对随附 SHA-256 后解压。工具包包含打包器、SDK、模板及本教程，不需要访问主仓库或安装 Rust、Node.js；仅编写网页即可开发插件。工具位置为解压目录下 `bin/yanmew-plugin.exe`。

维护者如需从源码构建，可在颜喵主源码目录按根 README 配好 Rust/MSVC、Node.js 和资源后执行：

```powershell
npm ci
npm run build
cargo build --release --locked --manifest-path src-tauri/Cargo.toml --bin yanmew-plugin
```

源码构建的工具位于 `src-tauri/target/release/yanmew-plugin.exe`。以下使用预编译工具，修改 `$pluginCli` 为实际解压路径，插件源码假设保存在 `C:\games\guess-number`，输出位于本机临时目录：

```powershell
$pluginCli = 'C:\tools\yanmew-plugin-devkit-1.0.1-windows-x64\bin\yanmew-plugin.exe'
$gameSource = 'C:\games\guess-number'
$packageFile = Join-Path $env:TEMP 'org.example.guess-number-1.0.0.yanmew-game'
& $pluginCli pack $gameSource $packageFile
& $pluginCli validate $packageFile
Get-FileHash -LiteralPath $packageFile -Algorithm SHA256
```

输出路径已存在时工具拒绝覆盖，选择新文件名或删除你自己上次生成的包后再运行。打包器不会运行游戏，只验证结构；不能把“校验通过”当作游戏已经测试通过。

打开颜喵“全局设置 → 小游戏插件 → 从本地安装”，选择输出的 `.yanmew-game`，检查插件名称、作者、版本及权限后确认安装。再从卡片打开游戏，试一次猜测，关闭并重新打开，确认进度恢复、管理页出现“猜中次数”。测试时建议使用独立便携副本，避免清除个人游戏存档。

如果打包器或安装器报错，保留完整错误信息并检查 manifest 和资源路径。包必须在根目录直接有 `manifest.json`，不能多套一层 `guess-number/`。不要只把普通文件夹重命名为 `.yanmew-game`。

## manifest 逐项说明

文件使用 UTF-8 JSON，键名为 snake_case；未知字段会被拒绝。

- `id`：发布后稳定不变，最多 128 字节，至少两段；每段只含小写字母、数字、短横线，以 `.` 分隔。社区投稿不使用维护者的 `com.yanmew.game.*` 空间。换 ID 会被视为另一个游戏，不能自动继承旧存档。
- `name`、`description`、`author`：非空，分别最多 100、2000、100 个 Unicode 字符。作者和仓库应真实可追溯。
- `version`：SemVer，例如 `1.0.0`、`1.1.0`、`2.0.0-beta.1`；每次公开更新递增。插件版本和存档版本是两回事。
- `api_version`：当前固定 `1.0.0`，不能填应用版本 `0.1.0`，也不能自行填写未来协议。
- `entry`：包内 `ui/` 下的 HTML 相对路径，例如 `ui/index.html`。不能使用本机绝对路径或 URL。
- `storage_version`：正整数，首次为 1；只改 UI 通常无需增加，改变存档或不兼容设置结构时增加并提供迁移。
- `permissions`：可省略，默认 `[]`；当前只支持 `wallet.modify`、`network.fetch`，不重复。声明权限不等于用户已授权。
- `network_origins`：不用网络时 `[]`。申请 `network.fetch` 时必须列出 1～16 个互不重复、规范化的 HTTPS origin，例如 `https://example.com`；不要包含 `/api`、末尾 `/`、查询参数、通配符或凭据。非默认端口须明确写出，如 `https://example.com:8443`。
- `settings_schema`：最多 64 项。定义可供玩家在插件管理页编辑的设置，省略时为 `[]`。运行时通过 `getContext().settings` 取得默认值与用户覆盖值合并后的只读快照，没有插件主动修改宿主设置的接口；运行中希望刷新设置可再次调用 `getContext()`。
- `repository`：私人试做可为 null；公开分发应填写无凭据的 GitHub 源码仓库地址。应用不会从这个字段下载和执行代码。
- `homepage`、`release_page`：可为 null，否则为无凭据的 HTTPS 页面地址；后者给出玩家手动下载的 Release 页面。
- `migration`：无迁移时 null；需要时填写包内 `.js` ES module 相对路径，如 `migrate.js`，模块导出 `migrate`，不能填 TypeScript 或网络 URL。

设置示例：把下面数组放入 `settings_schema`。`key` 规则同存档键；每项至少含 `key/title/type/default`，支持 boolean、integer、number、string、enum；`required` 默认为 false，`description` 默认为空。数字可加 `minimum/maximum`，enum 提供 `{value,label}` 的 `options`，默认值必须符合类型、范围和选项。

```json
[
  {"key":"sound.enabled","title":"音效","type":"boolean","default":true},
  {"key":"round.limit","title":"每局回合数","type":"integer","default":10,"minimum":1,"maximum":100},
  {"key":"difficulty","title":"难度","type":"enum","default":"easy","options":[{"value":"easy","label":"简单"},{"value":"hard","label":"困难"}]}
]
```

```javascript
const context = await window.yanmew.game.getContext();
const soundEnabled = context.settings["sound.enabled"] ?? true;
```

包只接受 ASCII 安全相对路径，单个路径最多 240 字节；不能有 `..`、空段、反斜杠、以点结尾的目录或 Windows 保留文件名。当前资源格式为 html、js/mjs、css、json、svg、png、jpg/jpeg、webp、gif、ico、woff/woff2/ttf、mp3/ogg/wav、mp4/webm、txt/md。TS/JSX/SCSS 等开发源码先编译为这些资源再打包。包最多 64 MiB，展开总量最多 128 MiB、单文件 32 MiB、4096 个条目；不接受符号链接、junction、EXE/DLL、大小写冲突的路径或压缩路径穿越。完整开发源码和依赖许可保留在源码仓库，不把 `node_modules`、`.git`、个人数据、密钥或无关文件塞进分发目录。

## 全部宿主接口

现有 API 共 11 个方法，全部返回 Promise；调用时 `await`，用 `try/catch` 处理失败。参数名称严格区分大小写。`Json` 是 null、布尔值、有限数字、字符串、JSON 数组或普通对象；不要传 `undefined`、BigInt、Date、函数、DOM 节点或循环引用。

### game.ready()

参数无，返回 `Promise<void>`。页面与事件处理准备好后通知宿主；它不会自动创建新游戏局、发奖、授予权限或恢复存档。

### game.getContext()

参数无，返回只读 `Context`：`language` 当前为 `zh-CN`；`gameRunId` 为本次打开的运行 ID 字符串；`pluginVersion` 为当前插件版本；`walletPermissionGranted` 为打开时的钱包授权快照；`permissions` 是查询时的权限摘要数组；`settings` 为合并后的设置对象。

每条权限包含 `permission/scope/decision/pluginVersion/decidedAt`。`decision` 为 pending、allowed、denied，钱包的 scope 为 `""`，网络为具体 HTTPS origin。`decidedAt` 可以为 null。不要修改该对象，也不要把它当作永久有效的授权凭据；宿主在每次实际操作时重新校验。

### game.random()

参数无，返回 `Promise<number>`，范围 **0 ≤ n < 1**。生成闭区间 `[min,max]` 的整数：`min + Math.floor(await api.game.random() * (max-min+1))`。需要恢复同一局时把产生的结果存档，不要在每次重绘、读取存档或重试结算时重新抽取。

### game.complete(outcome, details?)

`outcome` 为非空字符串、最多 256 UTF-8 字节，`details` 为可选 JSON、序列化后最多 64 KiB；返回 `Run`。Run 的字段见 `index.d.ts`，包括 `id/pluginId/pluginName/pluginVersion/walletPermissionGranted/status/outcome/details/coinDelta/startedAt/endedAt`，其中 ID 和金额为十进制字符串。

它结束**本次窗口运行**。同 outcome/details 重试返回原记录，不同内容报错；完成后不能提交新的钱包变化。先结算钱包、保存进度，再 complete。持续养成、连续多局的游戏可以不调用，关闭时仍会结束本次运行，长期进度保存在 storage。

```javascript
await api.storage.set("state.main", savedGame);
const run = await api.game.complete("won", {score: 120});
```

### game.close()

参数无，返回 `Promise<void>`，关闭当前游戏窗口。仍在 running 的运行记录结束为 cancelled；已 completed 的结果不覆盖。不会自动保存页面内存，先完成你需要的 `storage.set()`，再关闭。用户直接关闭窗口时也不会替插件保存尚未提交的内存数据，所以关键操作后及时保存，不依赖 unload 中的异步请求。

### storage.get(key)、storage.set(key, value)、storage.delete(key)

分别返回 `Promise<Json>`、`Promise<void>`、`Promise<void>`。缺失键 get 返回 null；delete 删除当前插件自己的一个键。读取到对象后修改内存不会自动保存，必须 set。存档不按人设或会话分开，而是按插件 ID 隔离；不要在键名里硬编码玩家或系统路径。容量、键名和摘要约定见后文。

```javascript
const best = await api.storage.get("score.best");
await api.storage.set("score.best", Math.max(typeof best === "number" ? best : 0, 100));
await api.storage.delete("temporary.round");
```

### wallet.getBalance()

参数无，返回 `{coins: string, updatedAt: string}`，只读全局喵币余额。当前读取余额无需 `wallet.modify`，修改才需要授权。余额为十进制字符串，展示用原值，需要本地比较时可 `BigInt(coins)`；BigInt 不能直接放进 JSON 或 SDK 请求。

### wallet.changeCoins(request)

参数和返回：

```typescript
type WalletRequest = {
  request_id: string;      // 每个独立业务操作的稳定唯一 ID，最多 128 字节
  game_run_id: string;     // getContext().gameRunId；不要猜测或替换
  delta: number;           // 安全整数，负数扣币，正数发币，0 也记录流水
  reason: string;          // 非空、最多 200 字，向玩家说明本次变化
  details?: Json;         // 可选，最多 64 KiB
};
type WalletResult = {
  ledgerId: string;
  appliedDelta: string;
  duplicate: boolean;      // true 表示返回之前同一请求的结算，不应再发奖励
  balance: string;         // 查询时的权威余额
};
```

`delta` 的绝对值不超过 `Number.MAX_SAFE_INTEGER`。请求失败不能自行给玩家加币、展示成功或用新 request_id 重试。后文说明如何持久化待核对请求；不要把钱包代码直接套进第一个入门示例。

### network.fetch(request)

参数为 `{url, method?, headers?, body?}`，method 默认 GET，仅支持 GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS；headers 为字符串键值对象，body 为字符串或 null。返回标准 `Response`，可读取 `status/ok/headers/text()/json()/arrayBuffer()`；响应体只能消费一次。

先在 manifest 同时声明 `"network.fetch"` 和目标 origin，安装后由用户允许。下面只是读取方式，`example.com` 不保证返回 JSON；开发时换成自己实际控制且已申请的服务。

```javascript
const response = await api.network.fetch({url: "https://example.com/game/config.json"});
if (!response.ok) throw new Error(`服务返回 HTTP ${response.status}`);
const remoteConfig = await response.json();
```

HTTP 404/500 通常仍返回 Response，作者检查 `ok`；权限、网络、超时、过大响应等会 reject。这个桥接当前只重建 status、headers 和 body，不保留原请求 URL、redirected 等浏览器 Response 元数据，也不提供流式响应、AbortSignal 或上传文件接口。不要依赖 `response.url` 得到最终地址。网络限制和跨来源行为见后文。

原生宿主注入 `window.yanmew`，接口类型见 `index.d.ts`。作者不导入 Tauri，不自行声明插件身份，不调用 `__TAURI_INTERNALS__`；宿主会拒绝其他命令。

## 存档与运行

`game.ready()` 只通知页面准备完成。`game.getContext()` 返回当前运行 ID、版本、权限摘要及已合并的设置默认值；`walletPermissionGranted` 是本次打开时的钱包授权快照，`permissions` 是查询时的当前授权。两者同时允许才可以提交新的钱包请求，中途授权需要下次打开才能结算。每次打开创建一条运行记录，重复入口聚焦已有窗口。正常关闭结束本次运行并保留存档；启动时将崩溃遗留的 running 记录标为 error。

运行记录不是游戏的一局，更不是长期进度。连续养成、剧情、角色等使用 `storage.*` 保存；按局游戏也自行保存局次，日次数限制按插件自己的进度规则实现。`game.complete()` 可选，表示本次运行已正常完成，之后不能提交新的钱包变化。`game.close()` 或窗口关闭不清除私有存档。

存档键最多 128 字节，由小写字母、数字、下划线、短横线组成，以点分段。单值最大 1 MiB，合计最大 10 MiB，最多 1024 个键。设置只接受 manifest 的 `settings_schema`（最多 64 项），支持 boolean/integer/number/string/enum，默认值和覆盖值同样校验。

## 管理页进度摘要（可选）

先保存游戏的实际进度，再使用现有存档接口写入约定键 `ui.summary`：

```js
await window.yanmew.storage.set("state.main", savedGame);
await window.yanmew.storage.set("ui.summary", {
  version: 1,
  status: "playing",
  metrics: [
    { label: "目前关卡", value: "第 8 关" },
    { label: "已收集", value: "12 / 30" }
  ]
});
```

类型见 `SaveSummary`，校验约定见 `summary.schema.json`。`version` 必须为1；可选 `status` 为 `saved`（默认）、`playing` 或 `completed`，由颜喵统一显示“进度已保存”“游戏进行中”或“已通关”。可选 `metrics` 最多3项，label最多20个Unicode字符，value最多40个，不接受空白文本、控制字符、未知字段或HTML渲染。文本以普通文字展示，不提供自定义按钮、样式、脚本或页面布局。

摘要仅供玩家查看，不是实际存档，不用于钱包结算或验证成绩。写入失败不应回滚已保存的实际进度；下次保存时可以再次更新摘要。`storage.delete("ui.summary")` 可恢复默认展示。使用当前的私有存档权限和容量限制，无新权限、RPC或存储版本要求，协议仍为1.0.0；旧插件无需改动，未提供摘要或格式无效时显示默认保存状态，内置游戏仍保留已有的简短进度解释。宿主的待核对结算提示优先于摘要状态。升级迁移时如需保留摘要，应在完整目标快照中保留该键；不需要时可以省略，由宿主删除。

## 钱包和网络

需要可运行的完整例子时，先看[喵币计算挑战](../examples/wallet-challenge/README.md)：包含manifest、完整HTML/CSS/JavaScript和许可，开始扣1喵币、答对奖励2喵币，演示原请求持久化、重复点击保护、响应未知、保存失败及重开核对。已发布开发工具包的CLI可打包本示例；请从公开的`developer/examples/wallet-challenge/`取得完整源码。当前1.0.1 ZIP不包含该示例，工具包内文档为打包时快照，最新说明以本公开教程为准。请在独立可丢弃资料中安装测试，不用自己的日常钱包测试；示例不是社区目录的正式作品。

### 钱包结算与存档的衔接

钱包和插件 JSON 存档不是同一个事务。含奖励、购买或入场费的操作应按以下顺序设计：

1. 检查 `getContext()` 的打开时授权快照及当前 `wallet.modify` 是否 allowed。权限缺失时进入明确的纯玩模式，或停止开始并解释原因，不能假报钱包操作成功。
2. 为本次业务只生成一次 `request_id`。把请求、旧进度和结算后的目标进度一起写入 `state.main.pending`，写成功前不调用钱包。
3. 调用 `wallet.changeCoins(pending.request)`。确认成功后，把目标进度保存为正式进度并清除 pending，然后展示成功；`duplicate:true` 也只落实一次进度。
4. 响应未知、关闭或保存失败时保留 pending，暂停会产生新结算的动作。下次重开带**原请求 ID、原运行 ID、原 delta 和原 reason**核对，不能把旧请求改成当前运行 ID。
5. 宿主查到同一已提交请求会返回旧账单，不再记账；未提交的旧运行请求会被拒绝。不能把每一种错误都当作“肯定没扣币”后清空 pending；未知或无法判断的错误继续保留，明确提示未确认，必要时人工核对账单。

只增加一个 `await wallet.changeCoins()` 并不能自动满足防重复结算。可参考 [第一方共用运行时](../first-party/shared/runtime.js) 对 pending 的恢复实现，但它包含第一方游戏的进度约定，不要求第三方引入其全局变量或完整架构。不得通过“清除存档后补发”重放旧喵币奖励。

所有金额及数据库 ID 返回十进制字符串。请求 `delta` 为 JavaScript 安全范围内整数；请求 ID 最多 128 字节、展示原因最多 200 字。每个独立操作使用唯一 `request_id`；失败重试复用相同 ID 和业务参数；不得为每次重试产生新 ID。已经提交的请求在关闭或升级后仍可携带原运行 ID、数量与原因核对原账单，不再次结算；未提交的新请求只能使用当前运行 ID。先结算钱包再 complete。

打开时未授权钱包的运行不能提交喵币变化；中途授权从下次打开生效，撤权立即阻止新请求。插件自己的进度和战绩不能代替宿主账单。拒绝钱包权限时不得提交喵币变动。支持纯玩的插件可以继续游玩；玩法明确依赖钱包的插件应解释所需权限并停止开始，例如自带我的家。

`network.fetch({url, method, headers, body})` 返回标准 Response。只允许已申请且获允许的 HTTPS origin、公网 IP、最多 3 次重定向。超时 30 秒、同时最多 4 个请求、请求正文和解压后响应各 1 MiB；请求最多 32 个头、合计 16 KiB。禁止 Cookie、Host、代理认证、转发和逐跳头。可提交插件自己向用户取得的服务凭据；宿主不附加任何颜喵密钥。跨来源重定向移除自定义头，带正文的跨来源跳转除 303 外拒绝。

每次重定向的目标也必须已申请且获允许。localhost、局域网和解析到私网的域名不在允许范围；插件网络当前直接联网，不使用系统代理，不能套用聊天提供商的代理配置。不要把玩家凭据写到源码、README、日志或公开存档示例中。

## 升级与数据迁移

插件版本为 SemVer，协议要求 `api_version: "1.0.0"`。存储版本是递增正整数，改变存档或不兼容设置结构时提高 `storage_version`，并通过 `migration` 指定包内 JS ES module。模块必须导出：

```js
export async function migrate(snapshot, targetVersion) {
  // snapshot = { storage_version, settings: {键:值}, data: {键:值} }
  const result = structuredClone(snapshot);
  // 在这里实现所有需要兼容的旧版本到目标版本转换。
  result.storage_version = targetVersion;
  return result;
}
```

迁移只获得自己的完整数据副本，无钱包、网络或存储写入接口。返回完整目标快照，省略的键会在成功后删除。迁移时间上限 15 秒；缺少迁移、异常、超时、无效结果或并发修改均保留原插件与存档。升级前关闭正在运行的插件，成功后再打开新版。不得使用迁移操作结算历史喵币。

例如从第1版进度增加 `hintsUsed` 字段：新版 manifest 的 version 改为 `1.1.0`、storage_version 改为 2、migration 填 `migrate.js`，根目录新增下面模块。这里假设你的旧进度确实保存在 `state.main`；不要直接用它迁移另一种数据结构。

```javascript
export function migrate(snapshot, targetVersion) {
  if (snapshot.storage_version !== 1 || targetVersion !== 2) throw new Error("不支持的迁移范围");
  const next = structuredClone(snapshot);
  const old = next.data["state.main"];
  if (old !== undefined) next.data["state.main"] = {...old, hintsUsed: 0};
  next.storage_version = 2;
  return next;
}
```

`storage_version` 控制宿主管理的整体快照，示例游戏对象中的 `version` 是作者自己的内部格式，两者不自动联动；内部格式也变更时须相应修改迁移和读取逻辑。迁移要支持用户可能跳过的旧版本，不假设每人都安装过所有中间版本。

## 浏览器开发

`mock-host.js` 的 `createMockHost({id,version,settings})` 返回模拟 SDK，可赋给开发页的 `window.yanmew`。存档按插件 ID 保存到浏览器 localStorage；模拟器不开放真实钱包和网络，也不承诺与宿主容量校验完全一致。这些权限、CSP、迁移隔离和原生关闭必须在真实颜喵宿主验收。

先建立自己的开发工作目录，将工具包的`sdk/mock-host.js`（社区源码中为`developer/sdk/mock-host.js`，主项目中为`plugins/sdk/mock-host.js`）复制到下列位置。游戏使用前面完整示例的四个文件，放在`example-game/`；这些路径均相对于此工作目录，不要求访问主仓库：

```text
workspace/
  sdk/mock-host.js
  example-game/manifest.json
  example-game/ui/index.html
  example-game/ui/style.css
  example-game/ui/main.js
  dev-game.html
```

`dev-game.html`仅用于普通浏览器，复制`example-game/ui/index.html`后将CSS地址改为`./example-game/ui/style.css`，保留页面主体的DOM，只把最后的脚本替换为：

```html
<script type="module">
  import {createMockHost} from "./sdk/mock-host.js";
  window.yanmew = createMockHost({id: "org.example.guess-number"});
  await import("./example-game/ui/main.js");
</script>
```

在上述`workspace/`目录执行`python -m http.server 8787 --bind 127.0.0.1`，访问`http://127.0.0.1:8787/dev-game.html`。先确认模拟宿主、游戏JS和CSS均从该地址正常载入；这只是可选调试方式，不增加插件的Python依赖。不要双击file://页面来判断模块加载结果。开发页及模拟宿主不放进正式游戏包；正式包仍以`example-game/`为源，原生宿主自动注入真实SDK。

## 调试与常见问题

- **`window.yanmew` 不存在**：普通浏览器不是颜喵宿主。使用模拟开发页，或将包从本地安装后打开；不要复制 bridge.js 试图获得权限。
- **页面空白或按钮不工作**：先看模块路径、大小写和 DOM ID，脚本必须是包内独立文件。检查内联事件、CDN、TS 源文件、远程资源或浏览器限制。原生插件窗口当前关闭 DevTools；先在浏览器调试纯界面，再用游戏内 status/错误区展示脱敏后的 SDK 错误。
- **`PLUGIN_PACKAGE_INVALID` / `VALIDATION_ERROR`**：核对 JSON、entry、文件格式、ASCII 路径、容量以及 manifest 未知字段。打包器会报告中文原因，不用手工改 ZIP 绕过。
- **`PLUGIN_PERMISSION_DENIED`**：可能是未授权、授权版本失效、错误运行 ID、运行已结束、非法请求或旧运行未提交。结合当前动作解释，不把所有拒绝都说成“网络坏了”。查看权限后重新打开，让新的钱包快照生效。
- **`PLUGIN_REQUEST_CONFLICT`**：同 request_id 携带的运行 ID、delta 或 reason 与已记账内容不同。恢复原请求，不能换 ID 再发一次。
- **`PLUGIN_RUN_FINISHED`**：同一窗口运行已结束，不能覆盖不同运行结果。连续多局游戏不要把 complete 当作每局结束。
- **`PLUGIN_NETWORK_BUSY` / `PLUGIN_NETWORK_FAILED`**：减少并发，检查允许的 origin、公网可达性、超时和正文大小；游戏保留原进度并允许重试，不静默伪造成功。
- **存档写失败**：保留旧进度，显示错误并停用依赖该写入的操作；不要在失败后继续扣币或宣称已保存。
- **模拟器能玩，原生不行**：重新检查 CSP、真实权限、资源路径、容量和迁移；模拟器不能代替这些验收。

错误通常包含 `code` 与面向用户的 `message`，可按 `error?.message ?? String(error)` 展示；不要依赖某一种 JS Error 原型。用户发来报错时不要要求 API Key 或完整私人存档。

## 发布与社区投稿

### 作者自行发布

1. 在自己的 GitHub 仓库提交完整可审查源码、玩法 README 和 LICENSE，说明图、字体、音乐与依赖的来源和再分发许可。公开分发前填写真实 author/repository/release_page，不提交个人数据、密钥、`.env`、依赖目录或自己的开发包缓存。
2. 决定不可变的插件 ID 和本次 SemVer 版本，打包并 validate。输出用 `<plugin-id>-<version>.yanmew-game`，计算 SHA-256。
3. 在源码对应提交上创建版本 tag。在 GitHub 仓库的 Releases 中选择该 tag，写玩法、所需权限、改动、兼容宿主与迁移说明，上传 `.yanmew-game` 和 SHA-256 校验文件。普通 GitHub 的“Source code.zip”不是插件包。
4. 在独立颜喵副本中从 Release 重新下载并安装，验证这是实际上传的包。旧版本保留下载与更新说明；发现问题提升版本，不悄悄替换同版本文件。

可用 PowerShell 生成校验文件：

```powershell
$sha256 = (Get-FileHash -LiteralPath $packageFile -Algorithm SHA256).Hash.ToLowerInvariant()
$fileName = Split-Path -Leaf $packageFile
[IO.File]::WriteAllText("$packageFile.sha256", "$sha256  $fileName`n", [Text.UTF8Encoding]::new($false))
```

### 向颜喵社区投稿

社区仓库：[xianyvqiu/yanmew-community-games](https://github.com/xianyvqiu/yanmew-community-games)。投稿采用 `games/<plugin-id>/` 的源码 PR，一个 PR 只新增或更新一个小游戏；维护者审阅玩法、代码、权限及许可后，统一打包发布。第一方自带游戏仍保留在主仓库。

作者提供 manifest、源码、运行资源、README.md、素材许可；社区分发目录使用 LICENSE.txt，保证打包器可识别。前端开发项目可以另带 `src/`，但正式分发的 ui 必须是可运行的 HTML/CSS/JS。不要提交只有成品包、无法复现的混淆文件或未授权素材，也不要在 manifest 自报“官方”身份。

社区采用本地校验和维护者本地发布，不依赖 GitHub Actions、付费 runner 或主仓库源码。作者提交前，在社区仓库根目录执行 `npm run check:local`，下载并核验固定版本开发包，检查目录并实际打包全部投稿；检查不代替真实游玩。源码 PR 经人工审阅和本地检查后合入 main。

维护者完整克隆社区仓库，在干净且与远端同步的 main 上执行：

```powershell
git switch main
git pull --ff-only
npm run release:local -- org.example.guess-number
```

将示例 ID 改为实际游戏 ID。命令从 manifest 读取版本，使用已校验 CLI 打包，复用本机 Git Credential Manager/GitHub Desktop 保存的 GitHub 登录，在内存中调用 Releases API 创建或核对 `<plugin-id>/v<version>`、上传包及校验文件，然后提交并推送 catalog.json。无需手填 token、设置仓库 Secret 或先手工推 tag。发布者必须有仓库写权限且本机 git push 可用；命令不修改 Git 姓名/邮箱。凭据不写入文件、URL或命令行。

同版本已上传附件仅在 SHA-256 一致时复用；修复游戏须提升版本。网络中断可重新执行命令，完整步骤和推送冲突处理见社区 [README](https://github.com/xianyvqiu/yanmew-community-games#维护者本地发布) 与 [CONTRIBUTING](https://github.com/xianyvqiu/yanmew-community-games/blob/main/CONTRIBUTING.md)。PR 分支先确认只包含游戏变更，再执行仓库脚本；工具改动单独审查。

应用内“社区小游戏”源码已接入，须使用包含该改动的新构建；它使用本社区仓库的 `catalog.json`，不会自动搜索作者自己的 Release。进入目录需要维护者为正式社区 Release 填入版本、附件地址/大小/SHA-256、发布时间，以及固定到源码提交的 README 地址和 SHA-256；仅合入源码不等于已经出现在列表。下载由浏览器完成，再返回颜喵从本地安装、确认权限，不自动安装或授权。当前可使用前述手动下载和本地安装流程。

## 发布前验收

应用内社区页的“下载”只打开系统浏览器，安装包保存位置由浏览器决定，通常是Windows“下载”文件夹；启用“每次询问保存位置”时由用户选择。下载不会自动写入颜喵包目录，也不会自动安装。下载完成后从颜喵的“从本地安装”选择`.yanmew-game`，确认后插件文件进入应用管理的`data/plugins/`；便携版的data位于EXE旁。下载文件与已安装插件是两份独立文件。

至少完成以下实际验证，并在游戏 README 写明宿主版本、结果和未测项：

- 新安装、运行、关闭重开、游戏存档恢复；忙碌时重复点击不会重复开始或覆盖进度。
- 窗口 800×600、最小尺寸、不同系统缩放；键盘/鼠标操作、滚动、长文本与错误说明可达。
- 无网络、网络失败、权限未授权/拒绝/撤回时行为清楚，不读其他插件或应用资料。
- 钱包授权前后、余额不足、连续点击、响应未知、退出重开后的同请求核对；不重复扣币或发奖。入门无钱包示例可记为不适用。
- 旧存档直接跳版本升级、迁移异常/超时、设置结构变化和失败后旧版保留。首版无旧存档时写明不适用，不假装通过。
- 从 Release 重新下载的实际包通过校验、安装和游玩；源码版本、manifest、tag、文件名与说明一致。
- 卸载或清除私有数据只影响本游戏；不会重放全局喵币或应用每日签到。

自动结构检查不能判断美观、玩法公平性、流畅度或所有失败恢复。把这些交给真人实测，未测保留为未测。
