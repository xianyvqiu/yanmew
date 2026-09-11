# 颜喵 YanMew 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 构建一个 Windows 常驻桌宠应用“颜喵 YanMew”：透明置顶可拖动的颜文字形象，支持多 API 提供商、多人设、用户设定、流式聊天（含暂停中断）、会话记忆与记事本、表情库、提醒、天气定位、人设自调、喵币与饰品、旅行/写信/伴手礼、导入导出与数据迁移。

**Architecture:** Tauri 2（Rust 核心）负责窗口、SQLite、调度、天气定位、AI 客户端、喵币、饰品、旅行、信件与伴手礼；React + TypeScript 前端负责宠窗、聊天窗、设置窗的渲染与交互。Rust 暴露 Tauri commands 给前端，AI 流式回复通过 events 推送到前端。数据用 SQLite 持久化，API Key 用 Windows DPAPI 加密。

**Tech Stack:** Tauri 2、Rust、React 18、TypeScript、Vite、SQLite（rusqlite）、windows crate（DPAPI、定位）。

---

## 约定

- 所有命令在项目根目录 `C:\my_program\my_idea\yanwenzi` 执行。
- 前端包管理器：npm。
- 提交信息用中文，格式如 `feat: 添加xxx`、`fix: 修复xxx`、`test: 添加xxx测试`。
- Rust 测试：`cargo test`；前端测试：`npm run test`（Vitest）。
- 每完成一个 Task 就提交一次，保持提交粒度与 Task 一致。
- Windows 换行：仓库用 LF，Git 已配置自动转换，不要手动改 CRLF。
- 本计划分 19 个阶段（阶段 0 到阶段 18），每个阶段结束时应用可编译运行。
- 数据库结构只维护 `db/schema.sql`，初始化阶段建立完整结构；后续任务补齐模型、命令和交互，不重复增加已存在列。事务与备份协议统一遵守 `docs/数据库与持久化设计.md`。
- AI 请求不持有数据库锁或事务；读取快照、请求完成后短事务校验并提交。

## 项目结构（本计划将创建/修改的文件）

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
  - `src/components/Kaomoji.tsx`、`Bubble.tsx`、`ContextMenu.tsx`、`ErrorBadge.tsx`、`AccessoryLayer.tsx`。
- `src-tauri/`：Rust 核心。
  - `src-tauri/Cargo.toml`、`tauri.conf.json`、`build.rs`、`capabilities/default.json`。
  - `src-tauri/src/main.rs`、`lib.rs`、`error.rs`、`state.rs`。
  - `src-tauri/src/db.rs`、`models.rs`、`config.rs`、`crypto.rs`。
  - `src-tauri/src/providers.rs`、`persona.rs`、`user_profiles.rs`、`expressions.rs`、`sessions.rs`、`memory.rs`、`memory_commands.rs`、`notebook.rs`、`context.rs`、`proactive.rs`、`festivals.rs`、`lunar.rs`。
  - `src-tauri/src/ai/mod.rs`、`ai/client.rs`、`ai/streaming.rs`。
  - `src-tauri/src/pet.rs`、`reminders.rs`、`weather.rs`、`location.rs`、`currency.rs`、`accessories.rs`、`travel.rs`、`letters.rs`、`souvenirs.rs`、`import_export.rs`、`commands.rs`。

---

## 阶段 0：脚手架（可运行空壳）

目标：能 `npm run tauri dev` 打开一个空窗口。

### Task 0.1: 初始化前端工程

**Files:**
- Create: `package.json`
- Create: `vite.config.ts`
- Create: `tsconfig.json`
- Create: `tsconfig.node.json`
- Create: `index.html`
- Create: `src/main.tsx`
- Create: `src/App.tsx`
- Create: `src/vite-env.d.ts`

- [ ] **Step 1: 写入前端工程文件**

`package.json`:
```json
{
  "name": "yanwenzi",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "test": "vitest run"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0",
    "@types/react": "^18.3.3",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.1",
    "typescript": "^5.5.4",
    "vite": "^5.4.0",
    "vitest": "^2.0.0"
  }
}
```

`vite.config.ts`:
```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ["VITE_", "TAURI_"],
});
```

`tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2021",
    "useDefineForClassFields": true,
    "lib": ["ES2021", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

`tsconfig.node.json`:
```json
{
  "compilerOptions": {
    "composite": true,
    "skipLibCheck": true,
    "module": "ESNext",
    "moduleResolution": "bundler",
    "allowSyntheticDefaultImports": true
  },
  "include": ["vite.config.ts"]
}
```

`index.html`:
```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>颜文字桌宠</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

`src/vite-env.d.ts`:
```ts
/// <reference types="vite/client" />
```

`src/main.tsx`:
```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

`src/App.tsx`:
```tsx
export default function App() {
  return <div>颜文字桌宠</div>;
}
```

- [ ] **Step 2: 安装依赖**

Run: `npm install`
Expected: 无错误，生成 `package-lock.json` 和 `node_modules/`。

- [ ] **Step 3: 验证类型与构建**

Run: `npm run build`
Expected: 输出 `dist/` 且无 TypeScript 错误。

- [ ] **Step 4: 提交**

```bash
git add package.json package-lock.json vite.config.ts tsconfig.json tsconfig.node.json index.html src
git commit -m "feat: 初始化 React + Vite 前端工程"
```

### Task 0.2: 初始化 Tauri 2 工程

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`
- Create: `src-tauri/capabilities/default.json`

- [ ] **Step 1: 写入 Tauri 工程文件**

`src-tauri/Cargo.toml`:
```toml
[package]
name = "yanwenzi"
version = "0.1.0"
description = "颜文字桌宠"
edition = "2021"

[lib]
name = "yanwenzi_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`src-tauri/tauri.conf.json`:
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "yanwenzi",
  "version": "0.1.0",
  "identifier": "com.yanwenzi.pet",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "颜文字桌宠",
        "width": 320,
        "height": 240,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "resizable": false
      }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": [],
    "icon": ["icons/icon.ico"]
  }
}
```

`src-tauri/build.rs`:
```rust
fn main() {
    tauri_build::build()
}
```

`src-tauri/src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    yanwenzi_lib::run();
}
```

`src-tauri/src/lib.rs`:
```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

`src-tauri/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "默认窗口能力",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

- [ ] **Step 2: 生成应用图标**

源图标已放在 `src-tauri/icons/icon.png`（1024x1024，来自用户提供的 `cat_icon.png`）。执行：
Run: `npx tauri icon src-tauri/icons/icon.png`
Expected: 在 `src-tauri/icons/` 下生成 `icon.ico` 等图标。

- [ ] **Step 3: 编译 Rust 工程**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 4: 运行桌面应用验证窗口**

Run: `npm run tauri dev`
Expected: 打开一个透明、无边框、置顶的 320x240 窗口，显示“颜文字桌宠”。

- [ ] **Step 5: 提交**

```bash
git add src-tauri
git commit -m "feat: 初始化 Tauri 2 桌面工程"
```
---

## 阶段 1：桌面壳（透明、置顶、可拖动、托盘）

目标：宠窗可拖动、透明区域点击穿透、系统托盘可退出；补齐窗口标签。

### Task 1.1: 可拖动透明宠窗

**Files:**
- Modify: `src/App.tsx`
- Create: `src/windows/PetWindow.tsx`

- [ ] **Step 1: 编写失败测试（窗口渲染）**

Create `src/windows/PetWindow.test.tsx`:
```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import PetWindow from "./PetWindow";

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ startDragging: vi.fn() }),
}));

describe("PetWindow", () => {
  it("渲染颜文字形象", () => {
    render(<PetWindow />);
    expect(screen.getByText("(・ω・)")).toBeTruthy();
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `npm test -- PetWindow.test.tsx`
Expected: FAIL（`PetWindow` 模块不存在）。

- [ ] **Step 3: 安装测试依赖**

Run: `npm install -D @testing-library/react @testing-library/dom jsdom`
修改 `vite.config.ts`，增加 Vitest 配置：
```ts
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ["VITE_", "TAURI_"],
  test: { environment: "jsdom", globals: true },
});
```

- [ ] **Step 4: 实现 PetWindow**

`src/windows/PetWindow.tsx`:
```tsx
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export default function PetWindow() {
  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button === 0) {
      void getCurrentWindow().startDragging();
    }
  };

  return (
    <div
      style={{ width: "100%", height: "100%", userSelect: "none", background: "transparent" }}
      onPointerDown={onPointerDown}
    >
      <div style={{ fontSize: 48, cursor: "grab", lineHeight: 1 }}>(・ω・)</div>
    </div>
  );
}
```

`src/App.tsx`:
```tsx
import PetWindow from "./windows/PetWindow";

export default function App() {
  return <PetWindow />;
}
```

- [ ] **Step 5: 运行测试确认通过**

Run: `npm test -- PetWindow.test.tsx`
Expected: PASS。

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "feat: 添加可拖动透明宠窗"
```

### Task 1.2: 系统托盘

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`

- [ ] **Step 1: 编写 Rust 托盘逻辑**

`src-tauri/src/lib.rs` 改为：
```rust
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "打开设置", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 3: 手动验证托盘**

Run: `npm run tauri dev`
Expected: 系统托盘出现图标；左键点击显示宠窗；右键菜单有“打开设置”和“退出”，“退出”能关闭应用。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/capabilities/default.json
git commit -m "feat: 添加系统托盘与退出菜单"
```
---

### Task 1.3: 点击穿透与隐藏恢复

**Files:** `src/windows/PetWindow.tsx`、`src-tauri/src/commands.rs`、`src-tauri/src/lib.rs`、`src-tauri/capabilities/default.json`。

- [ ] 建立可交互区域命中规则：颜文字、饰品相关交互、按钮、菜单和确认卡片接收鼠标；其他透明区域穿透到桌面。不能只用 CSS 透明或 pointer-events 代替系统窗口命中测试。
- [ ] 拖动仅由颜文字拖动区域触发，与单击区分；按钮事件不冒泡触发拖动/增加亲密度。缩放、尺寸变化和颜文字位移后更新命中区域。
- [ ] 实现 `set_pet_hidden(hidden)`：隐藏颜文字、饰品和气泡，只保留“显示宠窗”按钮；保存完整窗口尺寸，恢复时还原。隐藏期间暂停活动与游走。
- [ ] 托盘菜单明确提供“显示宠窗”“设置”“退出”；设置项打开设置窗口，显示项解除隐藏。退出关闭所有窗口与后台任务。
- [ ] Windows 手动验收：透明空白点击到下面窗口、颜文字可单击/拖动、控件不触发拖动、隐藏按钮仍可恢复、托盘可恢复；覆盖不同 DPI。

---

## 阶段 2：配置存储（SQLite、DPAPI、提供商与人设 CRUD）

目标：Rust 端可持久化提供商、人设和参数，API Key 加密；前端设置页可增删改查与切换。

### Task 2.1: 数据模型与数据库初始化

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/models.rs`
- Create: `src-tauri/src/db.rs`
- Create: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 添加依赖**

`src-tauri/Cargo.toml` 的 `[dependencies]` 增加：
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
chrono = { version = "0.4", features = ["serde"] }
```

- [ ] **Step 2: 编写模型**

`src-tauri/src/models.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: i64,
    pub name: String,
    pub base_url: Option<String>,
    pub api_key_encrypted: Option<String>,
    pub model: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    pub id: i64,
    pub name: String,
    pub prompt: String,
    pub is_active: bool,
    pub expressions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub hungry_minutes: u64,
    pub bored_minutes: u64,
    pub sleep_min_minutes: u64,
    pub sleep_max_minutes: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hungry_minutes: 120,
            bored_minutes: 45,
            sleep_min_minutes: 60,
            sleep_max_minutes: 120,
        }
    }
}
```

- [ ] **Step 3: 编写数据库初始化与版本入口**

`src-tauri/src/db.rs` 通过 `include_str!("../../db/schema.sql")` 读取唯一完整结构，不另写一套建表 SQL。

1. `init(path)` 建立连接并启用外键，识别新库与已有库。
2. 新库在一个事务中执行完整 schema、创建默认人设（initial_prompt=prompt）和 user_state 单行，并设置当前结构版本后提交。
3. 已有库先按 user_version 运行版本化迁移，再执行幂等种子逻辑；IF NOT EXISTS 不替代迁移。迁移前备份，失败回滚并停止业务启动。
4. 未带版本的旧结构需有明确识别与回填方案；NOT NULL、外键和唯一约束迁移要验证已有数据，不能删除冲突记录来凑过检查。
5. 阶段 10、12、13、14、18 接入功能时字段已经存在；默认人设插入不会引用尚未建好的列。
6. 对空库、重复启动和代表性旧版本升级分别验证，升级后比较字段、外键及索引与完整 schema 一致。

- [ ] **Step 4: 编写共享状态**

`src-tauri/src/state.rs`:
```rust
use rusqlite::Connection;
use std::sync::Mutex;

pub struct AppState {
    pub conn: Mutex<Connection>,
}
```

- [ ] **Step 5: 在应用启动时初始化数据库**

`src-tauri/src/lib.rs` 中 `setup` 闭包内、`run` 之前加入：
```rust
use crate::db;
use crate::state::AppState;
use std::sync::Mutex;
// 在 setup 内：
let exe_dir = std::env::current_exe()?
    .parent()
    .expect("无法获取可执行文件目录")
    .to_path_buf();
let data_dir = exe_dir.join("data");
std::fs::create_dir_all(&data_dir)?;
let conn = db::init(&data_dir.join("yanwenzi.db"))?;
app.manage(AppState { conn: Mutex::new(conn) });
```
同时在 `src-tauri/src/lib.rs` 顶部声明模块：
```rust
mod db;
mod models;
mod state;
```

- [ ] **Step 6: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 7: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/src
git commit -m "feat: 添加 SQLite 数据模型与初始化"
```

### Task 2.2: DPAPI 加解密

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/crypto.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 添加 windows 依赖**

`src-tauri/Cargo.toml` 增加：
```toml
windows = { version = "0.58", features = ["Win32_Security_Cryptography", "Win32_System_Memory"] }
```

- [ ] **Step 2: 编写 DPAPI 加解密**

`src-tauri/src/crypto.rs`:
```rust
use windows::core::PCWSTR;
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN,
};
use windows::Win32::System::Memory::LocalFree;

pub fn protect(plain: &[u8]) -> Result<Vec<u8>, String> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: plain.len() as u32,
        pbData: plain.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    unsafe {
        let ok = CryptProtectData(
            &input,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN.0 as u32,
            &mut output,
        );
        if ok.as_bool() {
            let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            let _ = LocalFree(output.pbData as isize);
            Ok(bytes)
        } else {
            Err("DPAPI 加密失败".to_string())
        }
    }
}

pub fn unprotect(blob: &[u8]) -> Result<Vec<u8>, String> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: blob.len() as u32,
        pbData: blob.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    unsafe {
        let ok = CryptUnprotectData(&input, None, None, None, None, 0, &mut output);
        if ok.as_bool() {
            let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            let _ = LocalFree(output.pbData as isize);
            Ok(bytes)
        } else {
            Err("DPAPI 解密失败".to_string())
        }
    }
}
```

- [ ] **Step 3: 注册模块**

`src-tauri/src/lib.rs` 顶部加 `mod crypto;`。

- [ ] **Step 4: 编写并运行测试**

