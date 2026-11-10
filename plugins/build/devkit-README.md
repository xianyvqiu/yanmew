# 颜喵小游戏开发工具包

适用于 Windows 10/11 x64，宿主协议 `1.0.0`。此包用于编写、打包和校验小游戏，不包含颜喵主程序或用户数据。

## 开始开发

本包另有[喵币计算挑战完整示例](examples/wallet-challenge/README.md)，用于学习扣费、奖励和异常恢复；不要用自己的日常钱包测试。它不列入社区游戏目录。基础入门仍先使用不申请钱包权限的猜数字示例。

1. 从 [开发工具发布页](https://github.com/xianyvqiu/yanmew-community-games/releases/tag/devkit-v1.0.2) 下载 ZIP 和同名 `.sha256`。在 PowerShell 运行 `Get-FileHash -Algorithm SHA256 <ZIP路径>`，与校验文件比对后解压。
2. 阅读 [从零编写小游戏及完整接口教程](sdk/README.md)。`sdk/index.d.ts` 是完整接口类型，三个 JSON Schema 可用于编辑器提示。
3. 复制 `template/` 到自己的开发目录，修改 manifest 中的 ID、名称、作者及版本，编写 `ui/` 下的 HTML、CSS、JavaScript。模板只演示连接和存档，不会被收录为社区作品。
4. 用下方命令打包，再在颜喵“全局设置 → 小游戏插件 → 从本地安装”中安装测试。

仅使用打包器不需要安装 Rust、Node.js、WebView2 或模型。CLI 使用 Windows 系统运行库，全部随包文件须保留。使用颜喵运行游戏仍需按颜喵本身的环境要求安装 WebView2。Node.js 22 及以上用于社区仓库的本地检查和维护者发布脚本。

## 打包与校验

以下命令在本开发包解压目录执行，修改源码和输出路径为自己的实际位置：

```powershell
$gameSource = 'C:\games\my-game'
$packageFile = Join-Path $env:TEMP 'org.example.my-game-1.0.0.yanmew-game'
& .\bin\yanmew-plugin.exe pack $gameSource $packageFile
& .\bin\yanmew-plugin.exe validate $packageFile
Get-FileHash -LiteralPath $packageFile -Algorithm SHA256
```

输出必须为尚不存在的 `.yanmew-game` 文件。打包工具与颜喵安装器使用相同的结构校验规则，不执行游戏。校验通过后仍须手动测试玩法、授权、喵币、存档和升级。

`DEVKIT.json` 记录本开发包版本、协议、Windows 平台、CLI 哈希和构建来源；版本号与颜喵应用版本分别管理。`SHA256SUMS` 校验解压后各文件，根目录 `LICENSE` 及 `THIRD_PARTY_NOTICES.md` 保留工具与依赖许可。

## 社区投稿和发布

阅读 [社区投稿说明](https://github.com/xianyvqiu/yanmew-community-games/blob/main/CONTRIBUTING.md)，向 `games/<plugin-id>/` 提交源码 PR，附带 README 和 `LICENSE.txt`。不要提交成品包、密钥、个人数据或 `node_modules`。

作者使用社区仓库的 `npm run check:local` 进行本地结构检查和实际打包，再提交源码 PR。维护者审查并合入 main 后，在社区仓库干净且已同步的 main 执行 `npm run release:local -- <plugin-id>`；脚本下载并核验固定工具，读取 manifest 版本、复用本机 GitHub 登录，发布 `<plugin-id>/v<version>` 的插件附件并更新目录。步骤见社区 README，不需要 GitHub Actions、付款方式、仓库 Secret 或手填 token。

开发包使用 `devkit-v<version>` tag，独立于游戏版本，不进入小游戏目录。Node.js/Git 仅供社区脚本使用，单独编写和打包插件仍可直接运行本包 CLI。
