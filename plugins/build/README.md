# 插件构建和校验

从零编写、完整 manifest、全部宿主接口、调试、迁移和发布步骤见[小游戏插件开发与发布教程](../sdk/README.md)。本文件负责打包工具本身，不要求插件作者修改主程序源码。

普通作者推荐下载[预编译开发工具包](https://github.com/xianyvqiu/yanmew-community-games/releases/tag/devkit-v1.0.2)，校验 SHA-256 后使用 `bin/yanmew-plugin.exe pack <源码目录> <输出包>` 和 `validate <包>`，无需编译颜喵或安装 Rust/Node.js。SDK、模板、完整教程及许可证随开发包提供，公开文档也可在[社区 developer 目录](https://github.com/xianyvqiu/yanmew-community-games/tree/main/developer)阅读。

维护者在项目根目录也可以使用同一个 Rust 校验器打包，不需要新增 Node.js 或 Python 依赖：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin yanmew-plugin -- pack plugins/template example.yanmew-game
cargo run --manifest-path src-tauri/Cargo.toml --bin yanmew-plugin -- validate example.yanmew-game
```

输出已存在时不会覆盖。压缩包最大 64 MiB，展开总量最大 128 MiB，单文件最大 32 MiB，最多 4096 个条目。只接受宿主支持的网页、图片、字体、音视频及文本资源；拒绝可执行程序、符号链接、大小写重复路径及 Windows 特殊路径。资源使用 ASCII 相对路径，页面内资源引用不得指向外网。

安装使用 `.yanmew-game` ZIP，必须在根目录包含 `manifest.json`。模板只是宿主连接和存档演示，不是内置小游戏，也不列入随包清单。主仓库或社区公开分发必须补充 GitHub `repository`；私人开发包可以留空。

## 发布开发工具包

维护者配好主仓库开发环境后，运行 `./scripts/build-plugin-devkit.ps1 -Version 1.0.2`，生成 `releases/plugin-devkit/<版本>/` 下的 ZIP、SHA-256 和解压目录。脚本编译 release CLI，按白名单导出 SDK、模板和教程，收集 Rust 依赖及标准库许可，生成 `DEVKIT.json` 和文件清单 `SHA256SUMS`；不包含应用、用户数据、模型或第一方游戏。

输出目录已存在会拒绝覆盖；`-SkipBuild` 只供维护者复用已经验证的 CLI，不跳过版本和许可收集。产物需从 ZIP 解压到独立目录测试 `pack`/`validate`，再以 `devkit-v<版本>` 在社区仓库发布 ZIP 和同名 `.sha256`。社区的 `toolchain.json` 固定 URL、文件大小、ZIP/CLI 两个 SHA-256；`developer/` 保留对应版本的 SDK、模板及教程文本快照，不提交 EXE。

社区本地检查和发布脚本只下载公开开发包，不访问主仓库；作者运行 `npm run check:local`，维护者在同步的干净 main 运行 `npm run release:local -- <plugin-id>`。发布登录复用本机 Git 凭据，命令不会修改 Git 身份或要求手填 token。社区仓库不启用 Actions 自动工作流。工具包版本与应用、插件版本分别管理；更新工具不占用游戏目录，也不替代游戏实机验收。

## 六个自带游戏

运行 `npm run plugins:build`，构建脚本先生成共用Rust CLI，再从plugins/bundled-games.json打包六个源码目录到resources/plugins，并逐包validate。临时输出也必须以.yanmew-game结尾，校验成功才替换稳定文件名；shared/runtime.js复制到各包。前四款复制window.css，扫雷和2048复制classic.css与classic-runtime.js，各包可独立安装。新增规则、限次及结算检查为 `node --test plugins/build/classic-games.test.mjs`。

Tauri的beforeDevCommand/beforeBuildCommand会先执行该步骤。便携版发布需执行 `./scripts/copy-plugin-resources.ps1 -Destination <便携版目录>`，将包和清单放到exe旁resources/plugins；同样复制Embedding资源。不复制开发数据库和用户存档。源码、规则及检查见../first-party/README.md。