`src-tauri/src/crypto.rs` 末尾追加：
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_protects_and_unprotects() {
        let plain = b"sk-test-key-123".to_vec();
        let blob = protect(&plain).unwrap();
        assert_ne!(blob, plain);
        let decoded = unprotect(&blob).unwrap();
        assert_eq!(decoded, plain);
    }
}
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml crypto::tests::roundtrip_protects_and_unprotects`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/src/crypto.rs src-tauri/src/lib.rs
git commit -m "feat: 添加 DPAPI 加解密"
```

### Task 2.3: 提供商与人设 CRUD 命令

**Files:**
- Create: `src-tauri/src/providers.rs`
- Create: `src-tauri/src/persona.rs`
- Create: `src-tauri/src/config.rs`
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 编写提供商仓储**

`src-tauri/src/providers.rs`:
```rust
use crate::crypto;
use crate::models::Provider;
use rusqlite::{params, Connection};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ProviderInput {
    pub name: String,
    pub base_url: Option<String>,
    pub api_key: String,
    pub model: String,
}

pub fn list(conn: &Connection) -> Result<Vec<Provider>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, base_url, api_key_encrypted, model, is_active FROM providers ORDER BY id")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Provider {
                id: r.get(0)?,
                name: r.get(1)?,
                base_url: r.get(2)?,
                api_key_encrypted: r.get(3)?,
                model: r.get(4)?,
                is_active: r.get::<_, i64>(5)? != 0,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn create(conn: &Connection, input: ProviderInput) -> Result<i64, String> {
    let enc = crypto::protect(input.api_key.as_bytes())?;
    let encoded = base64_encode(&enc);
    conn.execute(
        "INSERT INTO providers (name, base_url, api_key_encrypted, model, is_active) VALUES (?1, ?2, ?3, ?4, 0)",
        params![input.name, input.base_url, encoded, input.model],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn set_active(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("UPDATE providers SET is_active = 0", []).map_err(|e| e.to_string())?;
    conn.execute("UPDATE providers SET is_active = 1 WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM providers WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn decrypt_key(conn: &Connection, id: i64) -> Result<String, String> {
    let enc: Option<String> = conn
        .query_row("SELECT api_key_encrypted FROM providers WHERE id = ?1", [id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let enc = enc.ok_or("请先配置 API Key")?;
    let blob = base64_decode(&enc)?;
    String::from_utf8(crypto::unprotect(&blob)?).map_err(|e| e.to_string())
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s).map_err(|e| e.to_string())
}
```
并在 `src-tauri/Cargo.toml` 增加 `base64 = "0.22"`。

- [ ] **Step 2: 编写人设仓储**

`src-tauri/src/persona.rs`:
```rust
use crate::models::Persona;
use rusqlite::{params, Connection};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct PersonaInput {
    pub name: String,
    pub prompt: String,
}

pub fn list(conn: &Connection) -> Result<Vec<Persona>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, prompt, is_active, expressions FROM personas ORDER BY id")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Persona {
                id: r.get(0)?,
                name: r.get(1)?,
                prompt: r.get(2)?,
                is_active: r.get::<_, i64>(3)? != 0,
                expressions: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn create(conn: &Connection, input: PersonaInput) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO personas (name, prompt, initial_prompt, is_active, expressions) VALUES (?1, ?2, ?2, 0, NULL)",
        params![input.name, input.prompt],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn set_active(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("UPDATE personas SET is_active = 0", []).map_err(|e| e.to_string())?;
    conn.execute("UPDATE personas SET is_active = 1 WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_expressions(conn: &Connection, id: i64, json: &str) -> Result<(), String> {
    conn.execute("UPDATE personas SET expressions = ?1 WHERE id = ?2", [json, id]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> Result<(), String> {
    // 外键 ON DELETE CASCADE 会自动删除该人设下的所有会话、消息、记忆与记事本。
    conn.execute("DELETE FROM personas WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 3: 编写设置仓储**

`src-tauri/src/config.rs`:
```rust
use crate::models::AppSettings;
use rusqlite::{params, Connection};

pub fn get_settings(conn: &Connection) -> Result<AppSettings, String> {
    let mut s = AppSettings::default();
    let mut stmt = conn.prepare("SELECT key, value FROM settings").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(|e| e.to_string())?;
    for row in rows {
        let (k, v) = row.map_err(|e| e.to_string())?;
        let parsed = v.parse::<u64>().unwrap_or_default();
        match k.as_str() {
            "hungry_minutes" => s.hungry_minutes = parsed,
            "bored_minutes" => s.bored_minutes = parsed,
            "sleep_min_minutes" => s.sleep_min_minutes = parsed,
            "sleep_max_minutes" => s.sleep_max_minutes = parsed,
            _ => {}
        }
    }
    Ok(s)
}

pub fn save_settings(conn: &Connection, s: &AppSettings) -> Result<(), String> {
    let pairs = [
        ("hungry_minutes", s.hungry_minutes),
        ("bored_minutes", s.bored_minutes),
        ("sleep_min_minutes", s.sleep_min_minutes),
        ("sleep_max_minutes", s.sleep_max_minutes),
    ];
    for (k, v) in pairs {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![k, v.to_string()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn get_weather_key(conn: &Connection) -> Result<Option<String>, String> {
    use base64::Engine;
    let encoded: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'weather_qweather_api_key_encrypted'",
            [],
            |r| r.get(0),
        )
        .ok();
    match encoded {
        Some(s) if !s.is_empty() => {
            let blob = base64::engine::general_purpose::STANDARD.decode(s).map_err(|e| e.to_string())?;
            Ok(Some(String::from_utf8(crate::crypto::unprotect(&blob)?).map_err(|e| e.to_string())?))
        }
        _ => Ok(None),
    }
}

pub fn set_weather_key(conn: &Connection, key: &str) -> Result<(), String> {
    use base64::Engine;
    let value = if key.is_empty() {
        String::new()
    } else {
        base64::engine::general_purpose::STANDARD.encode(crate::crypto::protect(key.as_bytes())?)
    };
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('weather_qweather_api_key_encrypted', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 4: 暴露 Tauri commands**

`src-tauri/src/commands.rs`:
```rust
use crate::config;
use crate::models::{AppSettings, Persona, Provider};
use crate::persona::{self, PersonaInput};
use crate::providers::{self, ProviderInput};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn list_providers(state: State<'_, AppState>) -> Result<Vec<Provider>, String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    providers::list(&conn)
}

#[tauri::command]
pub fn create_provider(state: State<'_, AppState>, input: ProviderInput) -> Result<i64, String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    providers::create(&conn, input)
}

#[tauri::command]
pub fn set_active_provider(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    providers::set_active(&conn, id)
}

#[tauri::command]
pub fn delete_provider(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    providers::delete(&conn, id)
}

#[tauri::command]
pub fn list_personas(state: State<'_, AppState>) -> Result<Vec<Persona>, String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    persona::list(&conn)
}

#[tauri::command]
pub fn create_persona(state: State<'_, AppState>, input: PersonaInput) -> Result<i64, String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    persona::create(&conn, input)
}

#[tauri::command]
pub fn set_active_persona(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    persona::set_active(&conn, id)
}

#[tauri::command]
pub fn delete_persona(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    persona::delete(&conn, id)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    config::get_settings(&conn)
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: AppSettings) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    config::save_settings(&conn, &settings)
}

#[tauri::command]
pub fn has_weather_key(state: State<'_, AppState>) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    Ok(config::get_weather_key(&conn)?.is_some())
}

#[tauri::command]
pub fn set_weather_key(state: State<'_, AppState>, key: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    config::set_weather_key(&conn, &key)
}
```

- [ ] **Step 4.1: 补齐编辑、独立记忆选择与切换事务**

- 新增 `update_provider(id, input)`；密钥字段为可选，未提交表示保留，清除使用明确的 clear_key 标志，不能把留空误作清除。
- 新增 `set_memory_provider(id: Option<i64>)`；None 表示跟随聊天，全表清除 memory_is_active。
- provider/persona 当前项切换先验证目标存在，再在同一事务清除旧项并启用目标；适配 schema 的唯一索引。所有模型/API 统一命名 memory_is_active。
- 前端列表 DTO 只返回 has_api_key，不返回密钥明文或密文；后端内部模型可保留加密字段。
- `commands.rs` 和 `lib.rs` 注册新增命令；在 Task 8.6 接入编辑表单和“跟随聊天”选项。

- [ ] **Step 5: 注册命令与模块**

`src-tauri/src/lib.rs` 顶部加：
```rust
mod commands;
mod config;
mod providers;
mod persona;
```
`tauri::Builder::default()` 链上注册：
```rust
.invoke_handler(tauri::generate_handler![
    commands::list_providers,
    commands::create_provider,
    commands::set_active_provider,
    commands::delete_provider,
    commands::list_personas,
    commands::create_persona,
    commands::set_active_persona,
    commands::delete_persona,
    commands::get_settings,
    commands::save_settings,
    commands::has_weather_key,
    commands::set_weather_key
])
```

- [ ] **Step 6: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 7: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/src
git commit -m "feat: 添加提供商、人设与设置 CRUD 命令"
```
---

## 阶段 3：AI 引擎（流式聊天、表情库生成）

目标：Rust 端统一 OpenAI 兼容流式客户端；能生成人设表情库；能“先切表情再流式回复”。

### Task 3.1: 添加异步 HTTP 依赖

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: 添加依赖**

`src-tauri/Cargo.toml` 增加：
```toml
reqwest = { version = "0.12", features = ["json", "stream"] }
tokio = { version = "1", features = ["full"] }
futures-util = "0.3"
```

- [ ] **Step 2: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/Cargo.toml Cargo.lock
git commit -m "chore: 添加异步 HTTP 与流式依赖"
```

### Task 3.2: OpenAI 兼容流式客户端

**Files:**
- Create: `src-tauri/src/ai/mod.rs`
- Create: `src-tauri/src/ai/client.rs`
- Create: `src-tauri/src/ai/streaming.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 编写模块声明**

`src-tauri/src/ai/mod.rs`:
```rust
pub mod client;
pub mod streaming;
```

`src-tauri/src/lib.rs` 顶部加 `mod ai;`。

- [ ] **Step 2: 编写消息与请求结构**

`src-tauri/src/ai/client.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    pub max_tokens: Option<u32>,
}

#[derive(Clone)]
pub struct ApiTarget {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl ApiTarget {
    pub fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }
}
```

- [ ] **Step 3: 编写流式解析**

`src-tauri/src/ai/streaming.rs`:
```rust
use futures_util::StreamExt;
use serde_json::Value;

pub struct ChatStream {
    inner: reqwest::Response,
}

impl ChatStream {
    pub fn new(inner: reqwest::Response) -> Self {
        Self { inner }
    }

    pub async fn next_delta(&mut self) -> Result<Option<String>, String> {
        let Some(bytes) = self.inner.bytes_stream().next().await else {
            return Ok(None);
        };
        let chunk = bytes.map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&chunk);
        let mut delta = String::new();
        for line in text.lines() {
            let line = line.trim();
            if !line.starts_with("data:") {
                continue;
            }
            let payload = line.trim_start_matches("data:").trim();
            if payload == "[DONE]" {
                return Ok(None);
            }
            if let Ok(value) = serde_json::from_str::<Value>(payload) {
                if let Some(content) = value
                    .pointer("/choices/0/delta/content")
                    .and_then(|c| c.as_str())
                {
                    delta.push_str(content);
                }
            }
        }
        if delta.is_empty() {
            Ok(Some(String::new()))
        } else {
            Ok(Some(delta))
        }
    }
}
```

- [ ] **Step 4: 编写流式聊天函数**

`src-tauri/src/ai/client.rs` 追加：
```rust
use crate::ai::streaming::ChatStream;

pub async fn stream_chat(
    target: &ApiTarget,
    messages: Vec<ChatMessage>,
    on_delta: impl Fn(String),
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let body = ChatRequest {
        model: target.model.clone(),
        messages,
        stream: true,
        max_tokens: None,
    };
    let resp = client
        .post(target.endpoint())
        .bearer_auth(&target.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API 错误 {status}: {text}"));
    }
    let mut stream = ChatStream::new(resp);
    let mut full = String::new();
    while let Some(delta) = stream.next_delta().await? {
        if !delta.is_empty() {
            full.push_str(&delta);
            on_delta(delta);
        }
    }
    Ok(full)
}

pub async fn complete_json(
    target: &ApiTarget,
    messages: Vec<ChatMessage>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let body = ChatRequest {
        model: target.model.clone(),
        messages,
        stream: false,
        max_tokens: Some(512),
    };
    let resp = client
        .post(target.endpoint())
        .bearer_auth(&target.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API 错误 {status}: {text}"));
    }
    let value: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    value
        .pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "响应格式错误".to_string())
}
```

- [ ] **Step 5: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/ai src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat: 添加 OpenAI 兼容流式客户端"
```

### Task 3.3: 人设表情库生成

**Files:**
- Create: `src-tauri/src/expressions.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 编写表情库生成**

`src-tauri/src/expressions.rs`:
```rust
use crate::ai::client::{complete_json, ApiTarget, ChatMessage};
use serde_json::Value;
use std::collections::BTreeMap;

pub async fn generate_persona_expressions(
    target: &ApiTarget,
    persona_prompt: &str,
) -> Result<String, String> {
    let system = "你是颜文字表情生成器。请只输出 JSON，不要输出任何解释。格式：{\"点击\":{\"开心\":[\"表情1\",\"表情2\"],\"惊讶\":[],\"害羞\":[]},\"饿\":[],\"吃东西\":[],\"困\":[],\"无聊\":[],\"思考\":[]}。点击下的开心/惊讶/害羞、以及其余分类各 2-3 个，全部用颜文字，不要英文。";
    let messages = vec![
        ChatMessage { role: "system".into(), content: system.into() },
        ChatMessage { role: "user".into(), content: format!("人设：{persona_prompt}") },
    ];
    let raw = complete_json(target, messages).await?;
    let json = extract_json_object(&raw)?;
    normalize_expression_json(&json)
}

fn extract_json_object(raw: &str) -> Result<Value, String> {
    let start = raw.find('{').ok_or("响应中无 JSON")?;
    let end = raw.rfind('}').ok_or("响应中无 JSON")?;
    serde_json::from_str(&raw[start..=end]).map_err(|e| e.to_string())
}

fn normalize_expression_json(value: &Value) -> Result<String, String> {
    let obj = value.as_object().ok_or("表情库不是对象")?;
    let mut out = BTreeMap::new();

    let click_input = obj
        .get("点击")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let mut click = BTreeMap::new();
    for key in ["开心", "惊讶", "害羞"] {
        let arr = click_input
            .get(key)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        click.insert(key.to_string(), Value::Array(arr));
    }
    out.insert("点击".to_string(), Value::Object(click.into_iter().collect()));

    for key in ["饿", "吃东西", "困", "无聊", "思考"] {
        let arr = obj.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default();
        out.insert(key.to_string(), Value::Array(arr));
    }

    serde_json::to_string(&Value::Object(out.into_iter().collect())).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: 注册模块**

`src-tauri/src/lib.rs` 顶部加 `mod expressions;`。

- [ ] **Step 3: 编写并运行测试**

`src-tauri/src/expressions.rs` 末尾追加：
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_missing_categories() {
        let value = serde_json::json!({
            "点击": {
                "开心": ["(≧▽≦)", "(＾▽＾)"],
                "惊讶": ["(°ロ°)"],
                "害羞": ["(*/ω＼*)"]
            },
            "吃东西": ["(＾～＾)ﾓｸﾞﾓｸﾞ"]
        });
        let out = normalize_expression_json(&value).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["点击"]["开心"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["吃东西"].as_array().unwrap().len(), 1);
        assert_eq!(parsed["困"].as_array().unwrap().len(), 0);
    }
}
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml expressions::tests::normalizes_missing_categories`
Expected: PASS。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/expressions.rs src-tauri/src/lib.rs
git commit -m "feat: 添加人设表情库生成"
```

### Task 3.4: 聊天命令（先切表情再流式回复）

**Files:**
- Create: `src-tauri/src/chat.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 编写聊天命令**

`src-tauri/src/chat.rs`:
```rust
use crate::ai::client::{complete_json, stream_chat, ApiTarget, ChatMessage};
use crate::models::Provider;
use crate::providers;
use crate::state::AppState;
use rusqlite::Connection;
use tauri::{AppHandle, Emitter, State};

#[derive(Clone, serde::Serialize)]
pub struct ChatDeltaEvent {
    pub session_id: i64,
    pub delta: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ChatStartEvent {
    pub session_id: i64,
    pub expression: String,
}

pub fn active_target(conn: &Connection) -> Result<ApiTarget, String> {
    let list = providers::list(conn)?;
    let active = list.into_iter().find(|p| p.is_active).ok_or("请先在设置中选择一个 API 提供商")?;
    let key = providers::decrypt_key(conn, active.id)?;
    let base = active.base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    Ok(ApiTarget { base_url: base, api_key: key, model: active.model })
}

pub async fn chat(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: i64,
    user_text: String,
    persona_prompt: String,
    history: Vec<ChatMessage>,
) -> Result<String, String> {
    let target = {
        let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
        active_target(&conn)?
    };

    let mut expression_messages = history.clone();
    expression_messages.push(ChatMessage {
        role: "system".into(),
        content: "先判断下一句回复时角色应该用哪个颜文字表情，只输出一个颜文字，不要解释。".into(),
    });
    expression_messages.push(ChatMessage { role: "user".into(), content: user_text.clone() });
    let expression = complete_json(&target, expression_messages).await?.trim().to_string();

    let _ = app.emit("chat-start", ChatStartEvent { session_id, expression: expression.clone() });

    let mut full_messages = vec![ChatMessage {
        role: "system".into(),
        content: persona_prompt.clone(),
    }];
    full_messages.extend(history);
    full_messages.push(ChatMessage { role: "user".into(), content: user_text });

    let full = stream_chat(&target, full_messages, {
        let app = app.clone();
        move |delta| {
            let _ = app.emit("chat-delta", ChatDeltaEvent { session_id, delta });
        }
    })
    .await?;

    Ok(full)
}
```

- [ ] **Step 2: 注册命令与模块**

`src-tauri/src/commands.rs` 增加：
```rust
#[tauri::command]
pub async fn chat(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    session_id: i64,
    user_text: String,
    persona_prompt: String,
    history: Vec<crate::ai::client::ChatMessage>,
) -> Result<String, String> {
    crate::chat::chat(app, state, session_id, user_text, persona_prompt, history).await
}
```

`src-tauri/src/lib.rs` 顶部加 `mod chat;`，并注册 `commands::chat`。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/chat.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: 添加先切表情再流式回复的聊天命令"
```
---

## 阶段 4：桌宠状态机与活动

目标：饿、无聊、睡觉由可配置时长触发；点击/喂食/摸摸更新状态；状态通过事件推给前端。

### Task 4.1: 桌宠状态机核心

**Files:**
- Create: `src-tauri/src/pet.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: 扩展共享状态**

`src-tauri/src/state.rs` 改为：
```rust
use crate::pet::PetRuntime;
use rusqlite::Connection;
use std::sync::Mutex;

pub struct AppState {
    pub conn: Mutex<Connection>,
    pub pet: PetRuntime,
}
```

- [ ] **Step 2: 编写桌宠状态机**

睡觉状态首次进入时从 `sleep_min_minutes..=sleep_max_minutes` 随机取一次睡眠时长，用户点击/摸摸/喂食可唤醒；下面循环中的 `sleep_max_minutes` 只作为兜底判定。

`src-tauri/src/pet.rs`:
```rust
use crate::config;
use crate::state::AppState;
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PetState {
    Normal,
    Hungry,
    Bored,
    Sleeping,
    Eating,
    Talking,
    Thinking,
    Dormant,
}

#[derive(Clone, Serialize)]
pub struct PetStateEvent {
    pub state: PetState,
}

pub struct PetRuntime {
    pub last_interaction: Mutex<Instant>,
    pub state: Mutex<PetState>,
}

impl Default for PetRuntime {
    fn default() -> Self {
        Self {
            last_interaction: Mutex::new(Instant::now()),
            state: Mutex::new(PetState::Normal),
        }
    }
}

pub fn touch(runtime: &PetRuntime, state: PetState) {
    if let Ok(mut last) = runtime.last_interaction.lock() {
        *last = Instant::now();
    }
    if let Ok(mut current) = runtime.state.lock() {
        *current = state;
    }
}

pub fn start_pet_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(15)).await;
            let state = app.state::<AppState>();
            let settings = {
                let conn = state.conn.lock().unwrap();
                config::get_settings(&conn).unwrap_or_default()
            };
            let last = *state.pet.last_interaction.lock().unwrap();
            let elapsed = last.elapsed().as_secs() / 60;
            let next = if elapsed >= settings.sleep_max_minutes {
                PetState::Sleeping
            } else if elapsed >= settings.hungry_minutes {
                PetState::Hungry
            } else if elapsed >= settings.bored_minutes {
                PetState::Bored
            } else {
                PetState::Normal
            };
            let mut current = state.pet.state.lock().unwrap();
            if *current != PetState::Talking && *current != PetState::Thinking {
                *current = next;
            }
            drop(current);
            let emitted = *state.pet.state.lock().unwrap();
            let _ = app.emit("pet-state", PetStateEvent { state: emitted });
        }
    });
}
```

- [ ] **Step 3: 启动状态机并注册命令**

`src-tauri/src/lib.rs` 顶部加 `mod pet;`。`setup` 中在 `app.manage(AppState { ... })` 之后调用：
```rust
use crate::pet::start_pet_loop;
start_pet_loop(app.handle().clone());
```

`src-tauri/src/commands.rs` 增加：
```rust
#[tauri::command]
pub fn register_interaction(state: tauri::State<'_, AppState>, kind: String) -> Result<(), String> {
    use crate::pet::{touch, PetState};
    let st = match kind.as_str() {
        "feed" => PetState::Eating,
        "pet" => PetState::Normal,
        "click" => PetState::Normal,
        _ => PetState::Normal,
    };
    touch(&state.pet, st);
    Ok(())
}

