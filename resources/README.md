# 便携版模型与小游戏资源

`models/bge-small-zh-v1.5/` 是 Windows x64 便携版所需资源，应用不依赖 Python、CUDA 或独立模型服务。默认 CPU 推理，最多使用 4 个逻辑线程，单实例按需加载，空闲不自旋。

模型为官方 `BAAI/bge-small-zh-v1.5` revision `7999e1d3359715c523056ef9478215996d62a620` 的 FP32 导出，512 维、最大序列 512，CLS 池化与 L2 归一化。查询前缀是“为这个句子生成表示以用于检索相关文章：”，记忆标题/正文不添加查询前缀；超长输入不截断，只保留全文检索。

`manifest.json` 锁定资源 SHA-256、版本、预处理、导出方法和许可证来源。Rust 使用 `ort=2.0.0-rc.10`、`tokenizers=0.21.4`，绝对路径加载随包的 ONNX Runtime `1.22.0` DLL。模型文件 94,847,655 字节，资源目录约 105.5 MiB；发布脚本复制清单内资源及 manifest，用户数据和开发工具不随包复制。

开发模式显式读取项目资源目录。发布模式读取可执行文件旁的 `resources/models/bge-small-zh-v1.5/`。准备便携版暂存目录后，在项目根目录执行：

```powershell
./scripts/copy-embedding-resources.ps1 -Destination <便携版暂存目录>
```

完整便携版仍须由最终发布阶段加入 exe 和其他应用资源；本脚本只负责 Embedding 资源。模型及运行库不进入用户备份，向量缓存可重建。

完整发布使用项目根目录的 `scripts/build-portable.ps1`，统一调用本地模型、小游戏和日历的复制脚本。ONNX Runtime 的 MSVC 依赖由构建机器 Visual Studio 的 x64 Redistributable CRT 目录复制到 exe 旁，不从 System32 复制系统 DLL；程序和 ONNX Runtime 共用这些运行组件。`portable-README.md` 作为使用说明随包提供，第三方依赖许可文本单独复制到发布目录的 `licenses/`，不复制开发数据库或个人存档。

开发期复现导出：使用 Python 3.12、CPU torch 2.6.0、transformers 4.49.0、onnx 1.17.0、onnxruntime 1.22.0、safetensors 0.5.3，安装到项目临时目录 `.test-temp/model-build/`，再运行 `scripts/prepare-embedding-model.py`。脚本下载固定 revision 的官方 safetensors 到 `.test-temp/model-source/`，导出 ONNX opset 17，并比较三个样本的官方 FP32/ONNX 输出，最大绝对误差必须 <1e-5。首次结果为约 3.13e-7。工具目录和原始权重是开发期材料，应用运行不使用它们；正式模型仍保存在 `resources/models/`，不得移到本地记录或临时目录。

一次 Rust/Windows CPU 验证：Intel Core i7-12700H，20 个逻辑 CPU，推理使用 4 线程，debug 测试进程。加载（含资源校验）1,911 ms，首条短样本 4.72 ms，后续三条平均 3.28 ms；工作集由 6.17 MiB 到加载后 181.95 MiB，测试峰值 184.48 MiB。此数据是本机短样本测量，不是所有机器的速度或完整应用内存承诺。相关查询余弦得分 0.620–0.666，无关查询最高 0.261，首版相关阈值 0.60；检索排名融合不是概率。

缺失、校验失败和推理错误只让 Embedding 局部不可用，不自动上传、不让桌宠休眠；修复后测试当前模型或重启恢复。资源变更时重新验证，常规开发无需反复跑资源测试。

许可证：模型官方标注 MIT，保留 `MODEL_CARD.md` 与 `MODEL_LICENSE`；ONNX Runtime 保留 `ONNXRUNTIME_LICENSE` 与第三方通知。固定来源与每份文件哈希见 manifest。

以上资源验证的 Windows 系统版本为 10.0.26200.0。

## 自带小游戏资源

plugins/包含剪刀石头布、骰子、接星星、我的家、扫雷、2048的.yanmew-game与bundled-games.json。由 `npm run plugins:build` 从标准插件源码生成并逐包校验；Tauri开发/构建自动调用。开发加载项目目录，发布加载exe旁resources/plugins。便携版还需运行 `./scripts/copy-plugin-resources.ps1 -Destination <便携版暂存目录>`，与上面的Embedding复制步骤一起执行。六个游戏没有单独权威余额或初始币，存档和权限由主程序SQLite保存；程序资源不进入用户备份。

## 内置日历与许可声明

`calendar/` 是18.1的离线农历/节气资源，采用MIT固定源码生成并编译进程序，运行时不依赖Python或外部资源下载。源码快照、二进制格式、版本、校验和及核对说明见 [日历资源说明](calendar/README.md)。便携打包执行 `./scripts/copy-calendar-notices.ps1 -Destination <便携版目录>`，保留上游版权、MIT许可证、manifest和来源说明。日历源码ZIP只用于开发复现，不随用户包或备份复制。

## 预置图片人设资源

personas/ 含 GPT、Claude、Gemini、DeepSeek 的初始图和独立基础互动图，详情及生成提示见 [资源说明](personas/README.md)。完整便携版/安装版必须带齐四套124张透明PNG；build-portable.ps1 校验并复制。首次运行发布到本机受管图片目录，之后以用户修改为准，不覆盖已删除角色。程序资源不需要生图 Key，用户自定义创作继续使用其配置的服务。

## 食品图

foods/包含免费Token小点心和五种付费食物的256×256透明PNG。由前端静态导入并随构建内嵌，不需要另外下载、不从用户备份覆盖；角色咀嚼图不画食物。来源与生成提示见foods/README.md及generation-prompts.json。

## 本地抠图资源

`models/background-removal/` 保存通用 U²-Net（175,997,641 字节）、固定来源/SHA-256 清单及上游许可。`model.onnx` 使用 Git LFS；从 GitHub 克隆源码前安装 Git LFS，克隆后执行 `git lfs pull` 取回完整模型。LFS 指针文件不能用作模型，发布脚本会核对 SHA-256 并在资源缺失或不完整时停止。

便携版通过 `scripts/copy-background-resources.ps1` 复制模型和许可，沿用随包 ONNX Runtime；运行时不下载模型、不需要抠图 API Key。仅透明资产生成结果缺乏透明度时自动兜底；普通聊天绘图保留原始背景，按用户需要再通过工具检测或抠图。
