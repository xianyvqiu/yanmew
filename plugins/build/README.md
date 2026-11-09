# 插件构建和校验

从零编写、完整 manifest、全部宿主接口、调试、迁移和发布步骤见[小游戏插件开发与发布教程](../sdk/README.md)。本文件负责打包工具本身，不要求插件作者修改主程序源码。

在项目根目录使用同一个 Rust 校验器打包，不需要新增 Node.js 或 Python 依赖：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin yanmew-plugin -- pack plugins/template example.yanmew-game
cargo run --manifest-path src-tauri/Cargo.toml --bin yanmew-plugin -- validate example.yanmew-game
```

输出已存在时不会覆盖。压缩包最大 64 MiB，展开总量最大 128 MiB，单文件最大 32 MiB，最多 4096 个条目。只接受宿主支持的网页、图片、字体、音视频及文本资源；拒绝可执行程序、符号链接、大小写重复路径及 Windows 特殊路径。资源使用 ASCII 相对路径，页面内资源引用不得指向外网。

安装使用 `.yanmew-game` ZIP，必须在根目录包含 `manifest.json`。模板只是宿主连接和存档演示，不是内置小游戏，也不列入随包清单。主仓库或社区公开分发必须补充 GitHub `repository`；私人开发包可以留空。

## 六个自带游戏

运行 `npm run plugins:build`，构建脚本先生成共用Rust CLI，再从plugins/bundled-games.json打包六个源码目录到resources/plugins，并逐包validate。临时输出也必须以.yanmew-game结尾，校验成功才替换稳定文件名；shared/runtime.js复制到各包。前四款复制window.css，扫雷和2048复制classic.css与classic-runtime.js，各包可独立安装。新增规则、限次及结算检查为 `node --test plugins/build/classic-games.test.mjs`。

Tauri的beforeDevCommand/beforeBuildCommand会先执行该步骤。便携版发布需执行 `./scripts/copy-plugin-resources.ps1 -Destination <便携版目录>`，将包和清单放到exe旁resources/plugins；同样复制Embedding资源。不复制开发数据库和用户存档。源码、规则及检查见../first-party/README.md。