#[tauri::command]
pub fn set_dormant(state: tauri::State<'_, AppState>) -> Result<(), String> {
    use crate::pet::{touch, PetState};
    touch(&state.pet, PetState::Dormant);
    Ok(())
}
```
并注册 `register_interaction`、`set_dormant`。

- [ ] **Step 4: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/pet.rs src-tauri/src/state.rs src-tauri/src/lib.rs src-tauri/src/commands.rs
git commit -m "feat: 添加桌宠状态机与交互命令"
```

## 阶段 5：聊天会话与记忆

目标：会话按人设隔离、新建/切换/继续、消息持久化与历史查询。

### Task 5.1: 会话与消息仓储

**Files:**
- Create: `src-tauri/src/sessions.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 编写会话仓储**

`src-tauri/src/sessions.rs`:
```rust
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Serialize)]
pub struct Session {
    pub id: i64,
    pub persona_id: i64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct Message {
    pub id: i64,
    pub session_id: i64,
    pub role: String,
    pub content: String,
    pub expression: Option<String>,
    pub created_at: String,
}

pub fn create_session(conn: &Connection, persona_id: i64) -> Result<i64, String> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO sessions (persona_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![persona_id, "新会话", now],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn list_sessions(conn: &Connection, persona_id: i64) -> Result<Vec<Session>, String> {
    let mut stmt = conn
        .prepare("SELECT id, persona_id, title, created_at, updated_at FROM sessions WHERE persona_id = ?1 ORDER BY updated_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([persona_id], |r| {
            Ok(Session {
                id: r.get(0)?,
                persona_id: r.get(1)?,
                title: r.get(2)?,
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn list_messages(conn: &Connection, session_id: i64) -> Result<Vec<Message>, String> {
    let mut stmt = conn
        .prepare("SELECT id, session_id, role, content, expression, created_at FROM messages WHERE session_id = ?1 ORDER BY id ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([session_id], |r| {
            Ok(Message {
                id: r.get(0)?,
                session_id: r.get(1)?,
                role: r.get(2)?,
                content: r.get(3)?,
                expression: r.get(4)?,
                created_at: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn append_message(
    conn: &Connection,
    session_id: i64,
    role: &str,
    content: &str,
    expression: Option<&str>,
) -> Result<i64, String> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO messages (session_id, role, content, expression, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![session_id, role, content, expression, now],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
        params![now, session_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn update_title(conn: &Connection, session_id: i64, title: &str) -> Result<(), String> {
    conn.execute("UPDATE sessions SET title = ?1 WHERE id = ?2", params![title, session_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 2: 注册会话命令**

`src-tauri/src/commands.rs` 增加：
```rust
#[tauri::command]
pub fn create_session(state: tauri::State<'_, AppState>, persona_id: i64) -> Result<i64, String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    crate::sessions::create_session(&conn, persona_id)
}

#[tauri::command]
pub fn list_sessions(state: tauri::State<'_, AppState>, persona_id: i64) -> Result<Vec<crate::sessions::Session>, String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    crate::sessions::list_sessions(&conn, persona_id)
}

#[tauri::command]
pub fn list_messages(state: tauri::State<'_, AppState>, session_id: i64) -> Result<Vec<crate::sessions::Message>, String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    crate::sessions::list_messages(&conn, session_id)
}
```
`src-tauri/src/lib.rs` 顶部加 `mod sessions;`，并注册这三个命令。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/sessions.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: 添加会话与消息持久化"
```
---

## 阶段 6：提醒系统

目标：一次性/重复提醒、运行中弹气泡、关闭期间到期下次询问、状态流转。

### Task 6.1: 提醒规则与每次触发记录

**Files:** `src-tauri/src/reminders.rs`、`models.rs`、`commands.rs`、`lib.rs`。

- [ ] 从完整 schema 接入 reminders 与 reminder_occurrences 模型；后者保存 scheduled_at、status、delivery_state，不再仅用一次性事件保存待提醒信息。
- [ ] 实现规则 CRUD 和下一次时间计算。日/周/月规则统一本地日期计算、UTC 保存；月度不存在的日期跳过。重复规则 status 保持 pending，删除才永久停止。
- [ ] 在线调度事务：插入唯一 occurrence(queued) 并推进 fire_at；一次性 fire_at 可置 NULL，但原时间已经存到 occurrence。后台 AI 润色在事务外执行，失败回退原文。
- [ ] 启动补偿先于常规循环：错过一次性写 awaiting_choice，错过重复只建立最近一次 occurrence 并推进到未来。两者均用唯一约束避免重复。
- [ ] 暴露 `list_pending_occurrences`、`resolve_missed_once(occurrence_id, action)`、`ack_reminder_shown(occurrence_id)`、`complete_reminder_occurrence(occurrence_id)` 和本次标过期命令。完成重复提醒只改本次，完成一次性同时更新规则。
- [ ] 前端启动后主动读取待处理 occurrence，事件只是刷新通知。用户未处理就退出，下一次仍能继续；展示确认前崩溃允许重新展示但不能丢失。
- [ ] 验证：启动时监听未就绪、未选补/过期退出、重复点击完成、调度与补偿同时发生、月末无日期、删除规则级联记录。事务失败不推进游标。

### Task 6.2: 聊天提醒工具

**Files:** `src-tauri/src/reminder_extract.rs`、`chat.rs`、`commands.rs`、`lib.rs`。

- [ ] 提供 `create_reminder` function calling 工具，支持一次性及 daily/weekly/monthly 结构，输入上下文包含当前本地时间与时区。
- [ ] 解析结果交由 Rust 校验日期、循环规则和内容；没有明确时间时请用户补充，不保存无法触发的空时间提醒。
- [ ] 工具调用写入规则后返回 ID；阶段 9 工具循环注册该工具，前端设置页显示结果。不要依靠前端额外猜测用户消息是否要求提醒。
- [ ] 数据库锁仅用于取配置和保存结果，不跨 AI await 持有；模型解析失败不写半成品。
- [ ] 验证一次性、每天、每周和每月聊天创建，以及缺失时间、非法规则；执行 Rust 测试与编译。

---

## 阶段 7：天气与定位

目标：Windows 系统定位 + Open-Meteo/和风天气兜底，注入 AI 上下文。

### Task 7.1: 天气与定位服务

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/location.rs`
- Create: `src-tauri/src/weather.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 添加 windows 定位 feature**

`src-tauri/Cargo.toml` 的 `windows` feature 增加：`"Devices_Geolocation", "Foundation"`。

- [ ] **Step 2: 编写定位服务**

`src-tauri/src/location.rs`:
```rust
use windows::Devices::Geolocation::{Geolocator, GeolocationAccessStatus};

pub fn current_coords() -> Result<(f64, f64), String> {
    let access = Geolocator::RequestAccessAsync()
        .map_err(|e| e.to_string())?
        .get()
        .map_err(|e| e.to_string())?;
    if access != GeolocationAccessStatus::Allowed {
        return Err("未授权使用系统定位".to_string());
    }
    let geolocator = Geolocator::new().map_err(|e| e.to_string())?;
    let pos = geolocator
        .GetGeopositionAsync()
        .map_err(|e| e.to_string())?
        .get()
        .map_err(|e| e.to_string())?;
    let coord = pos.Coordinate().map_err(|e| e.to_string())?;
    Ok((coord.Latitude().map_err(|e| e.to_string())?, coord.Longitude().map_err(|e| e.to_string())?))
}
```

- [ ] **Step 3: 编写天气服务**

`src-tauri/src/weather.rs`:
```rust
use serde::Serialize;

#[derive(Serialize)]
pub struct Weather {
    pub temperature: f64,
    pub windspeed: f64,
    pub weathercode: i64,
    pub is_day: bool,
}

pub async fn fetch_open_meteo(lat: f64, lon: f64) -> Result<Weather, String> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,weather_code,is_day,wind_speed_10m"
    );
    let value: serde_json::Value = reqwest::get(&url).await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let current = value.get("current").ok_or("天气响应格式错误")?;
    Ok(Weather {
        temperature: current.get("temperature_2m").and_then(|v| v.as_f64()).unwrap_or_default(),
        windspeed: current.get("wind_speed_10m").and_then(|v| v.as_f64()).unwrap_or_default(),
        weathercode: current.get("weather_code").and_then(|v| v.as_i64()).unwrap_or_default(),
        is_day: current.get("is_day").and_then(|v| v.as_i64()).map(|v| v == 1).unwrap_or(true),
    })
}

pub async fn fetch_qweather(lat: f64, lon: f64, key: &str) -> Result<Weather, String> {
    let url = format!("https://devapi.qweather.com/v7/weather/now?location={lon},{lat}&key={key}");
    let value: serde_json::Value = reqwest::get(&url).await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    if value.get("code").and_then(|v| v.as_str()) != Some("200") {
        return Err("和风天气接口返回错误".to_string());
    }
    let now = value.get("now").ok_or("和风天气响应格式错误")?;
    Ok(Weather {
        temperature: now.get("temp").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or_default(),
        windspeed: now.get("windSpeed").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or_default(),
        weathercode: now.get("icon").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or_default(),
        is_day: true,
    })
}

pub async fn fetch_weather(lat: f64, lon: f64, qweather_key: Option<&str>) -> Result<Weather, String> {
    if let Ok(weather) = fetch_open_meteo(lat, lon).await {
        return Ok(weather);
    }
    if let Some(key) = qweather_key {
        if !key.is_empty() {
            return fetch_qweather(lat, lon, key).await;
        }
    }
    Err("天气不可用".to_string())
}
```

- [ ] **Step 4: 注册环境查询命令**

`src-tauri/src/commands.rs` 增加：
```rust
#[derive(serde::Serialize)]
pub struct Environment {
    pub time: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub weather: Option<crate::weather::Weather>,
}

#[tauri::command]
pub async fn get_environment(state: tauri::State<'_, crate::AppState>) -> Result<Environment, String> {
    let time = chrono::Utc::now().to_rfc3339();
    let mut latitude = None;
    let mut longitude = None;
    let mut weather = None;
    let qweather_key = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        crate::config::get_weather_key(&conn)?
    };
    if let Ok((lat, lon)) = crate::location::current_coords() {
        latitude = Some(lat);
        longitude = Some(lon);
        weather = crate::weather::fetch_weather(lat, lon, qweather_key.as_deref()).await.ok();
    }
    Ok(Environment { time, latitude, longitude, weather })
}
```
`src-tauri/src/lib.rs` 顶部加 `mod location; mod weather;` 并注册 `commands::get_environment`。

- [ ] **Step 5: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/src/location.rs src-tauri/src/weather.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: 添加天气与定位服务"
```
---

## 阶段 8：多窗口与前端交互

目标：宠窗、聊天窗、设置窗可用；事件与命令在前端打通。

### Task 8.1: 动态创建聊天窗与设置窗

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 添加窗口命令**

`src-tauri/src/commands.rs` 增加：
```rust
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

fn build_child_window(app: &tauri::AppHandle, label: &str, title: &str, width: f64, height: f64) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    WebviewWindowBuilder::new(app, label, WebviewUrl::App(format!("index.html?window={label}").into()))
        .title(title)
        .inner_size(width, height)
        .decorations(true)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn open_chat_window(app: tauri::AppHandle) -> Result<(), String> {
    build_child_window(&app, "chat", "聊天", 420.0, 560.0)
}

#[tauri::command]
pub fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    build_child_window(&app, "settings", "设置", 760.0, 640.0)
}

#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}
```
`src-tauri/src/lib.rs` 注册 `commands::open_chat_window`、`commands::open_settings_window`、`commands::exit_app`。

- [ ] **Step 2: 更新窗口能力**

`src-tauri/capabilities/default.json` 改为：
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "默认窗口能力",
  "windows": ["main", "chat", "settings"],
  "permissions": ["core:default", "core:window:allow-start-dragging"]
}
```

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "feat: 添加聊天窗与设置窗的动态创建"
```

### Task 8.2: 前端类型与 API 封装

**Files:**
- Create: `src/types.ts`
- Create: `src/api.ts`

- [ ] **Step 1: 编写类型**

`src/types.ts`:
```ts
export interface Provider {
  id: number;
  name: string;
  base_url: string | null;
  api_key_encrypted: string | null;
  model: string;
  is_active: boolean;
}

export interface Persona {
  id: number;
  name: string;
  prompt: string;
  is_active: boolean;
  expressions: string | null;
}

export interface AppSettings {
  hungry_minutes: number;
  bored_minutes: number;
  sleep_min_minutes: number;
  sleep_max_minutes: number;
}

export interface Session {
  id: number;
  persona_id: number;
  title: string;
  created_at: string;
  updated_at: string;
}

export interface Message {
  id: number;
  session_id: number;
  role: string;
  content: string;
  expression: string | null;
  created_at: string;
}

export interface Reminder {
  id: number;
  kind: string;
  schedule: string | null;
  content: string;
  status: string;
  fire_at: string | null;
  source: string;
  created_at: string;
}

export type PetState =
  | "normal"
  | "hungry"
  | "bored"
  | "sleeping"
  | "eating"
  | "talking"
  | "thinking"
  | "dormant";
```

- [ ] **Step 2: 编写 API 封装**

`src/api.ts`:
```ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppSettings, Message, Persona, Provider, Reminder, Session } from "./types";

export const listProviders = () => invoke<Provider[]>("list_providers");
export const createProvider = (input: { name: string; base_url: string | null; api_key: string; model: string }) =>
  invoke<number>("create_provider", { input });
export const setActiveProvider = (id: number) => invoke<void>("set_active_provider", { id });
export const deleteProvider = (id: number) => invoke<void>("delete_provider", { id });

export const listPersonas = () => invoke<Persona[]>("list_personas");
export const createPersona = (input: { name: string; prompt: string }) =>
  invoke<number>("create_persona", { input });
export const setActivePersona = (id: number) => invoke<void>("set_active_persona", { id });
export const deletePersona = (id: number) => invoke<void>("delete_persona", { id });

export const getSettings = () => invoke<AppSettings>("get_settings");
export const saveSettings = (settings: AppSettings) => invoke<void>("save_settings", { settings });
export const hasWeatherKey = () => invoke<boolean>("has_weather_key");
export const setWeatherKey = (key: string) => invoke<void>("set_weather_key", { key });

export const createSession = (personaId: number) => invoke<number>("create_session", { personaId });
export const listSessions = (personaId: number) => invoke<Session[]>("list_sessions", { personaId });
export const listMessages = (sessionId: number) => invoke<Message[]>("list_messages", { sessionId });

export const createReminder = (input: {
  kind: string;
  schedule: string | null;
  content: string;
  fire_at: string | null;
  source: string;
}) => invoke<number>("create_reminder", { input });
export const listReminders = () => invoke<Reminder[]>("list_reminders");
export const setReminderStatus = (id: number, status: string) =>
  invoke<void>("set_reminder_status", { id, status });
export const deleteReminder = (id: number) => invoke<void>("delete_reminder", { id });
export const extractReminder = (text: string) => invoke<number>("extract_reminder", { text });

export const openChatWindow = () => invoke<void>("open_chat_window");
export const openSettingsWindow = () => invoke<void>("open_settings_window");
export const registerInteraction = (kind: string) => invoke<void>("register_interaction", { kind });
export const getEnvironment = () =>
  invoke<{ time: string; latitude: number | null; longitude: number | null; weather: unknown | null }>("get_environment");

export const chat = (sessionId: number, userText: string, personaPrompt: string, history: { role: string; content: string }[]) =>
  invoke<string>("chat", { sessionId, userText, personaPrompt, history });

export function onPetState(cb: (state: string) => void): Promise<UnlistenFn> {
  return listen<{ state: string }>("pet-state", (e) => cb(e.payload.state));
}
export function onChatStart(cb: (p: { session_id: number; expression: string }) => void): Promise<UnlistenFn> {
  return listen<{ session_id: number; expression: string }>("chat-start", (e) => cb(e.payload));
}
export function onChatDelta(cb: (p: { session_id: number; delta: string }) => void): Promise<UnlistenFn> {
  return listen<{ session_id: number; delta: string }>("chat-delta", (e) => cb(e.payload));
}
export interface ReminderDuePayload {
  reminder_id: number;
  text: string;
}
export function onReminderDue(cb: (p: ReminderDuePayload) => void): Promise<UnlistenFn> {
  return listen<ReminderDuePayload>("reminder-due", (e) => cb(e.payload));
}
export function onReminderMissed(cb: (p: ReminderDuePayload) => void): Promise<UnlistenFn> {
  return listen<ReminderDuePayload>("reminder-missed", (e) => cb(e.payload));
}
export const resolveMissedOnce = (id: number) => invoke<void>("resolve_missed_once", { id });
```

- [ ] **Step 3: 提交**

```bash
git add src/types.ts src/api.ts
git commit -m "feat: 添加前端类型与 Tauri API 封装"
```

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

`src/components/Kaomoji.tsx`:
```tsx
export default function Kaomoji({ text, size = 48 }: { text: string; size?: number }) {
  return <div style={{ fontSize: size, lineHeight: 1, whiteSpace: "nowrap" }}>{text}</div>;
}
```

`src/components/ErrorBadge.tsx`:
```tsx
export default function ErrorBadge({ message }: { message: string }) {
  return (
    <button
      title={message}
      style={{
        position: "absolute",
        top: 0,
        right: 0,
        color: "white",
        background: "red",
        border: "none",
        borderRadius: "50%",
        width: 20,
        height: 20,
        cursor: "pointer",
      }}
      onClick={() => alert(message)}
    >
      !
    </button>
  );
}
```

`src/components/ContextMenu.tsx`:
```tsx
export interface MenuItem {
  label: string;
  onClick: () => void;
}

export default function ContextMenu({ items }: { items: MenuItem[] }) {
  return (
    <div style={{ position: "absolute", background: "white", border: "1px solid #ddd", borderRadius: 6, zIndex: 10, padding: 4 }}>
      {items.map((item) => (
        <button
          key={item.label}
          onClick={item.onClick}
          style={{ display: "block", width: "100%", textAlign: "left", background: "none", border: "none", padding: "6px 12px", cursor: "pointer" }}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 3: 重写宠窗**

`src/windows/PetWindow.tsx`:
```tsx
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listPersonas, onPetState, openChatWindow, openSettingsWindow, registerInteraction } from "../api";
import ContextMenu from "../components/ContextMenu";
import ErrorBadge from "../components/ErrorBadge";
import Kaomoji from "../components/Kaomoji";
import type { PetState } from "../types";

const FALLBACK: Record<string, string> = {
  normal: "(・ω・)",
  hungry: "(＞﹏＜)",
  bored: "(￣～￣)",
  sleeping: "(－.－)…zzz",
  eating: "(＾～＾)ﾓｸﾞﾓｸﾞ",
  talking: "(｡•̀ᴗ-)✧",
  thinking: "(。・_・。)…?",
  dormant: "(×_×)",
};
function clickExpressions(raw: string): string[] {
  const map = JSON.parse(raw) as Record<string, unknown>;
  const group = map["点击"] as Record<string, string[]> | undefined;
  if (!group) return [];
  return [
    ...(group["开心"] ?? []),
    ...(group["惊讶"] ?? []),
    ...(group["害羞"] ?? []),
  ];
}

export default function PetWindow() {
  const [state, setState] = useState<PetState>("normal");
  const [expression, setExpression] = useState(FALLBACK.normal);
  const [menuOpen, setMenuOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const un = onPetState((s) => {
      setState(s as PetState);
      setExpression(FALLBACK[s] ?? FALLBACK.normal);
    });
    return () => { void un.then((f) => f()); };
  }, []);

  useEffect(() => {
    void listPersonas().then((personas) => {
      const active = personas.find((p) => p.is_active);
      if (active?.expressions) {
        const click = clickExpressions(active.expressions);
        if (click.length > 0) setExpression(click[0]);
      }
    }).catch((e) => setError(String(e)));
  }, []);

  const onClick = () => {
    void registerInteraction("click");
    setError(null);
    void listPersonas().then((personas) => {
      const active = personas.find((p) => p.is_active);
      if (active?.expressions) {
        const click = clickExpressions(active.expressions);
        if (click.length > 0) setExpression(click[Math.floor(Math.random() * click.length)]);
      }
    }).catch((e) => setError(String(e)));
  };

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button === 0) {
      void getCurrentWindow().startDragging();
    }
  };

  return (
    <div
      style={{ width: "100%", height: "100%", position: "relative", background: "transparent", userSelect: "none" }}
      onPointerDown={onPointerDown}
      onClick={onClick}
      onContextMenu={(e) => { e.preventDefault(); setMenuOpen(true); }}
    >
      <Kaomoji text={expression} size={56} />
      {error && <ErrorBadge message={error} />}
      {menuOpen && (
        <ContextMenu
          items={[
            { label: "聊天", onClick: () => { setMenuOpen(false); void openChatWindow(); } },
            { label: "喂食", onClick: () => { setMenuOpen(false); void registerInteraction("feed"); } },
            { label: "摸摸", onClick: () => { setMenuOpen(false); void registerInteraction("pet"); } },
            { label: "设置", onClick: () => { setMenuOpen(false); void openSettingsWindow(); } },
            { label: "退出", onClick: () => { setMenuOpen(false); void invoke("exit_app"); } },
          ]}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 4: 运行测试与构建**

Run: `npm test -- PetWindow.test.tsx`
Expected: PASS（若测试断言默认颜文字需改为 `(・ω・)` 仍然成立）。
Run: `npm run build`
Expected: 无错误。

- [ ] **Step 5: 提交**

```bash
git add src/App.tsx src/windows/PetWindow.tsx src/components
git commit -m "feat: 添加窗口路由与宠窗交互"
```
### Task 8.4: 暴露消息保存与表情生成命令

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/api.ts`

- [ ] **Step 1: 添加命令**

`src-tauri/src/commands.rs` 增加：
```rust
#[tauri::command]
pub fn append_message(
    state: tauri::State<'_, AppState>,
    session_id: i64,
    role: String,
    content: String,
    expression: Option<String>,
) -> Result<i64, String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    crate::sessions::append_message(&conn, session_id, &role, &content, expression.as_deref())
}

#[tauri::command]
pub async fn generate_expressions(
    state: tauri::State<'_, AppState>,
    persona_id: i64,
) -> Result<String, String> {
    let (target, prompt) = {
        let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
        let personas = crate::persona::list(&conn)?;
        let persona = personas.into_iter().find(|p| p.id == persona_id).ok_or("人设不存在")?;
        let target = crate::chat::active_target(&conn)?;
        (target, persona.prompt)
    };
    let json = crate::expressions::generate_persona_expressions(&target, &prompt).await?;
    {
        let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
        crate::persona::set_expressions(&conn, persona_id, &json)?;
    }
    Ok(json)
}
```
`src-tauri/src/lib.rs` 注册 `commands::append_message`、`commands::generate_expressions`。

- [ ] **Step 2: 封装前端 API**

`src/api.ts` 增加：
```ts
export const appendMessage = (sessionId: number, role: string, content: string, expression: string | null) =>
  invoke<number>("append_message", { sessionId, role, content, expression });
export const generateExpressions = (personaId: number) => invoke<string>("generate_expressions", { personaId });
```

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src/api.ts
git commit -m "feat: 暴露消息保存与表情生成命令"
```

### Task 8.5: 聊天窗

**Files:**
- Create: `src/windows/ChatWindow.tsx`
- Create: `src/components/Bubble.tsx`

- [ ] **Step 1: 编写气泡组件**

`src/components/Bubble.tsx`:
```tsx
export default function Bubble({ text }: { text: string }) {
  return (
    <div style={{ maxWidth: 320, padding: "8px 12px", background: "white", borderRadius: 12, boxShadow: "0 1px 4px rgba(0,0,0,0.2)", whiteSpace: "pre-wrap" }}>
      {text}
    </div>
  );
}
```

- [ ] **Step 2: 编写聊天窗**

`src/windows/ChatWindow.tsx`:
```tsx
import { useEffect, useRef, useState } from "react";
import {
  appendMessage,
  chat,
  createSession,
  listMessages,
  listPersonas,
  listSessions,
  onChatDelta,
  onChatStart,
} from "../api";
import Bubble from "../components/Bubble";
import Kaomoji from "../components/Kaomoji";
import type { Message, Persona, Session } from "../types";

export default function ChatWindow() {
  const [persona, setPersona] = useState<Persona | null>(null);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [sessionId, setSessionId] = useState<number | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState("");
  const [expression, setExpression] = useState("(・ω・)");
  const bottomRef = useRef<HTMLDivElement>(null);

  const load = async (sid: number) => {
    setSessionId(sid);
    setMessages(await listMessages(sid));
  };

  useEffect(() => {
    void listPersonas().then(async (list) => {
      const active = list.find((p) => p.is_active) ?? list[0] ?? null;
      setPersona(active);
      if (active) {
        const ss = await listSessions(active.id);
        setSessions(ss);
        const sid = ss[0]?.id ?? (await createSession(active.id));
        await load(sid);
      }
    });
  }, []);

  useEffect(() => {
    const unStart = onChatStart((p) => {
      if (p.session_id === sessionId) setExpression(p.expression);
    });
    const unDelta = onChatDelta((p) => {
      if (p.session_id === sessionId) setStreaming((prev) => prev + p.delta);
    });
    return () => {
      void unStart.then((f) => f());
      void unDelta.then((f) => f());
    };
  }, [sessionId]);

  useEffect(() => { bottomRef.current?.scrollIntoView(); }, [messages, streaming]);

  const send = async () => {
    if (!persona || sessionId == null || input.trim() === "") return;
    const text = input.trim();
    setInput("");
    setStreaming("");
    await appendMessage(sessionId, "user", text, null);
    setMessages((prev) => [...prev, { id: Date.now(), session_id: sessionId, role: "user", content: text, expression: null, created_at: "" }]);
    const history = messages.map((m) => ({ role: m.role, content: m.content }));
    try {
      const full = await chat(sessionId, text, persona.prompt, history);
      await appendMessage(sessionId, "assistant", full, expression);
      setMessages((prev) => [...prev, { id: Date.now() + 1, session_id: sessionId, role: "assistant", content: full, expression, created_at: "" }]);
      setStreaming("");
    } catch (e) {
      setStreaming("");
      setMessages((prev) => [...prev, { id: Date.now() + 1, session_id: sessionId, role: "assistant", content: `出错：${String(e)}`, expression: null, created_at: "" }]);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100vh", fontFamily: "sans-serif" }}>
      <div style={{ display: "flex", gap: 8, padding: 8, alignItems: "center" }}>
        <Kaomoji text={expression} size={28} />
        <select value={sessionId ?? ""} onChange={(e) => { void load(Number(e.target.value)); }}>
          {sessions.map((s) => <option key={s.id} value={s.id}>{s.title}</option>)}
        </select>
        <button onClick={() => { if (persona) { void createSession(persona.id).then((sid) => { setSessions((prev) => [{ id: sid, persona_id: persona.id, title: "新会话", created_at: "", updated_at: "" }, ...prev]); void load(sid); }); } }}>新会话</button>
      </div>
      <div style={{ flex: 1, overflowY: "auto", padding: 8, display: "flex", flexDirection: "column", gap: 8 }}>
        {messages.map((m) => <div key={m.id} style={{ alignSelf: m.role === "user" ? "flex-end" : "flex-start" }}><Bubble text={m.content} /></div>)}
        {streaming && <div style={{ alignSelf: "flex-start" }}><Bubble text={streaming} /></div>}
        <div ref={bottomRef} />
      </div>
      <div style={{ display: "flex", gap: 8, padding: 8 }}>
        <input value={input} onChange={(e) => setInput(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") void send(); }} style={{ flex: 1 }} />
        <button onClick={() => void send()}>发送</button>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: 构建验证**

Run: `npm run build`
Expected: 无错误。

- [ ] **Step 4: 提交**

```bash
git add src/windows/ChatWindow.tsx src/components/Bubble.tsx
git commit -m "feat: 添加流式聊天窗"
```

### Task 8.6: 设置窗

**Files:**
- Create: `src/windows/SettingsWindow.tsx`

- [ ] **Step 1: 编写设置窗**

`src/windows/SettingsWindow.tsx`:
```tsx
import { useEffect, useState } from "react";
import {
  createPersona,
  createProvider,
  deletePersona,
  deleteProvider,
  generateExpressions,
  getSettings,
  hasWeatherKey,
  listPersonas,
  listProviders,
  listReminders,
  saveSettings,
  setActivePersona,
  setActiveProvider,
  setWeatherKey,
  createReminder,
  setReminderStatus,
  deleteReminder,
} from "../api";
import type { AppSettings, Persona, Provider, Reminder } from "../types";

type Tab = "providers" | "personas" | "user_profiles" | "reminders" | "time_weather" | "general" | "pet_status" | "pet_time" | "memory" | "travel" | "accessories";

export default function SettingsWindow() {
  const [tab, setTab] = useState<Tab>("providers");

  const groups: { title: string; items: [Tab, string][] }[] = [
    { title: "全局设置", items: [
      ["providers", "API 提供商"],
      ["personas", "人设"],
      ["user_profiles", "用户设定"],
      ["reminders", "提醒"],
      ["time_weather", "时间与天气"],
      ["general", "通用"],
    ]},
    { title: "桌宠与内容", items: [
      ["pet_status", "桌宠状态"],
      ["pet_time", "时间与桌宠"],
      ["memory", "记忆"],
      ["travel", "出行"],
      ["accessories", "饰品"],
    ]},
  ];

  return (
    <div style={{ display: "flex", height: "100vh", fontFamily: "sans-serif" }}>
      <nav style={{ width: 160, borderRight: "1px solid #ddd", padding: 8 }}>
        {groups.map((group) => (
          <div key={group.title}>
            <div style={{ fontWeight: 700, margin: "8px 0 4px" }}>{group.title}</div>
            {group.items.map(([key, label]) => (
              <button key={key} onClick={() => setTab(key)} style={{ display: "block", width: "100%", textAlign: "left", padding: "6px 0", background: tab === key ? "#eef" : "none", border: "none", cursor: "pointer" }}>
                {label}
              </button>
            ))}
          </div>
        ))}
      </nav>
      <main style={{ flex: 1, overflowY: "auto", padding: 16 }}>
        {tab === "providers" && <ProvidersTab />}
        {tab === "personas" && <PersonasTab />}
        {tab === "user_profiles" && <div>用户设定（Task 10.6 补齐）</div>}
        {tab === "reminders" && <RemindersTab />}
        {tab === "time_weather" && <TimeWeatherTab />}
        {tab === "general" && <div>通用：开机自启、版本、导入与导出（阶段 17 补齐）</div>}
        {tab === "pet_status" && <div>桌宠状态：亲密度、喵币、签到、游戏（阶段 13 补齐）</div>}
        {tab === "pet_time" && <TimePetTab />}
        {tab === "memory" && <div>记忆：参数与小窗入口（阶段 9/12 补齐）</div>}
        {tab === "travel" && <div>出行：邮箱、伴手礼、历史（阶段 14/16 补齐）</div>}
        {tab === "accessories" && <div>饰品：当前会话饰品（阶段 15 补齐）</div>}
      </main>
    </div>
  );
}
function ProvidersTab() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [name, setName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");

  const refresh = () => { void listProviders().then(setProviders); };
  useEffect(refresh, []);

  return (
    <div>
      <h2>API 提供商</h2>
      {providers.map((p) => (
        <div key={p.id} style={{ display: "flex", gap: 8, alignItems: "center", marginBottom: 8 }}>
          <span>{p.name}</span>
          <span>{p.model}</span>
          <button onClick={() => { void setActiveProvider(p.id).then(refresh); }}>{p.is_active ? "使用中" : "设为当前"}</button>
          <button onClick={() => { void deleteProvider(p.id).then(refresh); }}>删除</button>
        </div>
      ))}
      <h3>添加提供商</h3>
      <input placeholder="名称" value={name} onChange={(e) => setName(e.target.value)} />
      <input placeholder="base URL（自定义接口）" value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} />
      <input placeholder="API Key" type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} />
      <input placeholder="模型名" value={model} onChange={(e) => setModel(e.target.value)} />
      <button onClick={() => { void createProvider({ name, base_url: baseUrl || null, api_key: apiKey, model }).then(refresh); }}>添加</button>
    </div>
  );
}

function PersonasTab() {
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [name, setName] = useState("");
  const [prompt, setPrompt] = useState("");

  const refresh = () => { void listPersonas().then(setPersonas); };
  useEffect(refresh, []);

  return (
    <div>
      <h2>人设</h2>
      {personas.map((p) => (
        <div key={p.id} style={{ display: "flex", gap: 8, alignItems: "center", marginBottom: 8 }}>
          <span>{p.name}</span>
          <button onClick={() => { void setActivePersona(p.id).then(refresh); }}>{p.is_active ? "使用中" : "设为当前"}</button>
          <button onClick={() => { void generateExpressions(p.id).then(refresh).catch((e) => alert(String(e))); }}>生成表情库</button>
          <button onClick={() => { void deletePersona(p.id).then(refresh); }}>删除</button>
        </div>
      ))}
      <h3>添加人设</h3>
      <input placeholder="名称" value={name} onChange={(e) => setName(e.target.value)} />
      <textarea placeholder="prompt 人设" value={prompt} onChange={(e) => setPrompt(e.target.value)} />
      <button onClick={() => { void createPersona({ name, prompt }).then(refresh); }}>添加</button>
    </div>
  );
}

function TimeWeatherTab() {
  const [weatherKey, setWeatherKeyValue] = useState("");
  const [weatherKeySet, setWeatherKeySet] = useState(false);
  useEffect(() => { void hasWeatherKey().then(setWeatherKeySet); }, []);
  return (
    <div>
      <h2>时间与天气</h2>
      <label style={{ display: "block", marginBottom: 8 }}>
        和风天气 API Key（可选，{weatherKeySet ? "已配置" : "未配置"}）
        <input type="password" placeholder="留空表示不修改" value={weatherKey} onChange={(e) => setWeatherKeyValue(e.target.value)} />
      </label>
      <button onClick={() => { void setWeatherKey(weatherKey).then(() => setWeatherKeySet(true)); }}>保存</button>
    </div>
  );
}

function TimePetTab() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  useEffect(() => { void getSettings().then(setSettings); }, []);
  if (!settings) return <div>加载中…</div>;
  const fields: [keyof AppSettings, string][] = [
    ["hungry_minutes", "饥饿（分钟）"],
    ["bored_minutes", "无聊（分钟）"],
    ["sleep_min_minutes", "睡眠最短（分钟）"],
    ["sleep_max_minutes", "睡眠最长（分钟）"],
  ];
  return (
    <div>
      <h2>时间与桌宠</h2>
      {fields.map(([key, label]) => (
        <label key={key} style={{ display: "block", marginBottom: 8 }}>
          {label}
          <input type="number" value={settings[key]} onChange={(e) => setSettings({ ...settings, [key]: Number(e.target.value) })} />
        </label>
      ))}
      <button onClick={() => { void saveSettings(settings); }}>保存</button>
    </div>
  );
}
function RemindersTab() {
  const [reminders, setReminders] = useState<Reminder[]>([]);
  const [content, setContent] = useState("");
  const [fireAt, setFireAt] = useState("");

  const refresh = () => { void listReminders().then(setReminders); };
  useEffect(refresh, []);

  return (
    <div>
      <h2>提醒</h2>
      {reminders.map((r) => (
        <div key={r.id} style={{ display: "flex", gap: 8, alignItems: "center", marginBottom: 8 }}>
          <span>{r.content}</span>
          <span>{r.fire_at ?? ""}</span>
          <span>{r.status}</span>
          <button onClick={() => { void setReminderStatus(r.id, "done").then(refresh); }}>完成</button>
          <button onClick={() => { void setReminderStatus(r.id, "expired").then(refresh); }}>过期</button>
          <button onClick={() => { void deleteReminder(r.id).then(refresh); }}>删除</button>
        </div>
      ))}
      <h3>手动添加</h3>
      <input placeholder="内容" value={content} onChange={(e) => setContent(e.target.value)} />
      <input placeholder="触发时间（RFC3339）" value={fireAt} onChange={(e) => setFireAt(e.target.value)} />
      <button onClick={() => { void createReminder({ kind: "once", schedule: null, content, fire_at: fireAt || null, source: "manual" }).then(refresh); }}>添加</button>
    </div>
  );
}

```

- [ ] **Step 1.1: 补齐提供商设置与提醒状态交互**

提供商列表接入 update_provider，编辑已保存项目；密钥留空保留、清除明确区分。添加独立记忆提供商选择器和“跟随聊天提供商”。调用后刷新状态，模型/API 使用 has_api_key 与 memory_is_active。
提醒页面接入规则和 occurrence：列出本次原定时间、下一次时间；完成按钮传 occurrence ID，删除才停止重复规则。补提醒选择从数据库加载，不能仅监听事件。

- [ ] **Step 2: 构建验证**

Run: `npm run build`
Expected: 无错误。

- [ ] **Step 3: 提交**

```bash
git add src/windows/SettingsWindow.tsx
git commit -m "feat: 添加设置窗"
```

---

## 阶段 9：记忆与记事本（会话级）

目标：按会话维护记事本与短期/长期记忆；聊天上下文按新规则组装；记忆小窗可查看/编辑/合并。

### Task 9.1: 扩展数据模型

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/config.rs`

- [ ] **Step 1: 添加模型**

`src-tauri/src/models.rs` 追加：
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortMemory {
    pub id: i64,
    pub session_id: i64,
    pub content: String,
    pub source_type: String,
    pub source_ref_id: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongMemory {
    pub id: i64,
    pub session_id: i64,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notebook {
    pub session_id: i64,
    pub content: String,
    pub updated_at: String,
}
```
Provider 及公开摘要同步 memory_is_active: bool；`providers.rs` 的 `list`/`create`/`set_active` 查询与字段同步 `providers.memory_is_active` 列（该列已包含在 `db.rs::init` 的建表语句中）。并在 `AppSettings` 增加字段：
```rust
pub short_memory_min_chars: usize,
pub short_memory_max_chars: usize,
pub long_memory_min_chars: usize,
pub long_memory_max_chars: usize,
pub merge_memory_min_chars: usize,
pub merge_memory_max_chars: usize,
pub tool_call_rounds: usize,
pub notebook_max_chars: usize,
```
默认值在 `Default` 中补齐：短期 40–80，长期 80–160，合并 80–160，工具轮数 3，记事本 4000。

- [ ] **Step 2: 确认建表**

short_memories、short_memory_messages、long_memories、notebooks 与版本字段均已由完整 schema 初始化；本 Task 不重复建表，来源 ID 使用非空 i64。

- [ ] **Step 3: 扩展设置读写**

`src-tauri/src/config.rs` 的 `get_settings`/`save_settings` 增加这些键：`short_memory_min_chars`、`short_memory_max_chars`、`long_memory_min_chars`、`long_memory_max_chars`、`merge_memory_min_chars`、`merge_memory_max_chars`、`tool_call_rounds`、`notebook_max_chars`。读写方式与现有 `hungry_minutes` 一致。

- [ ] **Step 4: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/models.rs src-tauri/src/db.rs src-tauri/src/config.rs
git commit -m "feat: 扩展记忆与记事本数据模型"
```

### Task 9.2: 记事本工具与聊天 function calling

**Files:**
- Modify: `src-tauri/src/ai/client.rs`
- Modify: `src-tauri/src/ai/streaming.rs`
- Create: `src-tauri/src/notebook.rs`
- Modify: `src-tauri/src/chat.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 在请求结构中加入 tools**

`src-tauri/src/ai/client.rs` 的 `ChatRequest` 增加：
```rust
pub tools: Option<Vec<serde_json::Value>>,
pub tool_choice: Option<String>,
```
并在构造请求时默认 `tools: None, tool_choice: None`。

- [ ] **Step 2: 扩展流式解析支持 tool_calls**

`src-tauri/src/ai/streaming.rs` 的 `next_delta` 在解析 `choices/0/delta` 时，同时收集：
- `delta.content` → 文本。
- `delta.tool_calls` 数组中的 `index`、`id`、`function.name`、`function.arguments`（参数是分片拼接）。
把工具调用信息通过返回值一并输出。为保持简单，将返回值改为结构体：
```rust
pub struct Delta {
    pub text: String,
    pub tool_calls: Vec<ToolCallDelta>,
}
pub struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: String,
}
```

- [ ] **Step 3: 编写记事本仓储与工具执行**

`src-tauri/src/notebook.rs`:
```rust
use crate::models::Notebook;
use chrono::Utc;
use rusqlite::{params, Connection};

pub fn get(conn: &Connection, session_id: i64) -> Result<String, String> {
    let row: Option<String> = conn
        .query_row("SELECT content FROM notebooks WHERE session_id = ?1", [session_id], |r| r.get(0))
        .ok();
    Ok(row.unwrap_or_default())
}

pub fn save(conn: &Connection, session_id: i64, content: &str) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO notebooks (session_id, content, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(session_id) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
        params![session_id, content, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn apply_operation(current: &str, op: &str, section_index: Option<usize>, content: &str) -> Result<String, String> {
    let mut parts: Vec<&str> = current.split("\n\n").filter(|p| !p.trim().is_empty()).collect();
    match op {
        "append" => Ok(format!("{current}{}{content}", if current.is_empty() { "" } else { "\n\n" })),
        "replace" => {
            let i = section_index.ok_or("缺少 section_index")?.saturating_sub(1);
            if i >= parts.len() { return Err("段落序号超出范围".to_string()); }
            parts[i] = content;
            Ok(parts.join("\n\n"))
        }
        "delete" => {
            let i = section_index.ok_or("缺少 section_index")?.saturating_sub(1);
            if i >= parts.len() { return Err("段落序号超出范围".to_string()); }
            parts.remove(i);
            Ok(parts.join("\n\n"))
        }
        _ => Err("未知操作".to_string()),
    }
}
```

- [ ] **Step 4: 改造聊天命令以支持工具循环**

`src-tauri/src/chat.rs` 的 `chat` 函数改为：先组装上下文（见 Task 9.4 的 `assemble`），然后循环最多 `tool_call_rounds` 次。同时把 `ai/client.rs` 的 `stream_chat` 改造为返回 `(String, Vec<ToolCallDelta>)`：正文仍逐字回调，同时累积工具调用。
1. 调用改造后的 `stream_chat` 流式输出正文，并收集 tool_calls。
2. 若有工具调用：执行 `read_notebook`（返回记事本全文）或 `update_notebook`（按结构化操作更新，并检查字数上限）；把工具结果作为 `role: "tool"` 消息追加，继续下一轮。
3. 无工具调用则结束，返回完整文本。
最终把 `expression` 决策保持在说话前，逻辑不变。

- [ ] **Step 5: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/ai src-tauri/src/notebook.rs src-tauri/src/chat.rs src-tauri/src/lib.rs
git commit -m "feat: 添加记事本工具与聊天 function calling"
```

### Task 9.3: 记忆生成、来源关联与事务

**Files:** `src-tauri/src/memory.rs`、`chat.rs`、`models.rs`、`commands.rs`、`lib.rs`。

- [ ] 每个会话只运行一个自动整合任务。读取固定消息 ID、正文、summary_cursor 和 memory_revision 快照，再释放锁调用记忆 provider。
- [ ] 最近 30 条之外的消息每 10 条生成一个摘要；chat 来源 ID 为 session_id，同时写 short_memory_messages 的精确消息关联。
- [ ] AI 返回后开启短事务，验证会话和输入仍存在且游标/版本未变化；插入短期记忆、来源关联、更新游标及版本一同提交，失败全部回滚。快照过期则丢弃并按最新状态重新检查。
- [ ] 固定 10 条短期记忆 ID 和内容生成长期记忆；提交前复核，在一个事务插入长期记忆并删除这组 ID，禁止生成后重查“最早 10 条”来删除。
- [ ] 自动/手动记忆更新、消息新增/编辑/删除均增加 memory_revision。手动合并预览携带版本和 ID，确认替换时同样校验。
- [ ] Rust 保存完整 AI 回复后触发后台整合，不在前端另行保存回复；整合失败仅提示，不进入休眠。
- [ ] 验证整合期间编辑/删除输入、两个请求同时触发、插入摘要后模拟失败、用户确认过期合并预览，确认无误删/重复/部分提交。

### Task 9.4: 上下文组装与记忆 CRUD 命令

**Files:**
- Create: `src-tauri/src/context.rs`
- Create: `src-tauri/src/memory_commands.rs`
- Modify: `src-tauri/src/chat.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 编写上下文组装**

`src-tauri/src/context.rs`:
```rust
use crate::ai::client::ChatMessage;
use crate::memory;
use crate::models::AppSettings;
use crate::notebook;
use crate::sessions;
use crate::state::AppState;
use rusqlite::Connection;
use tauri::Manager;

pub fn assemble(
    app: &tauri::AppHandle,
    session_id: i64,
    persona_prompt: &str,
    user_profile: Option<&str>,
    environment: &str,
    tools: &str,
    memory_params: &str,
    _settings: &AppSettings,
) -> Result<Vec<ChatMessage>, String> {
    let state = app.state::<AppState>();
    let conn = state.conn.lock().unwrap();
    let long: Vec<String> = {
        let mut stmt = conn.prepare("SELECT content FROM long_memories WHERE session_id = ?1 ORDER BY id ASC").map_err(|e| e.to_string())?;
        let rows = stmt.query_map([session_id], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };
    let short: Vec<String> = {
        let mut stmt = conn.prepare("SELECT content FROM short_memories WHERE session_id = ?1 ORDER BY id ASC").map_err(|e| e.to_string())?;
        let rows = stmt.query_map([session_id], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };
    let recent = sessions::list_messages(&conn, session_id)?;
    let recent = recent.into_iter().rev().take(30).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>();

    let mut out = vec![ChatMessage { role: "system".into(), content: "你是桌宠角色，按人设回复。".into() }];
    out.push(ChatMessage { role: "system".into(), content: environment.to_string() });
    out.push(ChatMessage { role: "system".into(), content: tools.to_string() });
    out.push(ChatMessage { role: "system".into(), content: memory_params.to_string() });
    if let Some(profile) = user_profile {
        out.push(ChatMessage { role: "system".into(), content: format!("用户设定：{profile}") });
    }
    out.push(ChatMessage { role: "system".into(), content: format!("人设：{persona_prompt}") });
    if !long.is_empty() { out.push(ChatMessage { role: "system".into(), content: format!("长期记忆：\n{}", long.join("\n")) }); }
    if !short.is_empty() { out.push(ChatMessage { role: "system".into(), content: format!("短期记忆：\n{}", short.join("\n")) }); }
    for m in recent { out.push(ChatMessage { role: m.role, content: m.content }); }
    Ok(out)
}

```

- [ ] **Step 2: 让聊天命令改用上下文组装**

`src-tauri/src/commands.rs` 的 `chat` 不再接收 `persona_prompt` 和 `history`，改为接收 `session_id` 与 `user_text`。`chat.rs` 中：

1. 在 Rust 事务中保存当前用户消息一次并更新会话版本，再从 `sessions` 查询当前会话、所属人设和 `user_profile_id`；绑定用户设定时读取 `user_profiles.content`，未绑定为 `None`。
2. 读取当前人设 prompt、环境文本、工具说明和记忆参数，调用 `context::assemble` 生成 `Vec<ChatMessage>`。
3. 先用 `complete_json` 生成说话表情并 `emit("chat-start", ...)`。
4. 用户消息已经由 Rust 保存，最近消息包含它；直接调用 stream_chat，不再次追加当前用户消息。
5. 删除前端发送流程中的 appendMessage(user/assistant)；Rust 保存最终 AI 正文与表情一次。已有阶段的前端写库代码必须在本 Task 同步移除。

前端 API 类型同步更新为 `chat(sessionId, userText)`，不再传历史数组；历史由 Rust 从数据库读取。

- [ ] **Step 3: 编写记忆 CRUD 命令**

`src-tauri/src/memory_commands.rs` 增加 `list_short_memories`、`list_long_memories`、`get_notebook`、`save_notebook`、`update_short_memory`、`delete_short_memory`、`update_long_memory`、`delete_long_memory`、`merge_long_memories`。其中 `merge_long_memories` 接收 `ids: Vec<i64>` 和字数范围，调用记忆 AI 合并并返回结果文本；确认后由前端再次调用 `replace_long_memories` 完成替换。

- [ ] **Step 4: 注册模块与命令**

`src-tauri/src/lib.rs` 顶部加 `mod context; mod memory_commands;`，并注册这些命令。

- [ ] **Step 5: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/context.rs src-tauri/src/memory_commands.rs src-tauri/src/chat.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: 添加上下文组装、聊天接入与记忆 CRUD 命令"
```

### Task 9.5: 记忆管理小窗（前端）

**Files:**
- Modify: `src-tauri/src/commands.rs` 增加 `open_memory_window`
- Modify: `src-tauri/capabilities/default.json` 增加 `"memory"` 窗口
- Create: `src/windows/MemoryWindow.tsx`
- Modify: `src/App.tsx`
- Modify: `src/api.ts`

- [ ] **Step 1: 添加记忆窗口命令**

`src-tauri/src/commands.rs` 增加 `open_memory_window(app: tauri::AppHandle)`，用 `build_child_window(&app, "memory", "记忆", 560.0, 640.0)`；并在 `capabilities/default.json` 的 `windows` 数组加入 `"memory"`，注册命令。

- [ ] **Step 2: 编写记忆小窗**

`src/windows/MemoryWindow.tsx`：打开时从聊天窗同步的 `currentSessionId`（通过 Rust 全局状态或事件）读取；展示短期记忆列表、长期记忆列表、记事本文本域；短期/长期每条提供“编辑/删除”；长期记忆支持勾选连续项后“合并”，合并结果弹窗可编辑，确认后调用替换命令；短期记忆显示来源和来源 ID；手动合并失败在小窗内提示，确认携带选中记录版本，过期则要求重新预览。

- [ ] **Step 3: 更新路由与 API**

`src/App.tsx` 增加 `if (windowType === "memory") return <MemoryWindow />;`；`src/api.ts` 增加 `openMemoryWindow`、`listShortMemories`、`listLongMemories`、`getNotebook`、`saveNotebook`、`updateShortMemory`、`deleteShortMemory`、`updateLongMemory`、`deleteLongMemory`、`mergeLongMemories`、`replaceLongMemories`。

- [ ] **Step 4: 构建验证**

Run: `npm run build`
Expected: 无错误。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/commands.rs src-tauri/capabilities/default.json src/windows/MemoryWindow.tsx src/App.tsx src/api.ts
git commit -m "feat: 添加记忆管理小窗"
```

### Task 9.6: 聊天窗接入记忆上下文与记忆入口

**Files:**
- Modify: `src/windows/ChatWindow.tsx`
- Modify: `src/api.ts`

- [ ] **Step 1: 在聊天窗添加“记忆”入口并传递当前会话**

`src/windows/ChatWindow.tsx` 顶部增加“记忆”按钮，点击调用 `openMemoryWindow()`；切换会话时调用新命令 `set_current_session(sessionId)`，供记忆小窗读取。

- [ ] **Step 2: 暴露当前会话命令**

`src-tauri/src/commands.rs` 增加 `set_current_session(state, session_id)` 与 `get_current_session(state)`，在 AppState 中维护 current_session，并在 settings.current_session_id 持久化；启动时校验恢复。切换命令按持久化设计第 3 节取消原会话旅行与待回信，所有窗口读取同一状态。

- [ ] **Step 3: 构建验证**

Run: `npm run build` 与 `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 4: 提交**

```bash
git add src/windows/ChatWindow.tsx src/api.ts src-tauri/src/commands.rs src-tauri/src/state.rs
git commit -m "feat: 聊天窗接入记忆入口与当前会话同步"
```

---

### Task 9.7: 聊天请求、中断与可靠保存

**Files:** `src-tauri/src/chat.rs`、`ai/client.rs`、`ai/streaming.rs`、`state.rs`、`commands.rs`、`src/windows/ChatWindow.tsx`、`src/api.ts`。

- [ ] 为聊天请求分配 request_id，同一会话只允许一个执行中的请求；start/delta/end/error 事件携带 session_id 和 request_id。
- [ ] 暴露 cancel_chat(request_id)，覆盖表情请求、正文流、工具执行边界；前端发送按钮变为暂停。切换/删除会话或关闭聊天窗时取消当前请求。
- [ ] Rust 保存收到的正文一次；无正文不插空消息，中断不增加亲密度、不执行完成后自调，已提交工具操作不回滚。丢失前端事件不影响持久化。
- [ ] 正常发送顺序：Rust 保存用户消息 → 组装含它的最近 30 条 → 生成表情和正文 → Rust 保存 AI 消息 → 增加完整轮次亲密度 → 后台记忆。前端不追加重复用户消息或再次保存 AI。
- [ ] 工具调用消息模型包含 tool_call_id 与 assistant.tool_calls，按调用 ID 拼接结果；设置轮次上限，超限不得无限递归。
- [ ] 流解析保持跨网络块缓冲，支持 UTF-8 和 SSE 事件被拆分；不把网络块当完整事件，不丢弃与结束标志同批到达的正文。
- [ ] 验证暂停前后、表情阶段取消、工具阶段取消、窗口关闭、请求晚到事件、重复发送以及正文仅保存一次；测试使用可控流，不依赖真实 API。

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

`src-tauri/src/sessions.rs` 增加：
```rust
pub fn delete_session(conn: &Connection, session_id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM sessions WHERE id = ?1", [session_id]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn rename_session(conn: &Connection, session_id: i64, title: &str) -> Result<(), String> {
    conn.execute("UPDATE sessions SET title = ?1 WHERE id = ?2", params![title, session_id]).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: 暴露命令**

`src-tauri/src/commands.rs` 增加：
```rust
#[tauri::command]
pub fn delete_session(state: tauri::State<'_, AppState>, session_id: i64) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    crate::sessions::delete_session(&conn, session_id)
}

#[tauri::command]
pub fn rename_session(state: tauri::State<'_, AppState>, session_id: i64, title: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|_| "状态锁失败".to_string())?;
    crate::sessions::rename_session(&conn, session_id, &title)
}
```
注册到 `lib.rs`。

- [ ] **Step 3: 会话标题 AI 生成**

`src-tauri/src/chat.rs` 的 `chat` 在第一次发送完成后，若会话标题仍为“新会话”，调用标题生成逻辑：
```rust
async fn maybe_generate_title(conn: &Connection, target: &ApiTarget, session_id: i64, user_text: &str) -> Result<(), String> {
    let current: String = conn.query_row("SELECT title FROM sessions WHERE id = ?1", [session_id], |r| r.get(0)).unwrap_or_default();
    if current != "新会话" { return Ok(()); }
    let system = "你是会话标题生成器。根据用户第一条消息生成一个不超过 12 个中文字符的标题，只输出标题。";
    let messages = vec![
        ChatMessage { role: "system".into(), content: system.into() },
        ChatMessage { role: "user".into(), content: user_text.to_string() },
    ];
    let title = complete_json(target, messages).await?.trim().chars().take(12).collect::<String>();
    crate::sessions::rename_session(conn, session_id, &title)
}
```

- [ ] **Step 4: 聊天窗会话与聊天记录查看**

`ChatWindow.tsx` 展示当前人设下的会话列表，可切换会话、查看历史消息、重命名、删除；删除操作需要确认弹窗。

- [ ] **Step 5: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/sessions.rs src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/chat.rs src/windows/ChatWindow.tsx src/api.ts src/types.ts
git commit -m "feat: 添加会话删除、重命名与标题生成"
```

### Task 10.2: 消息编辑、删除与重说

**Files:** `src-tauri/src/sessions.rs`、`memory.rs`、`chat.rs`、`commands.rs`、`src/windows/ChatWindow.tsx`、`src/api.ts`。

- [ ] 编辑/删除消息前按 short_memory_messages 查固定关联短期记忆 ID；在一个事务内删除这些短期记忆、修改/删除消息、增加 memory_revision。后续消息、旅行短期记忆和长期记忆保留；不回退 summary_cursor。
- [ ] 前端确认文案说明“相关短期记忆会删除，长期记忆需手动修改”，操作成功刷新消息与记忆窗口。
- [ ] 暴露 regenerate_last_reply(session_id, message_id)，只有最后一条是 AI 且有对应用户消息才允许；组装上下文排除旧 AI 回复，不重复插入用户消息。
- [ ] 新回复成功后事务校验最后一条和版本仍匹配，再覆盖原 AI 消息及表情、删除与旧消息关联的短期记忆；失败/中断保留旧回复。
- [ ] 重说不重复增加亲密度，不重放已有有副作用工具；普通发送与重说各有明确保存路径。
- [ ] 验证一条消息对应的短期记忆精确删除、无关摘要保留、摘要已转长期、重说失败、中断和消息被并发修改。

### Task 10.3: 开机自启与默认人设

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `package.json`
- Create: `src-tauri/src/autostart.rs`

- [ ] **Step 1: 添加 autostart 插件**

`src-tauri/Cargo.toml` 增加 `tauri-plugin-autostart = "2"`；`src-tauri/src/lib.rs` 注册 `.plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))`。前端 `package.json` 增加 `@tauri-apps/plugin-autostart`，`src/api.ts` 封装 `enable`/`disable`/`isEnabled`。

- [ ] **Step 2: 默认人设种子**

`src-tauri/src/db.rs` 的 `init` 建表后执行：
```rust
let count: i64 = conn.query_row("SELECT COUNT(*) FROM personas", [], |r| r.get(0))?;
if count == 0 {
    conn.execute(
        "INSERT INTO personas (name, prompt, initial_prompt, self_evolve_enabled, last_evolved_at, color, chat_bg_file, chat_bg_opacity, chat_bg_blur, is_active, expressions) VALUES ('喵喵桌宠', '你是一只可爱的猫咪桌宠，用颜文字和温柔语气陪伴用户。', '你是一只可爱的猫咪桌宠，用颜文字和温柔语气陪伴用户。', 1, NULL, '#ffb6c1', NULL, 85, 0, 1, NULL)",
        [],
    )?;
}
```

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs src-tauri/src/db.rs src-tauri/src/autostart.rs package.json src/api.ts
git commit -m "feat: 添加开机自启与默认人设"
```

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
Expected: 生成 `src-tauri/target/release/颜喵 YanMew.exe`（便携版，无安装包）。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/tauri.conf.json index.html package.json
git commit -m "chore: 更新应用名为颜喵 YanMew"
```
---

### Task 10.5: 宠窗默认位置与位置记忆

**Files:**
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 增加位置读写**

`src-tauri/src/config.rs` 增加：
```rust
pub fn get_window_position(conn: &Connection) -> Result<Option<(i32, i32)>, String> {
    let x = conn
        .query_row("SELECT value FROM settings WHERE key = 'window_pos_x'", [], |r| r.get::<_, String>(0))
        .ok()
        .and_then(|s| s.parse::<i32>().ok());
    let y = conn
        .query_row("SELECT value FROM settings WHERE key = 'window_pos_y'", [], |r| r.get::<_, String>(0))
        .ok()
        .and_then(|s| s.parse::<i32>().ok());
    Ok(match (x, y) {
        (Some(x), Some(y)) => Some((x, y)),
        _ => None,
    })
}

pub fn save_window_position(conn: &Connection, x: i32, y: i32) -> Result<(), String> {
    for (key, value) in [("window_pos_x", x), ("window_pos_y", y)] {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value.to_string()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

- [ ] **Step 2: 启动时定位并监听移动**

`src-tauri/src/lib.rs` 在 `setup` 中初始化数据库之后增加：
```rust
use tauri::{LogicalPosition, Manager, WindowEvent};
let window = app.get_webview_window("main").expect("未找到主窗口");
{
    let state = app.state::<AppState>();
    let conn = state.conn.lock().unwrap();
    match config::get_window_position(&conn).unwrap_or(None) {
        Some((x, y)) => {
            let _ = window.set_position(LogicalPosition::new(x as f64, y as f64));
        }
        None => {
            if let Ok(Some(monitor)) = app.primary_monitor() {
                let size = monitor.size();
                let scale = monitor.scale_factor();
                let x = (size.width as f64 / scale) - 320.0 - 24.0;
                let y = (size.height as f64 / scale) - 240.0 - 24.0;
                let _ = window.set_position(LogicalPosition::new(x.max(0.0), y.max(0.0)));
            }
        }
    }
}
let app_handle = app.handle().clone();
window.on_window_event(move |event| {
    if let WindowEvent::Moved(pos) = event {
        let state = app_handle.state::<AppState>();
        if let Ok(conn) = state.conn.lock() {
            let _ = config::save_window_position(&conn, pos.x, pos.y);
        }
    }
});
```

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/config.rs src-tauri/src/lib.rs
git commit -m "feat: 添加宠窗默认位置与位置记忆"
```

---

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

- [ ] **Step 7: 提交**

```bash
git add src-tauri/src/user_profiles.rs src-tauri/src/db.rs src-tauri/src/sessions.rs src-tauri/src/models.rs src-tauri/src/commands.rs src-tauri/src/lib.rs src/windows/SettingsWindow.tsx src/windows/ChatWindow.tsx src/api.ts src/types.ts
git commit -m "feat: 添加用户设定 CRUD 与会话绑定"
```

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

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/persona.rs src-tauri/src/commands.rs
git commit -m "feat: 创建人设后自动生成表情库"
```

### Task 11.2: 喂食计时与饿/无聊气泡

**Files:**
- Modify: `src-tauri/src/pet.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: 分离喂食计时**

`PetRuntime` 增加 `last_feed: Mutex<Instant>`。`register_interaction` 的 `feed` 分支同时更新 `last_feed` 与 `last_interaction`。饥饿判断使用 `last_feed.elapsed()`，无聊/睡觉使用 `last_interaction.elapsed()`。

- [ ] **Step 2: 状态气泡事件**

`start_pet_loop` 中当状态变为 `Hungry` 或 `Bored` 时，若与上一次发出的状态不同，额外 `emit("pet-state-bubble", state)`；前端只显示一次状态提示；无聊主动消息统一由 Task 11.3 后台概率任务触发，避免两条路径重复请求 AI。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/pet.rs src-tauri/src/state.rs src-tauri/src/commands.rs
git commit -m "feat: 分离喂食计时并添加饿/无聊气泡"
```

### Task 11.3: 无聊主动消息

**Files:**
- Modify: `src-tauri/src/pet.rs`
- Create: `src-tauri/src/proactive.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 编写无聊主动消息生成**

`src-tauri/src/proactive.rs` 增加 `generate(app: &AppHandle)`：
1. 取当前人设（`personas.is_active`）。
2. 取上下文会话：优先 `current_session`；否则该人设最近更新的会话；否则 `None`。
3. 有会话时调用 `context::assemble`，传入会话所属人设 prompt、会话绑定的用户设定（未绑定为 `None`）、`get_environment()` 得到的环境文本（含当天节日）、工具说明与记忆参数；否则仅人设+环境。
4. 用聊天 provider 生成一条简短气泡并 `emit("proactive-message", text)`。

- [ ] **Step 2: 按无聊状态与概率触发**

`src-tauri/src/pet.rs` 的桌宠循环中，状态为 `Bored` 时按固定概率尝试触发一次 `proactive::generate`；用冷却时间与“同一段无聊状态只发一条”避免刷屏。休眠、出门中、隐藏宠窗、正在聊天时不触发。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/proactive.rs src-tauri/src/pet.rs src-tauri/src/lib.rs
git commit -m "feat: 添加无聊主动消息"
```

### Task 11.4: 提醒编辑、测试连接、短期记忆游标

**Files:**
- Modify: `src-tauri/src/reminders.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/providers.rs`
- Modify: `src-tauri/src/memory.rs`
- Modify: `src-tauri/src/db.rs`

- [ ] **Step 1: 提醒编辑命令**

`src-tauri/src/reminders.rs` 增加 `update_reminder(conn, id, kind, schedule, content, fire_at, status)`；`commands.rs` 暴露 `update_reminder`。

- [ ] **Step 2: 提供商测试连接**

`src-tauri/src/providers.rs` 增加 `test_connection(target: &ApiTarget) -> Result<(), String>`：发送使用所选模型的最小 chat 请求并验证响应；仅 models 列表可访问不能证明模型可用。`commands.rs` 暴露 `test_provider(input)` 临时构造 target 测试。

- [ ] **Step 3: 短期记忆游标**

`sessions.summary_cursor` 已包含在 `db.rs::init` 的建表语句中。`memory.rs` 的 `maybe_consolidate` 改为：读取 `summary_cursor`，只总结 `id > summary_cursor` 且属于最近 30 条之外的消息；生成短期记忆后把 `summary_cursor` 更新为当前已处理的最大消息 id。避免重复总结。

- [ ] **Step 4: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/reminders.rs src-tauri/src/commands.rs src-tauri/src/providers.rs src-tauri/src/memory.rs src-tauri/src/db.rs
git commit -m "feat: 添加提醒编辑、测试连接与短期记忆游标"
```

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

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/pet.rs src-tauri/src/lib.rs src/windows/PetWindow.tsx
git commit -m "feat: 添加桌宠桌面游走"
```

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

目标：当前人设与初始设定分离；聊天回复完成后由 AI 自判是否调整当前人设；用户可保存和恢复初始设定。

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

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/db.rs src-tauri/src/models.rs src-tauri/src/persona.rs src-tauri/src/commands.rs
git commit -m "feat: 添加人设初始设定与恢复命令"
```

### Task 12.2: 聊天后人设自调

**Files:**
- Modify: `src-tauri/src/chat.rs`
- Modify: `src-tauri/src/ai/client.rs`
- Modify: `src-tauri/src/persona.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: 增加人设自调工具**

为聊天 function calling 增加 `update_persona_prompt(new_prompt)`；仅当当前会话所属人设的 `self_evolve_enabled=1` 时允许调用。工具返回后写入 `personas.prompt` 和 `last_evolved_at`，保持原有人设核心。

- [ ] **Step 2: 接入回复完成流程**

流式回复完成后执行工具调用阶段；AI 自行判断是否需要调用。写入成功后在宠窗发出 `persona-updated` 气泡，并通知设置窗刷新。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/chat.rs src-tauri/src/ai/client.rs src-tauri/src/persona.rs src-tauri/src/commands.rs
git commit -m "feat: 添加聊天后人设自调"
```

### Task 12.3: 人设自调设置界面

**Files:**
- Modify: `src/windows/SettingsWindow.tsx`
- Modify: `src/api.ts`
- Modify: `src/types.ts`
- Modify: `src/components/Kaomoji.tsx`
- Modify: `src/windows/PetWindow.tsx`

- [ ] **Step 1: 展示人设当前值与初始设定**

人设卡片显示当前人设、初始设定、自调开关和最近调整时间。

- [ ] **Step 2: 增加操作按钮**

提供“保存为初始设定”“从初始设定恢复”按钮；恢复前弹确认框。

- [ ] **Step 3: 增加颜色选择与渲染**

人设卡片提供预设色板 + 自定义颜色选择器；`Kaomoji.tsx` 和 `PetWindow.tsx` 使用当前会话所属人设的 `color` 渲染基础颜文字，饰品层、气泡和窗口背景不受影响。

- [ ] **Step 4: 编译验证**

Run: `npm run build`
Expected: 无错误。

- [ ] **Step 5: 提交**

```bash
git add src/windows/SettingsWindow.tsx src/api.ts src/types.ts src/components/Kaomoji.tsx src/windows/PetWindow.tsx
git commit -m "feat: 添加人设自调与本体颜色设置界面"
```

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

Rust 命令接收文件名和字节内容，校验扩展名为 `jpg / png / webp`、单张不超过 10MB，写入 `data/chat_background/`，并在 `personas` 中保存相对文件名。前端只显示预览，不读原始路径。

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

- [ ] **Step 7: 提交**

```bash
git add src-tauri/src/db.rs src-tauri/src/models.rs src-tauri/src/persona.rs src-tauri/src/commands.rs src/windows/ChatWindow.tsx src/windows/SettingsWindow.tsx src/api.ts src/types.ts
git commit -m "feat: 添加按人设隔离的聊天窗背景设置"
```
## 阶段 13：亲密度与喵币

目标：会话亲密度、全局喵币、每日签到和石头剪刀布游戏。

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

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/db.rs src-tauri/src/models.rs src-tauri/src/currency.rs src-tauri/src/commands.rs
git commit -m "feat: 添加亲密度与喵币数据层"
```

### Task 13.2: 互动增加亲密度

**Files:**
- Modify: `src-tauri/src/pet.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src/windows/PetWindow.tsx`

- [ ] **Step 1: 更新互动命令**

点击、摸摸、喂食命令成功时给当前会话亲密度 +1；AI 出门中时不增加。

- [ ] **Step 2: 聊天轮数增加亲密度**

一轮完整用户消息与 AI 回复完成后，当前会话亲密度 +1。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/pet.rs src-tauri/src/commands.rs src/windows/PetWindow.tsx
git commit -m "feat: 添加互动与聊天亲密度"
```

### Task 13.3: 签到与石头剪刀布

**Files:**
- Modify: `src-tauri/src/currency.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src/windows/SettingsWindow.tsx`
- Modify: `src/api.ts`

- [ ] **Step 1: 实现每日签到**

签到命令按本地日期判断当天是否已签到；无节日命中时随机发放 1–10 喵币，写入流水和余额。命中节日时的固定 10 喵币与祝福词在阶段 18 接入。

- [ ] **Step 2: 实现石头剪刀布**

`play_rps(choice)` 由系统随机出拳；赢 +5、输 -4、平局 0；开始前校验余额 ≥ 4，并统计当天游戏次数不超过 20。

- [ ] **Step 3: 增加设置页入口**

“设置 > 桌宠与内容 > 桌宠状态”显示喵币余额、签到按钮、石头剪刀布按钮和亲密度。

- [ ] **Step 4: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Run: `npm run build`
Expected: 无错误。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/currency.rs src-tauri/src/commands.rs src/windows/SettingsWindow.tsx src/api.ts
git commit -m "feat: 添加签到与石头剪刀布"
```

---

## 阶段 14：旅行与写信

目标：会话级旅行状态、出门/返回、写信与回信。

### Task 14.1: 当前会话旅行计时与状态

**Files:** `src-tauri/src/travel.rs`、`models.rs`、`commands.rs`、`state.rs`。

- [ ] 接入完整 schema 的 travels 和会话计时字段，不重复 ALTER TABLE。
- [ ] 仅当前会话在家空闲时累计在线秒数，阈值抽取一次保存；聊天、互动、睡觉、休眠、隐藏宠窗、出门中不累计，其他会话不累计。
- [ ] 出发事务建立 away 旅行，保存计划返回时间与 midway_letter_at；前端事件提交后发出。
- [ ] 正常返回事务设 returned，取消未完成途中信与待回信，确定 gift_kind，保存下次间隔。回来信、礼物、总结使用各自 pending/done 状态。
- [ ] 切换会话/人设事务取消原 away 旅行与待回信，记录 cancelled_at，并把尚未完成旅行任务设 cancelled；不发返回结果。清零旧会话计时并保存新阈值，已存在记录保留。
- [ ] 验证多次重复返回、并发创建旅行、切换取消、仅当前会话计时；唯一索引防止同会话两条 away。

### Task 14.2: 关闭与启动恢复

**Files:** `src-tauri/src/travel.rs`、`lib.rs`、`config.rs`。

- [ ] 定期持久化在线秒数，正常退出再保存；关闭期间不累计在线时间。
- [ ] 启动先恢复 current_session_id，再恢复任务。未到期 away 继续，已到期先正常返回，实际返回时间采用计划返回时间；关闭不等于切换取消。
- [ ] 扫描所有 returned 的未完成生成状态进行补做，cancelled 不恢复；即使结果被用户删除，done 也不得重置。
- [ ] 返回与取消先于待回信补偿；旅行已到期就取消待回信，不补生成过期回复。
- [ ] 验证关闭未到期、关闭已到期、返回生成一半崩溃、取消后重启、结果删除后重启。

### Task 14.3: 信件与回信保存

**Files:** `src-tauri/src/letters.rs`、`travel.rs`、`models.rs`、`commands.rs`。

- [ ] 接入 reply_due_at、reply_state、has_been_replied、letter_kind；用户信到期时间首次随机抽取并保存，重启不重抽。
- [ ] midway/return 信分别在旅行到中点/正常返回后生成。结果和对应 done 标记在同一事务内提交，AI 请求在事务外。
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

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/db.rs src-tauri/src/models.rs src-tauri/src/accessories.rs src-tauri/src/commands.rs
git commit -m "feat: 添加饰品数据与命令"
```

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

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/chat.rs src-tauri/src/accessories.rs src-tauri/src/currency.rs
git commit -m "feat: 添加 AI 制作饰品流程"
```

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

- [ ] **Step 4: 提交**

```bash
git add src/components/Kaomoji.tsx src/components/AccessoryLayer.tsx src/windows/PetWindow.tsx src/windows/SettingsWindow.tsx
git commit -m "feat: 添加饰品层与饰品管理"
```

---

## 阶段 16：伴手礼与旅行总结

目标：返回礼物进入伴手礼管理；旅行总结写入短期记忆。

### Task 16.1: 伴手礼数据与管理

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/models.rs`
- Create: `src-tauri/src/souvenirs.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src/windows/TravelWindow.tsx`

- [ ] **Step 1: 创建伴手礼表**

按 `db/schema.sql` 创建 `souvenirs`；只保存普通文字礼物，可佩戴饰品进入 `accessories`。

- [ ] **Step 2: 生成返回礼物**

仅正常返回生成 1 件伴手礼；在返回事务中抽取并固定 gift_kind（10% accessory、90% text），生成后事务写结果与 gift_state=done。取消旅行不生成，失败重试不得重新抽类型或重复发放。

- [ ] **Step 3: 增加伴手礼操作**

支持查看、收藏、删除；不做背包、数量、容量或使用机制。

- [ ] **Step 4: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Run: `npm run build`
Expected: 无错误。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/db.rs src-tauri/src/models.rs src-tauri/src/souvenirs.rs src-tauri/src/commands.rs src/windows/TravelWindow.tsx
git commit -m "feat: 添加伴手礼数据与管理"
```

### Task 16.2: 旅行总结写入短期记忆

**Files:**
- Modify: `src-tauri/src/travel.rs`
- Modify: `src-tauri/src/memory.rs`
- Modify: `src-tauri/src/ai/client.rs`

- [ ] **Step 1: 生成旅行总结**

正常返回且回来信、礼物已保存后，使用记忆 provider 生成短期总结；读取持久化信件和礼物，不编造尚未生成结果。取消旅行不总结。

- [ ] **Step 2: 写入短期记忆**

以 source_type=travel、source_ref_id=travels.id 写入 short_memories，并在同一事务把 summary_state 设为 done、增加 memory_revision；参与后续长期整合。总结被合并或用户删除后保持 done，不再补生成。

- [ ] **Step 3: 应用关闭期间返回的补生成**

启动扫描正常 returned 且 summary_state=pending 的任务，按依赖顺序补生成；失败保持 pending 并提示，事务提交前重新检查状态。

- [ ] **Step 4: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/travel.rs src-tauri/src/memory.rs src-tauri/src/ai/client.rs
git commit -m "feat: 添加旅行总结短期记忆"
```

### Task 16.3: 记忆与出行页联动

**Files:**
- Modify: `src/windows/MemoryWindow.tsx`
- Modify: `src/windows/TravelWindow.tsx`
- Modify: `src/api.ts`

- [ ] **Step 1: 记忆小窗显示旅行总结**

短期记忆列表展示 `travel` 类型来源标记。

- [ ] **Step 2: 出行页与记忆小窗会话联动**

记忆小窗和出行页统一跟随 Rust 当前会话；在聊天窗/人设设置切换后刷新邮箱、伴手礼和记忆入口。出行页不增加独立会话选择器。

- [ ] **Step 3: 编译验证**

Run: `npm run build`
Expected: 无错误。

- [ ] **Step 4: 提交**

```bash
git add src/windows/MemoryWindow.tsx src/windows/TravelWindow.tsx src/api.ts
git commit -m "feat: 添加记忆与出行页联动"
```

---

## 阶段 17：导入导出与数据迁移

目标：提供全量备份、指定会话、指定用户设定、人设资料包（分享用）和人设完整包（迁移用）的导出与导入。

### Task 17.1: 统一导出协议

**Files:** `src-tauri/src/import_export.rs`、`commands.rs`、`lib.rs`。

- [ ] 按数据库与持久化设计第 9 节实现 manifest.json、data.json、files/，禁止混用按表分文件格式；manifest 记录格式/结构版本、类型、数量和文件清单。
- [ ] 全量导出覆盖所有非密钥全局与会话数据，包括 settings、提供商配置、occurrences、短期消息关联、档位标记、旅行完成状态、信件回复状态。节日母表不导出，子表母表引用导出稳定 code。
- [ ] 人设资料包仅定义和外观；人设完整包/会话包/带会话用户设定包带全所引用的人设定义、用户设定、背景图及会话级子记录，支持独立导入。
- [ ] 非全量包不带钱包、账单、全局设置、提醒或提供商。任何包都不带 API Key 或和风密钥明文/密文。
- [ ] 使用一致性读取快照与文件清单完成 ZIP；验证备份内容没有密钥，五种包关联均闭合。

### Task 17.2: 导入、映射与原子提交

**Files:** `src-tauri/src/import_export.rs`、`commands.rs`、`src/windows/SettingsWindow.tsx`、`src/api.ts`。

- [ ] 导入摘要展示范围和全局替换影响。默认新建副本；资料包仅新建，其他覆盖模式明确选择同类型/父级下的目标，不能用名称匹配后自动覆盖多条。
- [ ] 全量包用备份替换全局钱包、账单、settings、提醒与 occurrences、提供商配置；不叠加，不给人设复制钱包。部分包不动全局数据或现有密钥。
- [ ] 按表建立 ID 映射，处理多态 source_ref_id、reply_to_id、消息关联、summary_cursor 与 settings.current_session_id；festival_id 通过 code 映射。保留已回复、档位已用、取消及任务完成状态。
- [ ] 新导入提供商密钥为 NULL；全量密钥项清空并提示重新填写。重建当前项，保证部分唯一索引不冲突。
- [ ] 先验证版本、结构、字段、关系、图片大小和路径，暂存并使用唯一文件名；暂停后台写入，数据库同一事务完成覆盖、全局替换及所有关联写入。失败回滚并清理本次新文件，旧引用文件提交前不删除。
- [ ] 提交后恢复调度，取消任务不得恢复。测试五种包往返、同名歧义、ID 重映射、密钥排除、全局替换、文件失败与数据库失败回滚，不能只运行编译检查。

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

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/lunar.rs
git commit -m "feat: 添加内置农历与节气数据表"
```

### Task 18.2: 节日母表内置数据与每年查表更新

**Files:**
- Modify: `src-tauri/src/db.rs`
- Create: `src-tauri/src/festivals.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 建表与内置数据**

按 `db/schema.sql` 创建 `festival_catalog`；首次启动按 `code` 幂等写入内置节日（公历：元旦、劳动节、国庆节；农历：腊八、小年、除夕、春节、元宵、端午、七夕、中秋、重阳；节气：清明、冬至），写入时用 `lunar.rs` 直接把当年的公历月日算好一起写入，并写入内置祝福词与 `is_memorial`。

- [ ] **Step 2: 每年查表更新**

启动、跨年或系统日期变化时比较母表中的 year 与当前公历年份：不一致则只挑 `calendar_type` 为 `lunar` 或 `solar_term` 的行，查 `lunar.rs` 里的内置表推出 `solar_month`、`solar_day`，连同 `year` 一起 `UPDATE` 回该行；`calendar_type` 为 `solar` 的行只更新 `year`，不动月日。全程不删除、不重建、不改动 `code`。除夕这类“腊月最后一天”的节日，其农历日（廿九或三十）同样由内置农历表推出后写回 `lunar_day`。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/db.rs src-tauri/src/festivals.rs src-tauri/src/lib.rs
git commit -m "feat: 添加节日母表与每年查表更新"
```

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

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/festivals.rs src-tauri/src/commands.rs src-tauri/src/context.rs src-tauri/src/chat.rs
git commit -m "feat: 添加节日子表与 AI 工具"
```

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

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/currency.rs src-tauri/src/commands.rs src/windows/FestivalPanel.tsx src/windows/SettingsWindow.tsx src/api.ts
git commit -m "feat: 添加签到彩蛋与节日管理页"
```

### Task 18.5: 导出导入覆盖节日子表

**Files:**
- Modify: `src-tauri/src/import_export.rs`

- [ ] **Step 1: 纳入导出范围**

所有含会话的包都携带 session_festivals；关联母表改用稳定 code 传输并在目标端映射 ID。母表是内置数据，不导出。导入后按当前年份重新计算日期，不沿用过期换算值。

- [ ] **Step 2: 提交**

```bash
git add src-tauri/src/import_export.rs
git commit -m "feat: 导出导入覆盖节日子表"
```

---

## 收尾与交付检查

- [ ] **Step 1: 全量测试与构建**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS。
Run: `npm test`
Expected: PASS。
Run: `npm run tauri build`
Expected: 生成 `src-tauri/target/release/颜喵 YanMew.exe`。
Run: `New-Item -ItemType Directory -Force -Path dist | Out-Null; Compress-Archive -Path "src-tauri/target/release/颜喵 YanMew.exe" -DestinationPath "dist/颜喵 YanMew-portable.zip"`
Expected: 得到便携版 zip，解压后双击运行，数据库自动写入 exe 同目录 `data/`。

- [ ] **Step 2: 提交收尾**

```bash
git add -A
git commit -m "chore: 完成颜喵 YanMew 首版实现"
```

---

## 交付前检查

- 规格覆盖：窗口/托盘、提供商、人设、人设本体颜色、聊天窗背景、用户设定、表情库（点击：开心/惊讶/害羞、饿、吃东西、困、无聊、思考）、流式聊天与暂停中断、function calling 记事本、会话记忆、桌宠状态、节日母表与节日子表、签到彩蛋、无聊主动消息、桌面游走、提醒、天气定位、错误休眠、默认人设、开机自启、人设自调、喵币与游戏、亲密度、饰品、旅行、写信、伴手礼、旅行总结、导入导出与数据迁移均有对应 Task。
- 已确定：节日彩蛋完全离线——母表是内置静态数据，农历与节气日期每年查内置表推出后写回，不接任何日历接口，不做放假与调休。
- 已确定：签到命中节日固定 10 喵币并直接展示节日祝福词（不调用 AI）；同时命中母表与子表时合并祝福词、金额仍为 10；纪念性节日不使用“快乐”文案。
- 已确定：无聊主动消息取代原空闲气泡，触发概率与上下文由代码层决定；桌面游走在用户拖动、点击、聊天、睡觉、出门中、隐藏宠窗时暂停。
- 实现覆盖：每项有明确命令契约、事务边界和验收条件；示例代码需按后续任务同步更新，不将描述性步骤宣称为已完成实现。
- 类型一致性：前端类型与 Rust 模型字段命名一致；命令名前后一致。
- 应用图标使用用户提供的 `cat_icon.png`，已转为 `src-tauri/icons/icon.png`。
- 已确定：天气采用 Open-Meteo 优先、和风天气兜底，和风 API Key 在设置中配置（DPAPI 加密存储）；`create_persona` 自动生成表情库依赖 provider 可用，无 provider 时静默跳过并在设置页提示手动生成。
- 已确定：删除人设级联删除其会话与会话级数据；提醒到点由 AI 用人设语气润色、失败回退原文；关闭期间一次性提醒询问补/过期、重复提醒只补最近一次；宠窗默认右下角并记忆位置。
- 已确定：喵币为全局用户级，不随会话删除；亲密度、节日子表、饰品、旅行、信件、伴手礼均为会话级，删除会话时级联删除。
- 已确定：只有当前会话在家空闲时累计；切换取消旅行和待回信；正常关闭后启动按真实时间续算，取消任务不恢复。
- 已确定：密钥不导出，新导入提供商重新填写；全量导入替换全局数据，部分包不改目标全局配置；人设资料包只新建副本并自动重命名，且包含聊天窗背景图和表情库。
